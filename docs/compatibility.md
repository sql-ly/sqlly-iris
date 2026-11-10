# Compatibility

## Evidence baseline

The live baseline is IRIS for UNIX ARM64 Containers 2026.1 build 234U, native xDBC protocol 69, running in Docker on Apple Silicon. Differential ODBC tests use the InterSystems macOS ARM64 driver 2026.2.0.221.0 with unixODBC 2.3.14. The vendor driver reports ODBC 03.51, and the server reports `InterSystems IRIS Version 2026.1.0.234 xDBC Protocol Version 69`.

“Tested” means exercised against that live server by a Rust integration test or a recorded ODBC experiment. It does not imply coverage of older protocol versions or every IRIS edition. TLS is implemented, but a successful IRIS-over-TLS handshake has not yet been tested with a configured TLS server.

## Native Rust driver

| Area | State | Evidence or limit |
|---|---|---|
| TCP connection and protocol 69 handshake | Tested | Product, locale, job, feature flags, and server version decoded. |
| Password login | Tested | `_SYSTEM` against a disposable local server. Credential obfuscation is not transport security. |
| TLS with native roots and optional additional CA | Implemented | `Encryption::Required` performs an async rustls handshake with certificate and hostname verification. Plaintext refusal is tested; a successful IRIS TLS server handshake remains to be tested. |
| Direct SQL queries | Tested | Metadata, rows, empty results, SQL errors, and repeated queries. |
| Reusable prepared statements | Tested | `PP`, `PQ`/`PU`, parameter-count checks, repeated execution, explicit `CU`, and deferred `CU` before the next operation. |
| `futures_core::Stream` | Tested | Direct and prepared query results expose `next_row`, `collect`, and an async stream adapter. |
| Positional parameters | Tested | `?` outside quotes and comments; count mismatches fail before network I/O. |
| Incremental result fetching | Tested | Large results force `FD` frames with the active statement id. |
| Result limit | Observed | `max_rows=2` returned exactly two rows; this is a total limit, not a fetch size. |
| Transactions | Tested | Rollback removes an insert and commit preserves another. |
| Autocommit and isolation | Tested | `AF`/`AN`, `RC`/`RU`, and native `TC`/`TR` work. Synchronous commit returns a clean server error when refused with status 409. |
| Stored procedures | Tested | `DS`/`MS` direct calls, scalar `_isc_sp_ret_val` returns, input parameters, `MR` result sequences, and `MD` continuation. Prepared procedures and output parameters remain. |
| Multiple result sets | Tested | Two result sequences with 100 rows per result, return values, and cleanup after abandoned calls. |
| Parameter arrays and fast insert | Tested | Opt-in `fast_insert` matches the vendor's raw batch count, identity/default metadata, row delimiters, and packed values. Ordinary statements stay on the standard path. |
| Generated keys | Not reliable | Experimental `GG` and `LAST_IDENTITY` behavior is not exposed as a supported feature. |
| Server diagnostics | Tested | SQLCODE and IRIS messages are retrieved with `OE`; later queries still work. |
| Null, boolean, signed integers | Tested | Includes `i64::MIN` codec coverage and `i64::MAX` server round-trip. |
| Exact decimal text | Tested | `1234567890.123456789` round-trips without a float conversion. |
| IEEE floats | Tested | Compact float zero trimming and `DOUBLE` value 42.5. |
| UTF-8 and UTF-16LE strings | Tested | ` café 東京 ` round-trips as a Unicode list item. |
| Binary values | Tested | `VARBINARY` round-trip includes zero, high-bit, and `0xff` bytes. |
| Character and binary streams | Tested | Handle-owning and eager queries, stream size, prefix/full reads, truncation, one-based writes, and close. `LONGVARBINARY` uses the same handle path. |
| TIMESTAMP and POSIX time | Tested | `2026-11-09 13:14:15.123456` round-trips as Unix seconds 1,794,230,055 and 123,456 microseconds. |
| Globals | Tested | Get, set, kill, data, forward and previous order, children, and `<UNDEFINED>` mapped to `Value::Null`. |
| Objects and OREFs | Tested | Class methods, instance methods, and properties using native OREF values without the outgoing by-reference bit. |
| Catalogs | Tested | Async schema, table, column, and procedure metadata from `INFORMATION_SCHEMA`. |
| Oversized parameters | Guarded | Values over 65,534 encoded bytes return an explicit unsupported error. |
| Unfinished streams and transactions | Guarded | The connection becomes unusable instead of risking protocol desynchronization or ambiguous writes. |
| chrono and time conversions | Tested | Optional feature-gated conversions for `chrono::NaiveDateTime`, `chrono::DateTime<Utc>`, `time::PrimitiveDateTime`, and `time::OffsetDateTime`. |
| Decimal and UUID conversions | Tested | Optional feature-gated `rust_decimal::Decimal`, `bigdecimal::BigDecimal`, and `uuid::Uuid` conversions. |
| Serde | Implemented | Optional `Serialize` and `Deserialize` derives on `Value`. |
| Client-initiated cancellation | Not implemented | Server query timeout is sent; cancellation and future-drop behavior still need work. |
| Kerberos and OAuth access tokens | Not implemented | Documented ODBC connection options exist; no native handshake has been validated. |
| Runtime-independent I/O | Not implemented | The client uses Tokio and tokio-rustls behind an async stream trait object. |
| XEP, document access, vectors, and other non-ODBC APIs | Not implemented | These require separate protocol and API research, not SQL opcode aliases. |
| Older IRIS versions | Not verified | Only protocol 69 on IRIS 2026.1 has been exercised. |

