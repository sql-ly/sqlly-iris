# sqlly-iris

An independent, asynchronous Rust driver for InterSystems IRIS, paired with tools for comparing native protocol behavior against the vendor ODBC driver. The crate connects directly to the IRIS superserver port and does not require an ODBC installation or vendor client library.

## Status

The repository contains a broad native protocol 69 driver, not a completed Tiberius-equivalent release. Connections, direct and prepared SQL, streamed rows, stored procedures and multiple results, transactions and isolation, parameter arrays, native streams, globals, objects, catalog helpers, and the SQL types in [the compatibility matrix](docs/compatibility.md) pass live tests against IRIS 2026.1. All network operations are asynchronous and use safe Rust.

TLS is implemented with `tokio-rustls` and certificate verification using native trust roots, with optional additional CA certificates. `Encryption::Disabled` is the only plaintext mode; `Encryption::Required` never downgrades or accepts an invalid certificate. Kerberos and OAuth access tokens, prepared stored procedures and output parameters, reliable generated-key retrieval, older protocol versions, runtime-independent adapters, and XEP/document/vector APIs remain on [the roadmap](docs/roadmap.md).

## Driver capabilities

- Direct queries and reusable prepared statements with explicit or deferred `CU` cleanup, `PQ`/`PU` execution, and `futures_core::Stream` adapters.
- Stored procedures with input parameters, scalar returns, multiple result sets, `MD` continuation, and async cleanup.
- Autocommit, read committed, read uncommitted, synchronous-commit toggling, and native commit/rollback.
- Opt-in fast parameter arrays with exact identity/default metadata handling and per-row result processing.
- Eager or handle-owning queries with stream size, prefix and full reads, truncation, one-based writes, and close.
- Globals, class methods, instance methods, and properties using native OREF values.
- Async schema, table, column, and procedure catalog queries.
- Optional `chrono`, `time`, `rust_decimal`, `bigdecimal`, `uuid`, and `serde` conversions, with exact decimal and timestamp values preserved.

## TLS

```rust
use sqlly_iris::{Client, Config, Encryption};

let mut config = Config::from_url("iris://_SYSTEM:password@iris.example.com:1972/USER")?;
config
    .encryption(Encryption::Required)
    .ca_certificate("/path/to/private-ca.pem");
let client = Client::connect(config).await?;
```

The native root store is used by default. An additional CA file supplements those roots; it does not replace them or disable hostname verification. A TLS-enabled server is required, and a plaintext endpoint produces a TLS error instead of a connection fallback.

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

Parameters above 65,534 encoded bytes return an explicit error until stream-parameter writes are implemented. `max_rows` is IRIS's total result limit, not the network fetch size; IRIS chooses fetch buffers and the driver follows them with `FD` requests. Fast parameter arrays require `config.fast_insert(true)` or `fast_insert=1` in the connection URL, keeping ordinary statements on the standard execution path.

Optional Rust integrations are enabled independently: `chrono`, `time`, `rust_decimal`, `bigdecimal`, `uuid`, and `serde`. Add the features your application uses rather than taking every date/time and decimal dependency.

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
