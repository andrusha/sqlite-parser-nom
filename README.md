# sqlite-parser-nom

SQLite binary database format parser.

Homonym libraries:

- Unlike [sqlite_parser](https://crates.io/crates/sqlite_parser), which is a front-end
  to [rusqlite](https://crates.io/crates/rusqlite), actually parses the file and gives access to binary content as-is.
- [sqlite3-parser](https://crates.io/crates/sqlite3-parser) is parser + lexer for SQLite3-compatible SQL grammar.

## Usage

In your Cargo.toml:

```toml
[dependencies]
sqlite-parser = "0.1.0"
```

Load and parse file in memory:

```rust
use sqlite_parser;

fn main() -> Result<()> {
    let database = sqlite_parser::open("database.sqlite3")?;
    println!("{}", database.pages.len());

    Ok(())
}
```
