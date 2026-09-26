//! Trusted TASK-005 gate: preview flow and grid keys (D-011, D-033, D-050..D-053).

mod support;

use pgtui::app::{App, Effect, Focus, Msg, QueryKind, Screen, Status};
use pgtui::db::{DbError, QueryOutcome, TableRef};
use pgtui::grid::{Grid, SortDir};

fn browser_app(table_index: usize) -> App {
    let mut app = App::new();
    app.update(Msg::Connections(vec![support::saved_connection("local")]));
    app.update(Msg::Connected(Ok(support::fake_data::tables())));
    for _ in 0..table_index {
        app.update(support::key('j'));
    }
    app
}

fn preview_kind(kind: &QueryKind) -> TableRef {
    let QueryKind::Preview(table) = kind else {
        panic!("expected a preview, got {kind:?}");
    };
    table.clone()
}

/// Runs the effect the app emitted and feeds the fake seed rows back.
fn load_preview(app: &mut App) {
    let effects = app.update(support::enter());
    assert_eq!(effects.len(), 1, "{effects:?}");
    let table = preview_kind(&effect_kind(&effects[0]));
    let kind = QueryKind::Preview(table.clone());
    let rows = support::fake_data::preview(&table);
    let effects = app.update(Msg::QueryDone {
        kind,
        result: Ok(QueryOutcome::Rows(rows)),
    });
    assert!(effects.is_empty(), "{effects:?}");
}

fn effect_kind(effect: &Effect) -> QueryKind {
    match effect {
        Effect::Query { kind, .. } => kind.clone(),
        other => panic!("expected a query effect, got {other:?}"),
    }
}

fn grid(app: &App) -> &Grid {
    app.grid.as_ref().expect("a grid is loaded")
}

#[test]
fn enter_on_table_emits_preview_effect() {
    let mut app = browser_app(1);
    let effects = app.update(support::enter());
    assert_eq!(
        effects,
        vec![Effect::Query {
            kind: QueryKind::Preview(TableRef {
                schema: "public".to_string(),
                name: "customers".to_string(),
            }),
            sql: "SELECT * FROM \"public\".\"customers\" LIMIT 500".to_string(),
        }],
        "exact D-025 preview SQL"
    );
    assert_eq!(app.screen, Screen::Browser);
    assert!(app.grid.is_none(), "no grid until the reply arrives");
}

#[test]
fn query_done_ok_builds_grid_and_focuses_grid() {
    let mut app = browser_app(1);
    load_preview(&mut app);

    let loaded = grid(&app);
    assert_eq!(loaded.columns.len(), 5);
    assert_eq!(loaded.row_cursor, 0);
    assert_eq!(loaded.col_cursor, 0);
    assert_eq!(loaded.sort, None, "a new preview starts unsorted");
    assert_eq!(loaded.visible_rows().len(), 5);
    assert_eq!(app.session.as_ref().expect("session").focus, Focus::Grid);
}

#[test]
fn grid_cursors_move_clamped() {
    let mut app = browser_app(1);
    load_preview(&mut app);

    for _ in 0..8 {
        app.update(support::key('l'));
    }
    assert_eq!(grid(&app).col_cursor, 4, "last column of 5");
    for _ in 0..8 {
        app.update(support::key('j'));
    }
    assert_eq!(grid(&app).row_cursor, 4, "last row of 5");

    for _ in 0..8 {
        app.update(support::key('h'));
        app.update(support::key('k'));
    }
    assert_eq!(grid(&app).col_cursor, 0);
    assert_eq!(grid(&app).row_cursor, 0);

    app.update(support::right());
    app.update(support::down());
    assert_eq!(grid(&app).col_cursor, 1);
    assert_eq!(grid(&app).row_cursor, 1);
}

#[test]
fn s_cycles_sort_on_cursor_column() {
    let mut app = browser_app(1);
    load_preview(&mut app);
    app.update(support::key('l'));
    app.update(support::key('l'));

    app.update(support::key('s'));
    assert_eq!(
        grid(&app).sort,
        Some(pgtui::grid::SortState {
            column: 2,
            dir: SortDir::Asc
        })
    );

    app.update(support::key('s'));
    assert_eq!(
        grid(&app).sort,
        Some(pgtui::grid::SortState {
            column: 2,
            dir: SortDir::Desc
        })
    );

    app.update(support::key('s'));
    assert_eq!(grid(&app).sort, None, "third press clears the sort");
}

#[test]
fn s_on_other_column_starts_asc() {
    let mut app = browser_app(1);
    load_preview(&mut app);
    app.update(support::key('l'));
    app.update(support::key('l'));
    app.update(support::key('s'));
    app.update(support::key('s'));

    app.update(support::key('l'));
    app.update(support::key('s'));
    assert_eq!(
        grid(&app).sort,
        Some(pgtui::grid::SortState {
            column: 3,
            dir: SortDir::Asc
        }),
        "moving to another column restarts at Asc"
    );
}

#[test]
fn new_preview_resets_sort_and_cursors() {
    let mut app = browser_app(1);
    load_preview(&mut app);
    app.update(support::key('l'));
    app.update(support::key('l'));
    app.update(support::key('s'));
    app.update(support::key('j'));
    assert!(grid(&app).sort.is_some());

    app.update(support::tab());
    assert_eq!(app.session.as_ref().expect("session").focus, Focus::Sidebar);
    app.update(support::key('j'));
    load_preview(&mut app);

    let loaded = grid(&app);
    assert_eq!(loaded.sort, None, "sort resets on a new preview");
    assert_eq!(loaded.row_cursor, 0);
    assert_eq!(loaded.col_cursor, 0);
    assert_eq!(app.session.as_ref().expect("session").focus, Focus::Grid);
}

#[test]
fn query_error_keeps_grid_and_sets_status() {
    let mut app = browser_app(1);
    load_preview(&mut app);
    let before_columns = grid(&app).columns.clone();
    let before_rows = grid(&app).visible_rows().len();

    app.update(support::tab());
    app.update(support::key('j'));
    let effects = app.update(support::enter());
    assert_eq!(effects.len(), 1);
    let kind = effect_kind(&effects[0]);

    let effects = app.update(Msg::QueryDone {
        kind,
        result: Err(DbError::Query("relation does not exist".to_string())),
    });
    assert!(effects.is_empty());

    assert_eq!(
        app.status,
        Status::Error("relation does not exist".to_string())
    );
    let after = grid(&app);
    assert_eq!(before_columns, after.columns, "grid untouched");
    assert_eq!(before_rows, after.visible_rows().len());
}

#[test]
fn tab_toggles_focus_when_grid_is_loaded() {
    let mut app = browser_app(1);
    load_preview(&mut app);

    app.update(support::tab());
    assert_eq!(app.session.as_ref().expect("session").focus, Focus::Sidebar);
    app.update(support::tab());
    assert_eq!(app.session.as_ref().expect("session").focus, Focus::Grid);
}
