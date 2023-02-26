# sqlite-parser-nom

SQLite binary database format parser.

Homonym libraries:

- [sqlite_parser](https://crates.io/crates/sqlite_parser) is a front-end
  to [rusqlite](https://crates.io/crates/rusqlite) and doesn't actually do parsing.
- [sqlite3-parser](https://crates.io/crates/sqlite3-parser) is parser + lexer for SQLite3-compatible SQL grammar.

## Usage

In your Cargo.toml:

```toml
[dependencies]
sqlite-parser-nom = "0.1.0"
```

Load and parse file in memory:

```rust
use sqlite_parser_nom;

fn main() -> Result<()> {
    let database = sqlite_parser_nom::open("database.sqlite3")?;
    println!("{}", database.pages.len());

    Ok(())
}
```

## SQLite format specification

References:

- [Database File Format](https://www.sqlite.org/fileformat.html) - official file format guide
- [Requirements for the SQLite Database File Format
  ](http://www.sqlite.org/draft/hlr30000.html) - detailed list of assumptions
- [The Definitive Guide to SQLite](https://link.springer.com/book/10.1007/978-1-4302-3226-1) - Chapter 11,
  high level overview
- [Mobile Forensics – The File Format Handbook](https://link.springer.com/book/10.1007/978-3-030-98467-0) - detailed
  description until the cell contents

### Physical structure

#### Database file

```text
+---+-------+-----------+-----------+-----------+
| h |       |           |           |           |
| e |       |           |           |           |
| a | root  |   page 2  |    ...    |   page N  |
| d | page  |           |           |           |
| e |       |           |           |           |
| r |       |           |           |           |
+---+-------+-----------+-----------+-----------+
            ^           ^           ^
< page size | page size | page size | page size >
```

- The SQLite database file is divided into equally-sized pages
    - Page size is defined within the header
    - Root page includes file header, but together still fits page size
    - All pages, including root page, count internal offsets from the beginning of the page itself
    - Pages are referenced by the number, therefore their position in the binary file can be computed

#### Page

```text
+---------------------+
|     page header     |
+---------------------+
| cell pointer array  |
+---------------------+
|                     |
|  unallocated space  |
|                     |
+-------+-------------+
|cell N |free block   |
+-------+------+------+
|cell 5 |cell 4|cell 3|
+-------+----+-+------+
|   cell 2   | cell 1 |
+------------+--------+
```

Page types:

- Both Index and Table pages in their Interior and Leaf flavour have the same structure, but differ in the header and
  some extra fields
    - See [models](./src/model.rs) for exact definition and BTree section for logic
- Overflow page just has `0x00` in header and the rest is payload
- Locking page is empty page in databases > 1gb at 1,073,741,824 offset
- Pointer page exists in autovacuumed DBs and contains pointers to reorganized pages
- Free blocks are stored in free-list and are not nulled, they might contain data, which was supposed to be removed

Page structure:
- Cell pointer array grows from the top of the page to the bottom
  - Pointers are byte offsets within the page
- Cells grow from the bottom of the page to the top

#### Cell

```text
+-----------+--------+--------+--------+-----+--------+-----------+-----+-----------+
|Payload    |        | Header | Serial |     | Serial | Data Cell |     | Data Cell |
|(Cell Size)|  ...   |  Size  | Type 1 | ... | Type N | Column 1  | ... | Column N  |
+-----------+--------+--------+--------+-----+--------+-----------+-----+-----------+
                     |                                |
<    cell header     ^      record header             ^        table row data       >
                     <                            cell size                         >
```

- This structure with some amendments applies to Table Leaf, Index Leaf and Interior pages
- Table Interior page contains pointers to other pages and corresponding rowid
- Header and auxillary values within the cell are mostly as [varint](https://sqlite.org/src4/doc/trunk/www/varint.wiki)
- Serial types correspond to the data payloads and contain size information within them
