# Roadmap to Tiberius-level IRIS support

The goal is a production-quality native Rust driver, not an ODBC wrapper. A feature is complete only when its public API, protocol behavior, error paths, cancellation behavior, offline regressions, live tests, documentation, and supported-platform CI all exist.

## Phase 1: Harden the working SQL core

- Add reusable prepared query and update statements with explicit close, cache eviction, and reprepare handling.
- Implement `futures_core::Stream` for results without losing the current desynchronization safeguards.
- Add client-initiated cancellation, server query timeout tests, and deterministic behavior when a future is dropped mid-request.
- Support null, empty string, maximum-length identifiers, large SQL chunks, extreme numerics, NaN, infinities, all date/time variants, GUID, OREF, and long binary streams.
- Capture protocol downgrade behavior and define the oldest supported IRIS release from evidence.
- Add statement and connection option types rather than exposing raw feature masks.

## Phase 2: Complete relational protocol behavior

- Implement multiple result sets through `MR` and `MD`, including row counts, errors between results, and empty results.
- Implement direct and prepared stored procedures, input/output parameters, return values, procedure result sets, and `GG` identity retrieval.
- Implement autocommit, read committed, read uncommitted, synchronous commit, and isolation state transitions. Refuse unsupported isolation modes explicitly.
- Add parameter arrays, execute-many, fast insert, defaults, identity columns, and bulk-loading APIs with atomicity and partial-failure tests.
- Finish stream size, open, chunked read, ODBC-form read, binary/character write, position, truncate, and close operations.

## Phase 3: Secure deployment

- Implement TLS negotiation and certificate verification with rustls first, then evaluate native TLS parity with Tiberius. Never silently downgrade.
- Validate custom CA, hostname verification, mutual TLS if the server supports it, and insecure development modes with explicit naming.
- Implement Kerberos authentication and packet integrity/encryption levels, then OAuth access-token login and token refresh ownership rules.
- Add secret-redacted structured logging and capture sanitization for credentials, tokens, SQL values, and connection strings.
- Run security review and fuzz every frame and list decoder with arbitrary, nested, oversized, and truncated input.

## Phase 4: Schema and application APIs

- Add catalog, type, column, key, index, procedure, privilege, and special-column helpers validated against their ODBC counterparts.
- Implement globals, class methods, object methods, and properties with a clearly separate native API rather than disguising them as SQL.
- Determine whether XEP, document access, vectors, sharding, mirroring, and other IRIS capabilities use additional protocols or server APIs; implement or explicitly scope each one.
- Add chrono, time, rust_decimal, bigdecimal, UUID, and serde feature flags without losing exact values or making a specific date/time crate mandatory.

## Phase 5: Tiberius-grade engineering and compatibility

- Abstract asynchronous reads and writes so Tokio is not embedded in protocol state machines; add supported runtime adapters and transport choices.
- Match Tiberius-style query, execute, transaction, bulk, configuration, authentication, encryption, and error ergonomics where they fit IRIS.
- Run unit and live matrices on macOS ARM64, Linux x86_64 and ARM64, and Windows x86_64 and ARM64, with supported IRIS versions and Unicode/non-Unicode locales.
- Add cross-driver differential jobs for the vendor ODBC versions in the support matrix, plus stress, soak, cancellation, disconnect, failover, and memory-bounds tests.
- Publish versioned crates only after documentation, compatibility tables, semantic-versioning policy, security reporting, and release automation are in place.

## Current acceptance bar

Do not label the crate feature-complete while any phase-1 through phase-4 capability is represented only by an opcode, exported symbol, `SQLGetFunctions` response, or SQL string workaround. The roadmap tracks full native behavior, and unsupported operations must return precise errors until their implementations and tests exist.
