# sqlite-parser
SQLite database format parser.

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
