// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: Apache-2.0

//! Imports files of JSON lines into an `SQLite` database, one table per file.
//! A table is named after its file without `.jsonl`. The header line of the
//! file lists the columns of the table, the columns that order the rows and the
//! column that numbers them.

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic, clippy::let_underscore_must_use, unused_results)]

mod database;
mod roots;
mod table;

use std::error::Error;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use rusqlite::Connection;

use crate::database::{insert_table, InsertError};
use crate::roots::{parse_root, Root, RootError, Roots};
use crate::table::{Table, TableError};

#[derive(Parser)]
struct Args {
    /// The database to write.
    #[arg(long)]
    output: PathBuf,
    /// The directory of a root of the paths, as NAME=DIR. An empty DIR leaves
    /// the paths of the root as written.
    #[arg(long = "root", value_parser = parse_root)]
    roots: Vec<Root>,
    /// Files of JSON lines, or @FILE for the whitespace-separated paths in FILE.
    #[arg(required = true)]
    inputs: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
enum ImportError {
    #[error(transparent)]
    Roots(#[from] RootError),
    #[error("cannot read the list of files {path}")]
    ReadFileList { path: PathBuf, source: io::Error },
    #[error("{0} is not a .jsonl file")]
    NotJsonLines(PathBuf),
    #[error("cannot open {path}")]
    OpenInput { path: PathBuf, source: io::Error },
    #[error("cannot read {path}")]
    ReadTable { path: PathBuf, source: TableError },
    #[error("cannot remove {path}")]
    RemoveTemporaryDatabase { path: PathBuf, source: io::Error },
    #[error("cannot open the database {path}")]
    OpenDatabase { path: PathBuf, source: rusqlite::Error },
    #[error("cannot write the table of {path}")]
    WriteTable { path: PathBuf, source: InsertError },
    #[error("cannot write the database {path}")]
    WriteDatabase { path: PathBuf, source: rusqlite::Error },
    #[error("cannot move the database {path} into place")]
    RenameDatabase { path: PathBuf, source: io::Error },
}

fn main() -> ExitCode {
    match import(&Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let mut message = error.to_string();
            let mut source = error.source();
            while let Some(cause) = source {
                message = format!("{message}: {cause}");
                source = cause.source();
            }
            eprintln!("compliance_metadata_import: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Writes every table into `<output>.tmp` and renames it to the output once
/// it is complete, so a failed run leaves no partial database at the output.
fn import(args: &Args) -> Result<(), ImportError> {
    let roots = Roots::new(args.roots.clone())?;
    let inputs = expand_file_lists(&args.inputs)?;

    let mut temporary_name = OsString::from(args.output.as_os_str());
    temporary_name.push(".tmp");
    let temporary = PathBuf::from(temporary_name);
    match fs::remove_file(&temporary) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(ImportError::RemoveTemporaryDatabase { path: temporary, source: error });
        }
        _ => {}
    }

    let open_error = |source| ImportError::OpenDatabase { path: temporary.clone(), source };
    let mut connection = Connection::open(&temporary).map_err(open_error)?;
    // The file is renamed into place only when it is complete, so a crash
    // needs no journal to recover from. The unsorted rows of each table are
    // dropped once the table is written, and auto_vacuum removes the space
    // that they used from the file.
    connection
        .execute_batch(
            "PRAGMA auto_vacuum = FULL; PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;",
        )
        .map_err(open_error)?;
    let write_error = |source| ImportError::WriteDatabase { path: temporary.clone(), source };
    let transaction = connection.transaction().map_err(write_error)?;
    for input in &inputs {
        let name = table_name(input)?;
        let file = File::open(input)
            .map_err(|source| ImportError::OpenInput { path: input.clone(), source })?;
        let table = Table::read(BufReader::new(file), &roots)
            .map_err(|source| ImportError::ReadTable { path: input.clone(), source })?;
        insert_table(&transaction, name, table)
            .map_err(|source| ImportError::WriteTable { path: input.clone(), source })?;
    }
    transaction.commit().map_err(write_error)?;
    connection.close().map_err(|(_, source)| write_error(source))?;

    fs::rename(&temporary, &args.output)
        .map_err(|source| ImportError::RenameDatabase { path: args.output.clone(), source })
}

/// Returns the inputs with every `@FILE` replaced by the paths listed in FILE.
fn expand_file_lists(inputs: &[String]) -> Result<Vec<PathBuf>, ImportError> {
    let mut paths = Vec::new();
    for input in inputs {
        match input.strip_prefix('@') {
            Some(list) => {
                let text = fs::read_to_string(list).map_err(|source| {
                    ImportError::ReadFileList { path: PathBuf::from(list), source }
                })?;
                paths.extend(text.split_whitespace().map(PathBuf::from));
            }
            None => paths.push(PathBuf::from(input)),
        }
    }
    Ok(paths)
}

/// Returns the name of the file without `.jsonl`.
fn table_name(path: &Path) -> Result<&str, ImportError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".jsonl"))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ImportError::NotJsonLines(path.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_file_list_by_its_paths() {
        let list = std::env::temp_dir().join(format!("file-list-{}", std::process::id()));
        fs::write(&list, "a.jsonl\n b.jsonl  c.jsonl\n").unwrap();
        let inputs = ["x.jsonl".to_owned(), format!("@{}", list.display())];
        let paths = expand_file_lists(&inputs);
        fs::remove_file(&list).unwrap();
        assert_eq!(paths.unwrap(), ["x.jsonl", "a.jsonl", "b.jsonl", "c.jsonl"].map(PathBuf::from));
    }

    #[test]
    fn names_a_table_after_its_file() {
        assert_eq!(table_name(Path::new("a/modules.jsonl")).unwrap(), "modules");
        assert!(table_name(Path::new("a/modules.csv")).is_err());
        assert!(table_name(Path::new("a/.jsonl")).is_err());
    }
}
