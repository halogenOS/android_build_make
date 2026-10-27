// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: Apache-2.0

//! Writes tables into an `SQLite` database.

use std::io::BufRead;

use rusqlite::{params_from_iter, Connection};

use crate::table::{Table, TableError};

/// The table that contains the rows of one file in the order of the file,
/// until they are copied, sorted, into the table of the file.
const UNSORTED_ROWS: &str = "\"unsorted rows\"";

#[derive(Debug, thiserror::Error)]
pub enum InsertError {
    #[error("cannot create the table")]
    Create(#[source] rusqlite::Error),
    #[error("cannot prepare the insert statement")]
    PrepareInsert(#[source] rusqlite::Error),
    #[error("cannot read a row")]
    Read(#[source] TableError),
    #[error("cannot insert row {row}")]
    Insert { row: usize, source: rusqlite::Error },
    #[error("cannot copy the sorted rows into the table")]
    Sort(#[source] rusqlite::Error),
}

/// Creates the table `name` with a TEXT column for every column of the table
/// and inserts its rows, ordered by the sort columns and then by their line.
///
/// Each row is written into a table of unsorted rows as soon as it is read,
/// and `SQLite` sorts them into the table, so the rows are never held in memory
/// as a whole.
pub fn insert_table<R: BufRead>(
    connection: &Connection,
    name: &str,
    table: Table<'_, R>,
) -> Result<(), InsertError> {
    let layout = &table.layout;
    let values: Vec<String> = (0..layout.columns.len()).map(|i| format!("value_{i}")).collect();
    let order =
        comma_separated(layout.sort.iter().map(|&i| values[i].clone()).chain(["line".to_owned()]));
    let selected = comma_separated(values.iter().enumerate().map(|(i, value)| {
        if Some(i) == layout.number {
            format!("row_number() OVER (ORDER BY {order}) - 1")
        } else {
            value.clone()
        }
    }));
    let name = quote_identifier(name);
    connection
        .execute_batch(&format!(
            "CREATE TABLE {name} ({});
             CREATE TABLE {UNSORTED_ROWS} ({}, line TEXT);",
            comma_separated(
                layout.columns.iter().map(|column| format!("{} TEXT", quote_identifier(column)))
            ),
            comma_separated(values.iter().map(|value| format!("{value} TEXT"))),
        ))
        .map_err(InsertError::Create)?;

    let placeholders = vec!["?"; values.len() + 1].join(", ");
    let mut insert = connection
        .prepare(&format!("INSERT INTO {UNSORTED_ROWS} VALUES ({placeholders})"))
        .map_err(InsertError::PrepareInsert)?;
    for (row, read) in table.enumerate() {
        let read = read.map_err(InsertError::Read)?;
        let _inserted_rows = insert
            .execute(params_from_iter(read.values.iter().chain([&read.line])))
            .map_err(|source| InsertError::Insert { row, source })?;
    }

    connection
        .execute_batch(&format!(
            "INSERT INTO {name} SELECT {selected} FROM {UNSORTED_ROWS} ORDER BY {order};
             DROP TABLE {UNSORTED_ROWS};"
        ))
        .map_err(InsertError::Sort)
}

fn comma_separated(items: impl Iterator<Item = String>) -> String {
    items.collect::<Vec<_>>().join(", ")
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::tests::roots;

    /// Imports the table in `text` as `name` and returns the text of every
    /// column of every row, in the order of the rows.
    fn import(connection: &Connection, name: &str, text: &str) -> Vec<Vec<String>> {
        let roots = roots();
        insert_table(connection, name, Table::read(text.as_bytes(), &roots).unwrap()).unwrap();
        let mut select = connection
            .prepare(&format!("SELECT * FROM {} ORDER BY rowid", quote_identifier(name)))
            .unwrap();
        let count = select.column_count();
        select
            .query_map([], |row| (0..count).map(|i| row.get(i)).collect())
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn writes_every_value_as_text() {
        let connection = Connection::open_in_memory().unwrap();
        let rows = import(
            &connection,
            "my \"table\"",
            concat!(
                r#"{"columns":["id","a \"b\""],"sort":[],"number":"id"}"#,
                "\n",
                r#"{"a \"b\"":"x y"}"#,
                "\n",
                "{}\n",
            ),
        );
        assert_eq!(rows, [["0", "x y"], ["1", ""]]);

        let types: String = connection
            .query_row(
                r#"SELECT group_concat(DISTINCT typeof(id) || typeof("a ""b""")) FROM "my ""table""""#,
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(types, "texttext");
        let tables: String = connection
            .query_row("SELECT group_concat(name) FROM sqlite_schema", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tables, "my \"table\"");
    }

    #[test]
    fn sorts_by_the_sort_columns_then_by_line_and_numbers_the_rows() {
        let connection = Connection::open_in_memory().unwrap();
        let rows = import(
            &connection,
            "t",
            concat!(
                r#"{"columns":["id","name","variant"],"sort":["name"],"number":"id"}"#,
                "\n",
                r#"{"name":"b"}"#,
                "\n",
                r#"{"variant":"y","name":"a"}"#,
                "\n",
                r#"{"name":"a","variant":"z"}"#,
                "\n",
            ),
        );
        assert_eq!(rows, [["0", "a", "z"], ["1", "a", "y"], ["2", "b", ""]]);
    }

    #[test]
    fn sorts_by_resolved_paths() {
        let connection = Connection::open_in_memory().unwrap();
        let rows = import(
            &connection,
            "t",
            concat!(
                r#"{"columns":["file"],"sort":["file"]}"#,
                "\n",
                r#"{"file":{"src":["a"]}}"#,
                "\n",
                r#"{"file":{"out":["b"]}}"#,
                "\n",
            ),
        );
        assert_eq!(rows, [["/o/b"], ["a"]]);
    }
}
