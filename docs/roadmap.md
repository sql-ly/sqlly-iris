# Roadmap to Tiberius-level IRIS support

The goal is a production-quality native Rust driver, not an ODBC wrapper. A feature is complete only when its public API, protocol behavior, error paths, cancellation behavior, offline regressions, live tests, documentation, and supported-platform CI all exist.

## Delivered foundations

- Native protocol 69 framing, bounded typed-value decoding, direct and prepared SQL, result streaming, and reusable prepared statements with explicit or deferred `CU`.
- Direct stored procedures, scalar returns, multiple result sets, and `MD` continuation.
- Autocommit, read committed, read uncommitted, synchronous-commit toggling, and native commit/rollback.
- Opt-in fast parameter arrays with vendor-compatible identity/default metadata and packed rows.
- Eager and handle-owning queries, stream size, prefix/full reads, truncation, writes, and close.
- Globals, class methods, instance methods, properties, and native OREF values.
- Async catalog helpers, `futures_core::Stream`, and feature-gated chrono, time, decimal, UUID, and serde integrations.
- Async rustls transport with native roots and optional additional CA certificates.
- Live IRIS 2026.1 integration suites plus offline macOS, Ubuntu, and Windows CI.

## Phase 1: Complete relational edge cases

- Implement prepared stored procedures, output and input-output parameters, and procedure-level row counts.
- Make generated-key retrieval reliable for single and batch inserts, or expose a precise unsupported result until the server semantics are proven.
- Add per-row fast-insert error decoding, partial-failure reporting, and retry guidance without claiming all rows succeeded.
- Implement client-initiated cancellation and deterministic behavior when a query future is dropped mid-request.
- Cover empty strings, maximum-length identifiers, large SQL chunks, NaN, infinities, all date/time variants, GUID conversions, and additional stream edge cases.
- Capture protocol downgrade behavior and define the oldest supported IRIS release from evidence.
- Add statement and connection option types rather than exposing raw feature masks.

## Phase 2: Complete secure deployment

- Validate successful IRIS-over-TLS connections, custom CA handling, hostname verification, mutual TLS if supported, and explicit insecure development modes.
- Implement Kerberos authentication and packet integrity/encryption levels, then OAuth access-token login and refresh ownership rules.
- Add secret-redacted structured logging and capture sanitization for credentials, tokens, SQL values, and connection strings.
- Fuzz frame and list decoders with arbitrary, nested, oversized, and truncated input, and add soak tests for disconnects, failover, and cancellation.

## Phase 3: Complete schema and application APIs

- Add key, index, privilege, special-column, and remaining catalog helpers validated against ODBC behavior.
- Determine whether XEP, document access, vectors, sharding, mirroring, and other IRIS capabilities use additional protocols or server APIs; implement or explicitly scope each one.
- Add stream open/retrieve/store and position operations where supported, with tests for `LONGVARBINARY` and large values.
- Add connection pooling examples without embedding a specific pool implementation in the driver.

## Phase 4: Tiberius-grade engineering and compatibility

- Abstract asynchronous reads and writes further so Tokio is not embedded in protocol state machines; add supported runtime adapters and transport choices.
- Match Tiberius-style query, execute, transaction, bulk, configuration, authentication, encryption, and error ergonomics where they fit IRIS.
- Run unit and live matrices on macOS ARM64, Linux x86_64 and ARM64, and Windows x86_64 and ARM64, with supported IRIS versions and Unicode/non-Unicode locales.
- Add cross-driver differential jobs for the vendor ODBC versions in the support matrix, plus stress, soak, cancellation, disconnect, failover, and memory-bounds tests.
- Publish versioned crates only after documentation, compatibility tables, semantic-versioning policy, security reporting, and release automation are in place.

## Current acceptance bar

Do not label the crate feature-complete while any remaining capability is represented only by an opcode, exported symbol, `SQLGetFunctions` response, or SQL string workaround. Unsupported operations must return precise errors, and successful implementation must be backed by live behavior and regression tests.
