// SPDX-FileCopyrightText: The halogenOS Project
// SPDX-License-Identifier: Apache-2.0

//! The directories that the paths in the files are relative to.

use std::collections::HashMap;

/// A root given on the command line as `NAME=DIR`.
#[derive(Clone, Debug, PartialEq)]
pub struct Root {
    name: String,
    dir: String,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RootError {
    #[error("{0} is not NAME=DIR")]
    InvalidArgument(String),
    #[error("the root {0} is given twice")]
    Duplicate(String),
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PathError {
    #[error("the root {0} is not given with --root")]
    UnknownRoot(String),
    #[error("{0:?} is not a relative path")]
    InvalidRelativePath(String),
}

/// Reads a `--root` argument.
pub fn parse_root(argument: &str) -> Result<Root, RootError> {
    match argument.split_once('=') {
        Some((name, dir)) if !name.is_empty() => {
            Ok(Root { name: name.to_owned(), dir: dir.to_owned() })
        }
        _ => Err(RootError::InvalidArgument(argument.to_owned())),
    }
}

/// The directory of every root.
#[derive(Debug, Default)]
pub struct Roots(HashMap<String, String>);

impl Roots {
    pub fn new(roots: Vec<Root>) -> Result<Self, RootError> {
        let mut dirs = HashMap::new();
        for Root { name, dir } in roots {
            if dirs.contains_key(&name) {
                return Err(RootError::Duplicate(name));
            }
            let _previous = dirs.insert(name, dir);
        }
        Ok(Self(dirs))
    }

    /// Returns the path below the root, which is written as it was for a root
    /// with an empty directory.
    pub fn resolve(&self, root: &str, relative: &str) -> Result<String, PathError> {
        if relative.is_empty() || relative.starts_with('/') {
            return Err(PathError::InvalidRelativePath(relative.to_owned()));
        }
        match self.0.get(root).map(String::as_str) {
            None => Err(PathError::UnknownRoot(root.to_owned())),
            Some("") => Ok(relative.to_owned()),
            Some(dir) => Ok(format!("{dir}/{relative}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(arguments: &[&str]) -> Roots {
        Roots::new(arguments.iter().map(|a| parse_root(a).unwrap()).collect()).unwrap()
    }

    #[test]
    fn resolves_below_the_directory_of_the_root() {
        let roots = roots(&["out=/o", "src="]);
        assert_eq!(roots.resolve("out", "a/b"), Ok("/o/a/b".to_owned()));
        assert_eq!(roots.resolve("src", "a/b"), Ok("a/b".to_owned()));
        assert_eq!(roots.resolve("x", "a"), Err(PathError::UnknownRoot("x".to_owned())));
    }

    #[test]
    fn refuses_paths_that_are_not_relative() {
        let roots = roots(&["out=/o"]);
        for path in ["", "/a"] {
            assert_eq!(
                roots.resolve("out", path),
                Err(PathError::InvalidRelativePath(path.to_owned()))
            );
        }
    }

    #[test]
    fn refuses_invalid_arguments() {
        assert_eq!(parse_root("out"), Err(RootError::InvalidArgument("out".to_owned())));
        assert_eq!(parse_root("=/o"), Err(RootError::InvalidArgument("=/o".to_owned())));
        assert_eq!(
            Roots::new(vec![parse_root("out=a").unwrap(), parse_root("out=b").unwrap()])
                .unwrap_err(),
            RootError::Duplicate("out".to_owned())
        );
    }
}
