//! In-memory `TableRef`/`ResultSet` values identical to `tests/fixtures/seed.sql`
//! (D-072). Every cell is the exact text PostgreSQL returns over the
//! simple-query protocol for the container's default timezone `UTC`.

#![allow(dead_code)]

use pgtui::db::{Cell, ResultSet, TableRef};

fn table(schema: &str, name: &str) -> TableRef {
    TableRef {
        schema: schema.to_string(),
        name: name.to_string(),
    }
}

/// The four seed tables in D-025 server order.
pub fn tables() -> Vec<TableRef> {
    vec![
        table("audit", "events"),
        table("public", "customers"),
        table("public", "empty_table"),
        table("public", "orders"),
    ]
}

/// An empty string in a fixture row means SQL NULL (the seed has no empty
/// text values).
fn cells(row: &[&str]) -> Vec<Cell> {
    row.iter()
        .map(|value| {
            if value.is_empty() {
                Cell::Null
            } else {
                Cell::Text((*value).to_string())
            }
        })
        .collect()
}

/// `public.customers` — 5 rows, two NULL notes.
pub fn customers() -> ResultSet {
    ResultSet {
        columns: vec![
            "id".into(),
            "name".into(),
            "balance".into(),
            "signup_date".into(),
            "note".into(),
        ],
        rows: vec![
            cells(&["1", "Ada", "10.50", "2024-01-05", "vip"]),
            cells(&["2", "Bob", "250.00", "2024-02-10", ""]),
            cells(&["3", "Cyd", "-3.25", "2024-03-15", "refund"]),
            cells(&["4", "Dee", "99.99", "2024-04-20", ""]),
            cells(&["5", "Eve", "250.00", "2024-05-25", "tie"]),
        ],
    }
}

/// `public.orders` — 7 rows.
pub fn orders() -> ResultSet {
    ResultSet {
        columns: vec![
            "id".into(),
            "customer_id".into(),
            "total".into(),
            "status".into(),
        ],
        rows: vec![
            cells(&["1", "1", "5.00", "paid"]),
            cells(&["2", "1", "7.50", "paid"]),
            cells(&["3", "2", "100.00", "open"]),
            cells(&["4", "3", "1.25", "refunded"]),
            cells(&["5", "4", "49.99", "paid"]),
            cells(&["6", "5", "200.00", "open"]),
            cells(&["7", "5", "50.00", "paid"]),
        ],
    }
}

/// `public.empty_table` — 0 rows.
pub fn empty_table() -> ResultSet {
    ResultSet {
        columns: vec!["id".into()],
        rows: Vec::new(),
    }
}

/// `audit.events` — 3 rows, timestamptz rendered in UTC.
pub fn events() -> ResultSet {
    ResultSet {
        columns: vec!["id".into(), "kind".into(), "at".into()],
        rows: vec![
            cells(&["1", "login", "2024-06-01 10:00:00+00"]),
            cells(&["2", "query", "2024-06-01 10:05:00+00"]),
            cells(&["3", "logout", "2024-06-01 10:30:00+00"]),
        ],
    }
}

/// The seed rows for `table`; panics on anything else (the seed has no other
/// table, so a panic here means the test built a wrong `TableRef`).
pub fn preview(table: &TableRef) -> ResultSet {
    match (table.schema.as_str(), table.name.as_str()) {
        ("public", "customers") => customers(),
        ("public", "orders") => orders(),
        ("public", "empty_table") => empty_table(),
        ("audit", "events") => events(),
        other => panic!("fake_data::preview has no seed table {other:?}"),
    }
}

/// Text key of a row, used for order-insensitive comparison.
pub fn row_key(row: &[Cell]) -> String {
    row.iter()
        .map(|cell| match cell {
            Cell::Null => "<null>".to_string(),
            Cell::Text(text) => text.clone(),
        })
        .collect::<Vec<_>>()
        .join("|")
}

/// Rows of `set` sorted by `row_key`.
pub fn sorted_rows(set: &ResultSet) -> Vec<String> {
    let mut keys: Vec<String> = set.rows.iter().map(|row| row_key(row)).collect();
    keys.sort();
    keys
}
