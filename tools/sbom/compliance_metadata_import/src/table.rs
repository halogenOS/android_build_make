// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: Apache-2.0

//! A table read from a file of JSON lines.
//!
//! The first line is the header, every other line is a row:
//!
//! ```text
//! {"columns":["id","name","files"],"sort":["name"],"number":"id"}
//! {"name":"a","files":{"out":["x/a"],"src":["a"]}}
//! ```

use std::collections::{BTreeMap, HashMap};
use std::io::{self, BufRead, Lines};
use std::iter::Enumerate;

use serde::Deserialize;

use crate::roots::{PathError, Roots};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    /// The columns of the table, in table order.
    columns: Vec<String>,
    /// The columns whose text orders the rows.
    sort: Vec<String>,
    /// The column that gets the position of the row after sorting.
    #[serde(default)]
    number: Option<String>,
}

/// The value of a column in a row.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Value {
    Text(String),
    /// Words joined by single spaces.
    Words(Vec<String>),
    /// Paths below their roots, ordered by the names of the roots.
    Paths(BTreeMap<String, Vec<String>>),
}

#[derive(Debug, thiserror::Error)]
pub enum TableError {
    #[error("cannot read line {line}")]
    Read { line: usize, source: io::Error },
    #[error("the header line is missing")]
    MissingHeader,
    #[error("line {line} is not valid")]
    Parse { line: usize, source: serde_json::Error },
    #[error("the header lists {0}, which is not a column")]
    UnknownHeaderColumn(String),
    #[error("line {line} sets the column {column}, which is not a column")]
    UnknownColumn { line: usize, column: String },
    #[error("line {line} sets the column {column}, which the importer numbers")]
    NumberColumnSet { line: usize, column: String },
    #[error("line {line} has an invalid path")]
    Path { line: usize, source: PathError },
}

/// The positions of the columns that the header lists.
pub struct Layout {
    /// The columns of the table, in table order.
    pub columns: Vec<String>,
    /// The positions of the columns whose text orders the rows.
    pub sort: Vec<usize>,
    /// The position of the column that gets the position of the row after
    /// sorting.
    pub number: Option<usize>,
    indexes: HashMap<String, usize>,
}

/// A row with the line it was read from, which orders rows of equal sort
/// columns.
pub struct Row {
    pub line: String,
    /// The text of every column, in table order. The number column is empty.
    pub values: Vec<String>,
}

/// A table whose rows are read one at a time, in the order of the file.
pub struct Table<'r, R> {
    pub layout: Layout,
    lines: Enumerate<Lines<R>>,
    roots: &'r Roots,
}

impl<'r, R: BufRead> Table<'r, R> {
    /// Reads the header line. Every path of a row is resolved against its root.
    pub fn read(input: R, roots: &'r Roots) -> Result<Self, TableError> {
        let mut lines = input.lines().enumerate();
        let (_, header_line) = lines.next().ok_or(TableError::MissingHeader)?;
        let header_line = header_line.map_err(|source| TableError::Read { line: 1, source })?;
        let header: Header = serde_json::from_str(&header_line)
            .map_err(|source| TableError::Parse { line: 1, source })?;
        Ok(Self { layout: Layout::new(header)?, lines, roots })
    }
}

impl<R: BufRead> Iterator for Table<'_, R> {
    type Item = Result<Row, TableError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (index, line) = self.lines.next()?;
        let number = index + 1;
        Some(line.map_err(|source| TableError::Read { line: number, source }).and_then(|text| {
            let values = self.layout.read_row(&text, number, self.roots)?;
            Ok(Row { line: text, values })
        }))
    }
}

impl Layout {
    fn new(header: Header) -> Result<Self, TableError> {
        let indexes: HashMap<String, usize> =
            header.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
        let index_of = |column: String| {
            indexes.get(&column).copied().ok_or(TableError::UnknownHeaderColumn(column))
        };
        let sort = header.sort.into_iter().map(index_of).collect::<Result<_, _>>()?;
        let number = header.number.map(index_of).transpose()?;
        Ok(Self { columns: header.columns, sort, number, indexes })
    }

