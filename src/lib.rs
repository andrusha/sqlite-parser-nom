extern crate core;

use std::fs::File;
use std::io::Read;
use std::path::Path;

use nom::Finish;

use crate::error::{OwnedBytes, SQLiteError};
use crate::model::Database;
use crate::parser::database;

mod varint;
mod model;
mod error;
mod parser;

/**
todo: parse additional page types (overflow, lock, freelist, ?)
todo: determine when overflow page no is used
todo: how page size computation works?
todo: test with more records
todo: how freelist pages work?
 **/

pub fn open<P: AsRef<Path>>(path: P) -> Result<Database, SQLiteError> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let (_, db) = database(bytes.as_slice())
        .finish()
        .map_err(|e| nom::error::Error { code: e.code, input: OwnedBytes(e.input.to_owned()) })?;
    Ok(db)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::tempdir;
    use crate::model::{Page, Payload};
    use crate::model::SerialType::{I8, Null, Text};

    use super::*;

    #[test]
    fn empty_db() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.sqlite3");
        // let path = "empty.sqlite3";
        let conn = Connection::open(&path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)", ()).unwrap();
        conn.close().unwrap();
        let result = open(&path).unwrap();

        assert_eq!(result.header.page_size.real_size(), 4096);
        assert_eq!(result.pages.len(), 2); // root page + 1 empty page

        match result.pages.first().unwrap() {
            Page::LeafTablePage(p) => {
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
                        Some(Payload::Text("CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)".to_string())),
                    ]
                );
            }
            _ => unreachable!("root page should be table leaf page")
        }

        match result.pages.last().unwrap() {
            Page::LeafTablePage(p) => {
                assert_eq!(p.header.no_cells, 0);
                assert_eq!(p.cells.len(), 0);
            }
            _ => unreachable!("second page should be leaf page")
        }
    }

    #[test]
    fn parse_table_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.sqlite3");
        let conn = Connection::open(&path).unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)", ()).unwrap();
        conn.execute("INSERT INTO test VALUES (42, 'tjena tjena')", ()).unwrap();
        conn.close().unwrap();
        let result = open(&path).unwrap();

        assert_eq!(result.pages.len(), 2); // root page + 1st page with table content

        match result.pages.last().unwrap() {
            Page::LeafTablePage(p) => {
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
                    vec![
                        None,
                        Some(Payload::Text("tjena tjena".to_string())),
                    ]
                );
            }
            _ => unreachable!("root page should be table leaf page")
        }
    }
}