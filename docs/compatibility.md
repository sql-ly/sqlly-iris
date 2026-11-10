# Compatibility

## Evidence baseline

The initial live baseline is IRIS for UNIX ARM64 Containers 2026.1 build 234U, native xDBC protocol 69, running in Docker on Apple Silicon. Differential ODBC tests use the InterSystems macOS ARM64 driver 2026.2.0.221.0 with unixODBC 2.3.14. The driver reports ODBC 03.51 and the server reports `InterSystems IRIS Version 2026.1.0.234 xDBC Protocol Version 69`.

“Tested” below means exercised against that live server by `tests/live.rs` or a recorded ODBC experiment. It does not imply coverage of older protocol versions or every IRIS edition.

## Native Rust driver

| Area | State | Evidence or limit |
|---|---|---|
| TCP connection and protocol 69 handshake | Tested | Product, locale, job, feature flags, and server version decoded. |
| Password login | Tested | `_SYSTEM` against a disposable local server. Credential obfuscation is not transport security. |
| TLS | Not implemented | `Encryption::Required` returns an error without downgrading. |
| Direct SQL queries | Tested | Metadata, rows, empty results, SQL errors, and repeated queries. |
| Updates and DDL | Tested | `PP` preparation followed by `PU` execution and affected-row counts. |
| Positional parameters | Tested | `?` outside quotes and comments; count mismatches fail before network I/O. |
| Incremental result fetching | Tested | A 100-row, roughly 100 KB result forces `FD` frames with the active statement id. |
| Result limit | Observed | `max_rows=2` returned exactly two rows; this is a total limit, not a fetch size. |
| Transactions | Tested | Rollback removes an insert and commit preserves another. |
| Server diagnostics | Tested | SQLCODE and IRIS message retrieved with `OE`; later queries still work. |
| Null, boolean, signed integers | Tested | Includes `i64::MIN` codec coverage and `i64::MAX` server round-trip. |
| Exact decimal text | Tested | `1234567890.123456789` round-trips without a float conversion. |
| IEEE floats | Tested | Compact float zero trimming and `DOUBLE` value 42.5. |
| UTF-8 and UTF-16LE strings | Tested | ` café 東京 ` round-trips as a Unicode list item. |
| Binary values | Tested | `VARBINARY` round-trip includes zero, high-bit, and `0xff` bytes. |
| Character streams | Tested | `LONGVARCHAR` stream reference resolved with `JS`. |
| Binary streams | Partially tested | `VARBINARY` works; incremental `LONGVARBINARY` read, write, truncate, and seek remain. |
| TIMESTAMP and POSIX time | Tested | `2026-11-09 13:14:15.123456` round-trips as Unix seconds 1,794,230,055 and 123,456 microseconds. |
| Oversized parameters | Guarded | Values over 65,534 encoded bytes return an explicit unsupported error. |
| Unfinished streams and transactions | Guarded | The connection becomes unusable instead of risking protocol desynchronization or ambiguous writes. |
| Reusable prepared statements | Not implemented | Each update currently prepares and executes once; statement close and caching remain. |
| Multiple result sets | Not implemented | `MR` and `MD` are cataloged but not decoded. |
| Stored procedures | Not implemented | `DS`, `SP`, `SU`, `SQ`, and `SF` are cataloged but not implemented. |
| Array and bulk execution | Not implemented | ODBC advertises parameter-array support; native fast-insert encoding remains unverified. |
| Query cancellation | Not implemented | Server query timeout is sent; client-initiated cancellation remains. |
| Transaction isolation controls | Not implemented | Opcodes are cataloged; live isolation behavior remains to be verified. |
| Catalog and type helpers | Not implemented | The ODBC probe enumerates behavior to implement. |
| Globals, objects, class methods, properties | Not implemented | Opcodes from the independent Go implementation are cataloged, not claimed as supported. |
| Kerberos and OAuth access tokens | Not implemented | Documented ODBC connection options exist; no native handshake has been validated. |
| Runtime-independent I/O | Not implemented | The initial client uses Tokio `TcpStream`. |
| chrono, time, decimal, UUID, serde integrations | Not implemented | Public values currently preserve timestamps and decimals without converting through floats. |
| XEP, document access, vectors, and other non-ODBC APIs | Not implemented | These require separate protocol and API research, not SQL opcode aliases. |

## IRIS numeric limit confirmed through both clients

IRIS accepted `DECIMAL(38,9)` in DDL, but both this driver and vendor ODBC rejected `12345678901234567890.123456789` during insertion with SQLCODE -104. Both accepted and returned `1234567890.123456789` exactly. This matches InterSystems' documentation that standard decimal values have about 19 significant digits, so declared precision must not be mistaken for unlimited 38-digit storage.

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

Full compatibility requires versioned captures for supported IRIS releases, Unicode and non-Unicode locales, Windows and Linux ODBC drivers, TLS configurations, Kerberos and OAuth, stored procedures with input/output parameters, all catalog calls, multiple result sets, fast insert and parameter arrays, long-stream edge cases, server cancellation, and cross-platform CI. No version before 2026.1 has been validated by this repository yet.
