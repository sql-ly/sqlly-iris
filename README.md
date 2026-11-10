# sqlly-iris

An independent, asynchronous Rust driver for InterSystems IRIS, paired with tools for comparing native protocol behavior against the vendor ODBC driver. The crate connects directly to the IRIS superserver port and does not require an ODBC installation or vendor client library.

## Status

The repository contains a working core driver, not a completed Tiberius-equivalent release. Protocol 69 connections, direct queries, prepare/execute updates, positional parameters, streamed result fetching, transactions, server errors, and the SQL types listed in [the compatibility matrix](docs/compatibility.md) pass live tests against IRIS 2026.1. TLS, reusable prepared statements, multiple result sets, dedicated stored-procedure calls, bulk operations, incremental stream writes, catalog helpers, and non-SQL IRIS APIs are still on [the roadmap](docs/roadmap.md).

The driver refuses `Encryption::Required` rather than silently downgrading to plaintext. Password authentication currently uses IRIS's reversible credential obfuscation, so use it only on a trusted local test network until TLS lands.

## Quick start

```toml
[dependencies]
sqlly-iris = { git = "https://github.com/sql-ly/sqlly-iris" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use sqlly_iris::{Client, Config, Result, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let mut config = Config::from_url("iris://_SYSTEM:password@127.0.0.1:1972/USER")?;
    config.max_rows(1_000);

    let mut client = Client::connect(config).await?;
    println!("{}", client.server_info().product_info);

    let rows = client
        .query("SELECT ID, Name FROM Sample.Person WHERE ID > ? ORDER BY ID", &[Value::I32(0)])
        .await?
        .collect()
        .await?;
    for row in rows {
        println!("{:?}", row.values());
    }

    let mut transaction = client.begin().await?;
    transaction
        .execute("INSERT INTO Sample.Person (Name) VALUES (?)", &[Value::from("Ada")])
        .await?;
    transaction.commit().await?;

    client.close().await
}
```

`?` placeholders are rewritten only outside strings, quoted identifiers, and comments. `QueryStream::next_row` exposes incremental consumption; call `close` to drain a stream you do not finish. Dropping an unfinished stream or transaction deliberately makes the connection unusable, preventing later commands from reading stale protocol frames or running inside an ambiguous transaction.

Parameters above 65,534 encoded bytes return an explicit error until stream-parameter writes are implemented. `max_rows` is IRIS's total result limit, not the network fetch size; IRIS chooses fetch buffers and the driver follows them with `FD` requests.

## Research tools

- `iris-probe` connects with the Rust driver and runs one query or update.
- `iris-tap` is a localhost TCP relay that records both directions without packet-capture privileges.
- `iris-dissect` reconstructs frames from a tap file and prints counts, statement ids, status codes, hexadecimal payloads, and optional typed list items with `--items`.
- `tools/odbc-capabilities.c` enumerates vendor ODBC function support, connection information, and the complete advertised SQL type table.

```bash
export IRIS_DSN='iris://_SYSTEM:password@127.0.0.1:1972/USER'
cargo run --bin iris-probe -- --sql 'SELECT $namespace, $username, $ZVERSION'
cargo run --bin iris-probe -- --exec --sql 'CREATE TABLE Example (ID INTEGER)'
```

See [the reverse-engineering guide](docs/reverse-engineering.md) for vendor-driver acquisition, capture instructions, ODBC probe usage, and evidence-handling rules. Captures and downloaded vendor binaries stay in the ignored `.research/` directory and are never committed.

## Verification

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
IRIS_DSN='iris://_SYSTEM:password@127.0.0.1:1972/USER' cargo test --test live -- --ignored
```

The live test creates and drops its own table and must only target a disposable instance. Ordinary tests do not need IRIS.

## License and attribution

Apache-2.0. See [NOTICE](NOTICE) for the `go-irisnative` attribution and reviewed upstream commit. InterSystems and InterSystems IRIS are trademarks of InterSystems Corporation; this is an independent interoperability project.
