//! Trusted TASK-005 gate: `Grid` model and client-side sorting (D-050..D-053).

mod support;

use pgtui::db::Cell;
use pgtui::grid::{Grid, SortDir, SortState};

fn customers_grid() -> Grid {
    Grid::from(support::fake_data::customers())
}

/// Values of column 0 (`id`) in the current visible order.
fn ids(grid: &Grid) -> Vec<String> {
    grid.visible_rows()
        .iter()
        .map(|row| match &row[0] {
            Cell::Text(text) => text.clone(),
            Cell::Null => "<null>".to_string(),
        })
        .collect()
}

fn sorted(column: usize, dir: SortDir) -> Vec<String> {
    let mut grid = customers_grid();
    grid.sort = Some(SortState { column, dir });
    ids(&grid)
}

#[test]
fn from_result_set_keeps_original_order() {
    let grid = customers_grid();
    assert_eq!(
        grid.columns,
        vec![
            "id".to_string(),
            "name".to_string(),
            "balance".to_string(),
            "signup_date".to_string(),
            "note".to_string()
        ]
    );
    assert_eq!(grid.sort, None);
    assert_eq!(grid.row_cursor, 0);
    assert_eq!(grid.col_cursor, 0);
    assert_eq!(ids(&grid).len(), 5);
    assert_eq!(ids(&grid)[0], "1", "seed order by id");
    assert_eq!(ids(&grid)[4], "5");
}

#[test]
fn numeric_column_sorts_numerically() {
    assert_eq!(
        sorted(2, SortDir::Asc),
        vec!["3", "1", "4", "2", "5"],
        "-3.25 < 10.50 < 99.99 < 250.00, ties keep seed order"
    );
}

#[test]
fn numeric_desc_keeps_ties_stable() {
    assert_eq!(
        sorted(2, SortDir::Desc),
        vec!["2", "5", "4", "1", "3"],
        "equal 250.00 rows keep seed order"
    );
}

#[test]
fn text_column_sorts_byte_wise_with_nulls_last_on_asc() {
    assert_eq!(
        sorted(4, SortDir::Asc),
        vec!["3", "5", "1", "2", "4"],
        "refund < tie < vip, NULLs last"
    );
}

#[test]
fn text_column_nulls_first_on_desc() {
    assert_eq!(
        sorted(4, SortDir::Desc),
        vec!["2", "4", "1", "5", "3"],
        "NULLs first, then vip > tie > refund"
    );
}

#[test]
fn clearing_sort_restores_seed_order() {
    let mut grid = customers_grid();
    grid.sort = Some(SortState {
        column: 2,
        dir: SortDir::Asc,
    });
    assert_eq!(ids(&grid), vec!["3", "1", "4", "2", "5"]);

    grid.sort = None;
    assert_eq!(
        ids(&grid),
        vec!["1", "2", "3", "4", "5"],
        "source untouched"
    );
}

#[test]
fn other_column_starts_at_asc() {
    assert_eq!(
        sorted(1, SortDir::Asc),
        vec!["1", "2", "3", "4", "5"],
        "Ada < Bob < Cyd < Dee < Eve"
    );
    assert_eq!(sorted(1, SortDir::Desc), vec!["5", "4", "3", "2", "1"]);
}

#[test]
fn empty_result_set_yields_empty_grid() {
    let grid = Grid::from(support::fake_data::empty_table());
    assert_eq!(grid.columns, vec!["id".to_string()]);
    assert!(grid.visible_rows().is_empty());
}