    /// Returns the text of every column of the row, which is empty for a
    /// column that the row leaves out.
    fn read_row(&self, text: &str, line: usize, roots: &Roots) -> Result<Vec<String>, TableError> {
        let row: HashMap<String, Value> =
            serde_json::from_str(text).map_err(|source| TableError::Parse { line, source })?;
        let mut values = vec![String::new(); self.columns.len()];
        for (column, value) in row {
            let Some(&index) = self.indexes.get(&column) else {
                return Err(TableError::UnknownColumn { line, column });
            };
            if Some(index) == self.number {
                return Err(TableError::NumberColumnSet { line, column });
            }
            values[index] =
                value.resolve(roots).map_err(|source| TableError::Path { line, source })?;
        }
        Ok(values)
    }
}

impl Value {
    fn resolve(self, roots: &Roots) -> Result<String, PathError> {
        match self {
            Value::Text(text) => Ok(text),
            Value::Words(words) => Ok(words.join(" ")),
            Value::Paths(paths) => Ok(paths
                .iter()
                .flat_map(|(root, relatives)| relatives.iter().map(|r| roots.resolve(root, r)))
                .collect::<Result<Vec<_>, _>>()?
                .join(" ")),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::roots::parse_root;

    pub fn roots() -> Roots {
        Roots::new(["out=/o", "src="].map(|a| parse_root(a).unwrap()).into()).unwrap()
    }

    /// Returns the values of every row of the table in `text`.
    fn read(text: &str) -> Result<Vec<Vec<String>>, TableError> {
        let roots = roots();
        Table::read(text.as_bytes(), &roots)?.map(|row| Ok(row?.values)).collect()
    }

    #[test]
    fn resolves_every_kind_of_value() {
        let roots = roots();
        let text = concat!(
            r#"{"columns":["text","words","paths","absent"],"sort":[]}"#,
            "\n",
            r#"{"text":"a b","words":["c","d"],"paths":{"src":["s"],"out":["x","y"]}}"#,
            "\n",
        );
        let table = Table::read(text.as_bytes(), &roots).unwrap();
        assert_eq!(table.layout.columns, ["text", "words", "paths", "absent"]);
        let rows: Vec<Row> = table.collect::<Result<_, _>>().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].values, ["a b", "c d", "/o/x /o/y s", ""]);
        assert_eq!(rows[0].line, text.lines().nth(1).unwrap());
    }

    #[test]
    fn finds_the_sort_and_number_columns() {
        let roots = roots();
        let text = r#"{"columns":["id","name","variant"],"sort":["variant","name"],"number":"id"}"#;
        let layout = Table::read(text.as_bytes(), &roots).unwrap().layout;
        assert_eq!(layout.sort, [2, 1]);
        assert_eq!(layout.number, Some(0));
    }

    #[test]
    fn refuses_invalid_files() {
        assert!(matches!(read(""), Err(TableError::MissingHeader)));
        assert!(matches!(
            read(r#"{"columns":["a"],"sort":["b"]}"#),
            Err(TableError::UnknownHeaderColumn(column)) if column == "b"
        ));
        assert!(matches!(
            read(r#"{"columns":["a"],"sort":[],"numbers":"a"}"#),
            Err(TableError::Parse { line: 1, .. })
        ));
        assert!(matches!(
            read("{\"columns\":[\"a\"],\"sort\":[]}\n{\"b\":\"x\"}\n"),
            Err(TableError::UnknownColumn { line: 2, column }) if column == "b"
        ));
        assert!(matches!(
            read("{\"columns\":[\"a\"],\"sort\":[],\"number\":\"a\"}\n{\"a\":\"0\"}\n"),
            Err(TableError::NumberColumnSet { line: 2, .. })
        ));
        assert!(matches!(
            read("{\"columns\":[\"a\"],\"sort\":[]}\n{\"a\":7}\n"),
            Err(TableError::Parse { line: 2, .. })
        ));
        assert!(matches!(
            read("{\"columns\":[\"a\"],\"sort\":[]}\n{\"a\":{\"dist\":[\"x\"]}}\n"),
            Err(TableError::Path { line: 2, source: PathError::UnknownRoot(_) })
        ));
    }
}
