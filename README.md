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

### Principal structure

```
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