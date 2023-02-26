#![doc(issue_tracker_base_url = "https://github.com/mycelial/sqlite-parser-nom/issues", test(no_crate_inject))]
#![doc = include_str!("../README.md")]

extern crate core;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use nom::Finish;

use crate::error::{OwnedBytes, SQLiteError};
use crate::model::Database;
use crate::parser::database;

pub mod error;
pub mod model;
pub mod parser;
mod varint;
mod be_i48;

/*
todo: parse additional page types (overflow, lock, freelist, ?)
todo: determine when overflow page no is used
todo: how page size computation works?
todo: test with more records
todo: how freelist pages work?
todo: add mmap option for reading
*/

/// Loads the whole file in memory, does copying while parsing as well
/// hence, requires at least 2x of free memory of original file size.
///
/// Recommended way is to use specific parsers based on your needs instead.
///
/// ```
/// let database = sqlite_parser_nom::open("database.sqlite3");
/// ```
// todo: pass file as bytes and take page-by-page from it to avoid reading all bytes
pub fn open<P: AsRef<Path>>(path: P) -> Result<Database, SQLiteError> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let (_, db) = database(bytes.as_slice())
        .finish()
        .map_err(|e| nom::error::Error {
            code: e.code,
            input: OwnedBytes(e.input.to_owned()),
        })?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use crate::model::SerialType::{Null, Text, I8};
    use crate::model::{Page, Payload};
    use rusqlite::Connection;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn empty_db() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.sqlite3");
        // let path = "empty.sqlite3";
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)",
            (),
        )
        .unwrap();
        conn.close().unwrap();
        let result = open(&path).unwrap();

        assert_eq!(result.header.page_size.real_size(), 4096);
        assert_eq!(result.pages.len(), 2); // root page + 1 empty page

        match result.pages.first().unwrap() {
            Page::LeafTable(p) => {
                assert_eq!(p.header.no_cells, 1);
                assert_eq!(p.cells.len(), 1);
                assert_eq!(
                    p.cells.first().unwrap().payload.column_types,
                    // type, name, tbl_name, rootpage, sql
                    vec![Text(23), Text(21), Text(21), I8, Text(135)]
                );
                assert_eq!(
                    p.cells.first().unwrap().payload.column_values,
                    vec![
                        Some(Payload::Text("table".to_string())),
                        Some(Payload::Text("test".to_string())),
                        Some(Payload::Text("test".to_string())),
                        Some(Payload::I8(2)),
                        Some(Payload::Text(
                            "CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)"
                                .to_string()
                        )),
                    ]
                );
            }
            _ => unreachable!("root page should be table leaf page"),
        }

        match result.pages.last().unwrap() {
            Page::LeafTable(p) => {
                assert_eq!(p.header.no_cells, 0);
                assert_eq!(p.cells.len(), 0);
            }
            _ => unreachable!("second page should be leaf page"),
        }
    }

    #[test]
    fn parse_table_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.sqlite3");
        let conn = Connection::open(&path).unwrap();
        conn.execute(
            "CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)",
            (),
        )
        .unwrap();
        conn.execute("INSERT INTO test VALUES (42, 'tjena tjena')", ())
            .unwrap();
        conn.close().unwrap();
        let result = open(&path).unwrap();

        assert_eq!(result.pages.len(), 2); // root page + 1st page with table content

        match result.pages.last().unwrap() {
            Page::LeafTable(p) => {
                assert_eq!(p.header.no_cells, 1);
                assert_eq!(p.cells.len(), 1);
                assert_eq!(p.cells.first().unwrap().rowid, 42);
                assert_eq!(
                    p.cells.first().unwrap().payload.column_types,
                    // type, name, tbl_name, rootpage, sql
                    vec![Null, Text(35)]
                );
                assert_eq!(
                    p.cells.first().unwrap().payload.column_values,
                    vec![None, Some(Payload::Text("tjena tjena".to_string())),]
                );
            }
            _ => unreachable!("root page should be table leaf page"),
        }
    }
}