## IRIS numeric limit confirmed through both clients

IRIS accepted `DECIMAL(38,9)` in DDL, but both this driver and vendor ODBC rejected `12345678901234567890.123456789` during insertion with SQLCODE -104. Both accepted and returned `1234567890.123456789` exactly. This matches InterSystems documentation that standard decimal values have about 19 significant digits, so declared precision must not be mistaken for unlimited 38-digit storage.

## Vendor ODBC function surface

The capability probe queried every function individually. The 2026.2 macOS driver answered “yes” for all 65 probed ODBC entry points, including parameter arrays, catalog functions, scrollable fetch APIs, `SQLMoreResults`, and transaction functions. This is advertised support, not behavioral verification of every call.

The full probe covers allocation, binding, cancellation, cursors, column and table privileges, columns, primary and foreign keys, procedures and procedure columns, statistics, special columns, type information, diagnostics, statement and connection attributes, prepared execution, parameter data, and transactions. Reproduce the raw list with `tools/odbc-capabilities.c` rather than treating exported symbols as equivalent evidence.

## Advertised connection capabilities

| ODBC information value | Vendor result |
|---|---|
| Driver ODBC version | 03.51 |
| Identifier quote | `"` |
| ODBC API conformance | 2 |
| ODBC SQL conformance | 1 |
| SQL conformance | 1 |
| Transaction capable | 2 |
| Transaction isolation options | 3 |
| Default transaction isolation | 1 |
| Scroll options | 1 |
| Forward-only cursor attributes 1 / 2 | 1 / 6016 |
| Dynamic cursor attributes 1 / 2 | 0 / 0 |
| Keyset cursor attributes 1 / 2 | 0 / 0 |
| Parameter-array row counts | 2 |
| Parameter-array selects | 3 |
| Batch support / row count | 2 / 2 |
| Multiple result sets | Y |
| Multiple active transactions | Y |
| Outer joins | Y |
| Maximum identifier, table, column, schema length | 128 |
| Maximum character literal length | 1500 |
| Values reported as zero for maximum statement, row, binary literal, and several column counts | 0, meaning no fixed limit in this context |

The transaction, cursor, batch, and limit values are raw ODBC values. Their bit meanings and actual behavior must be tested before exposing equivalent Rust APIs.

## Advertised type-information rows

`SQLGetTypeInfo(SQL_ALL_TYPES)` returned GUID, BIT, TINYINT, BIGINT, LONGVARBINARY, VARBINARY, LONGVARCHAR, OREF, NUMERIC, INTEGER, SMALLINT, DOUBLE, VARCHAR, DATE, TIME, and TIMESTAMP. LONGVARBINARY and LONGVARCHAR report a column size of 2,147,483,647; VARBINARY and VARCHAR report 4096; GUID reports 36; BIGINT reports 19; NUMERIC reports precision 15 and maximum scale 15 by default; TIMESTAMP reports size 23.

Absence of a separate row does not prove that similarly named DDL is rejected. For example, the live server accepted `DECIMAL(38,9)` even though the driver's type-information result has a NUMERIC row rather than a separate DECIMAL row.

## Remaining coverage requirements

Full compatibility requires versioned captures for supported IRIS releases, Unicode and non-Unicode locales, Windows and Linux ODBC drivers, successful TLS configurations, Kerberos and OAuth, prepared procedures with input/output parameters, reliable generated keys, additional catalog calls, fast-insert partial-failure handling, more stream edge cases, cancellation, and cross-platform live CI. The offline test suite currently runs on macOS, Ubuntu, and Windows, while the live database suite has run on Apple Silicon.
