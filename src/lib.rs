#![doc(
    issue_tracker_base_url = "https://github.com/mycelial/sqlite-parser-nom/issues",
    test(no_crate_inject)
)]
#![doc = include_str ! ("../README.md")]

extern crate core;

use memmap2::{Mmap, MmapOptions};
use std::fs::File;
use std::path::Path;

use nom::Finish;

use crate::error::{OwnedBytes, SQLiteError};
use crate::model::{DbHeader, Page};
use crate::parser::{db_header, page, HEADER_SIZE};

mod be_i48;
pub mod error;
pub mod model;
pub mod parser;
mod varint;

/*
todo: parse additional page types (overflow, lock, freelist, ?)
todo: determine when overflow page no is used
todo: how page size computation works?
todo: test with more records
todo: how freelist pages work?
*/

// todo: use bufreader
pub struct Reader<S: AsRef<[u8]>> {
    buf: S,
    pub header: DbHeader,
}

impl Reader<Mmap> {
    /// Open a SQLite database file by memory mapping it.
    ///
    /// # Example
    ///
    /// ```
    /// let reader = sqlite_parser_nom::Reader::open_mmap("sample/sakila.db").unwrap();
    /// ```
    pub fn open_mmap<P: AsRef<Path>>(database: P) -> Result<Reader<Mmap>, SQLiteError> {
        let file_read = File::open(database)?;
        let mmap = unsafe { MmapOptions::new().map(&file_read) }?;
        Reader::from_source(mmap)
    }
}

impl Reader<Vec<u8>> {
    /// Open a SQLite database file by loading it into memory.
    /// Payloads are not copied until use, but all the metadata must be.
    ///
    /// # Example
    ///
    /// ```
    /// let reader = sqlite_parser_nom::Reader::open_readfile("sample/sakila.db").unwrap();
    /// ```
    pub fn open_readfile<P: AsRef<Path>>(database: P) -> Result<Reader<Vec<u8>>, SQLiteError> {
        use std::fs;

        let buf: Vec<u8> = fs::read(&database)?;
        Reader::from_source(buf)
    }
}

impl<S: AsRef<[u8]>> Reader<S> {
    /// Open a SQLite database from anything that implements AsRef<[u8]>
    ///
    /// # Example
    ///
    /// ```
    /// use std::fs;
    /// let buf = fs::read("sample/sakila.db").unwrap();
    /// let reader = sqlite_parser_nom::Reader::from_source(buf).unwrap();
    /// ```
    pub fn from_source(buf: S) -> Result<Reader<S>, SQLiteError> {
        let (_, header) = db_header(buf.as_ref())
            .finish()
            .map_err(|e| nom::error::Error {
                code: e.code,
                input: OwnedBytes(e.input.to_owned()),
            })?;

        let reader = Reader { buf, header };

        Ok(reader)
    }

    pub fn get_page(&self, pageno: u32) -> Result<Page, SQLiteError> {
        let page_size = self.header.page_size.real_size();
        let pageno = pageno as usize;

        // root page needs to be offsetted for header size
        if pageno == 0 {
            let page_bytes =
                &self.buf.as_ref()[page_size * pageno + HEADER_SIZE..page_size * (pageno + 1)];
            let (_, page) =
                page::<HEADER_SIZE>(page_bytes)
                    .finish()
                    .map_err(|e| nom::error::Error {
                        code: e.code,
                        input: OwnedBytes(e.input.to_owned()),
                    })?;
            Ok(page)
        } else {
            let page_bytes = &self.buf.as_ref()[page_size * pageno..page_size * (pageno + 1)];
            let (_, page) = page::<0>(page_bytes)
                .finish()
                .map_err(|e| nom::error::Error {
                    code: e.code,
                    input: OwnedBytes(e.input.to_owned()),
                })?;
            Ok(page)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::Page;
    use crate::model::SerialType::{Null, Text, I8};
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
        let reader = Reader::open_readfile(&path).unwrap();

        assert_eq!(reader.header.page_size.real_size(), 4096);

        match reader.get_page(0).unwrap() {
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
                        Some("table".into()),
                        Some("test".into()),
                        Some("test".into()),
                        Some(2i8.into()),
                        Some(
                            "CREATE TABLE test (id INTEGER PRIMARY KEY, foo TEXT NOT NULL)".into()
                        ),
                    ]
                );
            }
            _ => unreachable!("root page should be table leaf page"),
        }

        match reader.get_page(1).unwrap() {
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

        let reader = Reader::open_mmap(&path).unwrap();

        match reader.get_page(1).unwrap() {
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
                    vec![None, Some("tjena tjena".into())]
                );
            }
            _ => unreachable!("root page should be table leaf page"),
        }
    }
}
