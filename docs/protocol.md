# Native protocol notes

These notes describe independently observed behavior and the reviewed Apache-2.0 `go-irisnative` implementation at commit `97ff78dcce8a0a3fcb8cf7a677bc1d8b3504e3e2`. They are not vendor documentation. Fields marked unverified must not be implemented solely from this catalog.

## Frame envelope

Every observed frame has a 14-byte header followed by a payload:

| Offset | Width | Meaning |
|---|---|---|
| 0 | 4 | Payload length, unsigned little-endian. The header is excluded. |
| 4 | 4 | Message count, unsigned little-endian. Responses observed so far repeat the request count. |
| 8 | 4 | Statement id, unsigned little-endian. Prepared statement ids begin at one in ODBC captures. |
| 12 | 2 | Operation code on requests; SQL status on responses. |

The Rust reader uses `read_exact` for the header and body and rejects bodies over 256 MiB before allocation. SQL status zero indicates success and 100 indicates end of data. Other statuses are retrieved with `OE` and returned as structured server errors.

The handshake is operation `HS` with a two-byte little-endian protocol version, 69 in the baseline. The response carries the server protocol version and Unicode flag as raw little-endian 16-bit values, followed by a list-encoded locale string. The test server returned version 69, flag zero, and locale `Unicode`; Unicode values still arrived as UTF-16LE list items, so the flag must not be used as the sole text decoder.

## List items

Most payloads consist of typed values called list items here. A null is the single byte `01`. A short item starts with one byte equal to payload data length plus two, then a type tag, then data. An extended item starts with zero, a little-endian 16-bit value equal to data length plus one, a type tag, then data. The largest directly encoded data value accepted by the current writer is 65,534 bytes; larger parameters need stream operations.

Bit `0x20` on a type tag marks by-reference values and is masked out before decoding the underlying type. Confirmed base tags are:

| Tag | Meaning and encoding |
|---|---|
| `01` | Single-byte or opaque binary string. Valid UTF-8 is returned as text; other bytes can remain binary. |
| `02` | UTF-16LE string, validated including surrogate pairs. |
| `04` | Nonnegative integer, little-endian magnitude with trailing zero bytes omitted. |
| `05` | Negative integer, little-endian one's-complement magnitude with trailing bytes omitted. |
| `06` | Positive decimal: signed scale byte followed by little-endian coefficient. |
| `07` | Negative decimal using the corresponding negative coefficient. |
| `08` | Compact IEEE float; infinity uses a special two-byte form. |
| `09` | IEEE double; NaN has a special two-byte form. |
| `19` | Object reference text. |

Decimal scale is the first byte interpreted as signed two's-complement exponent. The coefficient is multiplied by ten to that exponent, with no binary float conversion. Negative coefficients are one's-complement plus one. This representation round-tripped a 19-significant-digit decimal exactly.

Compact floats omit trailing zero bytes. Reconstruction left-aligns the remaining bytes in the four-byte single-precision representation; for example, bytes `2a 42` mean little-endian `00 00 2a 42`, or 42.5. Copying them to the low end produces a tiny, incorrect value and was fixed after a live differential test.

Empty string items occasionally appear where metadata otherwise contains integers, such as `DOUBLE` scale. The vendor's effective result is zero, and the native decoder follows that behavior for integer metadata.

## Credential transformation

Password login sends namespace, transformed username, transformed password, machine user, machine name, application name, and session options. For each input byte at index `i`, with `r` equal to the number of bytes after it, the implementation computes `rotate_left_5(((byte XOR 0xa7) + r) mod 256)` and stores it at output index `r`. Non-ASCII credentials operate on UTF-8 bytes in the current implementation and still require differential coverage.

This transformation is reversible obfuscation, not encryption. Tap files, driver logs, and process memory must be treated as credential-bearing. The native client refuses a requested TLS mode until actual TLS negotiation is implemented.

## Connection session options

The default native login requests fast select, durable transactions, and output redirection, combined feature mask 37 (`1 | 4 | 32`). Setting `fast_insert` adds bit 2, producing mask 39. The captured ODBC driver requested mask 11 (`1 | 2 | 8`), which the independent implementation labels fast select, fast insert, and not-nullable. Feature masks must be decoded bit by bit, and each payload change needs its own capture.

A successful connection response contains product text, delimited-identifier behavior, an unknown integer, isolation state, server job number, empty-string behavior, and server feature options. SQLCODE 417 during connection carries its error text directly in the response payload. TLS uses an asynchronous rustls handshake with native trust roots and optional additional CA files before protocol negotiation; plaintext fallback is not attempted.

## Query flow

The native client uses `DQ` for direct queries and `PQ` for prepared queries. Its payload contains chunked SQL text, parameter descriptors, one parameter set, query timeout, and maximum result rows. SQL text is split into 31,904-byte chunks, matching the independent implementation; Unicode-safe chunking and larger parameter streams need more coverage.

Placeholders become `:%qpar(n)` before transmission. The rewriter tracks single quotes, double quotes, backticks, line comments, and block comments, so literal and commented question marks are untouched. ODBC captures show the vendor driver using the same host-variable naming scheme when it rewrites literals.

A successful direct query returns a metadata frame followed by a data frame. A prepared query reuses the `PP` metadata and returns data through `PQ`. Metadata contains statement features, column count, and per-column name, SQL type, precision, scale, nullability, label, table, schema, catalog, flags, and optionally a fast-select slot. Parameter count and flags follow the columns. Column flags observed include auto-increment, case sensitivity, currency, read-only, and row id.

Fast-select data packs each row inside another list item and uses one-based slot positions in metadata. The implementation validates every slot and nested item boundary. A procedure statement marker of -1 is not treated as fast-select, even though its low bit is set.

When the current buffer is exhausted and status is not 100, the client sends `FD` with the active statement id. ODBC captures confirm this statement id is required; sending zero caused SQLCODE -400 with an undefined statement-cache reference. `max_rows` limits total returned rows and does not control the continuation buffer size.

## Update flow

ODBC prepares with `PP` and executes with `PU`, using the same nonzero statement id. The prepare payload contains SQL and parameter descriptors. Its response contains statement features, parameter metadata, and cache flags, not an affected-row count. The `PU` payload contains an empty string, zero, parameter-set count, parameter count, and values; its response begins with the affected-row count.

An early attempt to treat the prepare response as if it also contained the affected-row count failed on the three zero-valued items returned for DDL. Sending `PU` after an inappropriate direct-update preparation produced SQLCODE 404, `Reprepare statement - INTERNAL ONLY, DO NOT DOCUMENT`. The final client follows the observed `PP`/`PU` sequence and passes DDL and parameterized inserts.

ODBC sends `CU` to close prepared statements, and there is no completion frame. Explicit statement close writes `CU` immediately; dropping a statement queues `CU` for the next operation. Delayed responses are correlated by message count rather than discarded.

Fast parameter arrays are opt-in. The execution starts with a raw little-endian 32-bit batch count, followed by key-request text, zero, key count, and per-row empty delimiters and packed list-item strings. The metadata provides identity/default values and target table information. Single-row statements use the standard update path.

## Procedure and session flow

Direct procedure calls use `DS` with a procedure name and arguments. A response with result kind -1 enters `MS` for the first result and `MR` for subsequent results. Scalar procedures return `_isc_sp_ret_val` directly during `DS` and do not enter `MS`. Result frames may contain a return value, metadata, and rows; `MD` drains continuation frames until status 100. The end-of-results marker is -2.

Autocommit and isolation setters use ordered writes without reads for `AF`/`AN` and `RC`/`RU`. `TC`/`TR` complete transactions, and `TS` returns a real success or error response. Delayed stream-write acknowledgments are tracked by message count and checked rather than treated as a later query response.

## Stream references and time

`LONGVARCHAR` cells can contain a composite list-encoded stream reference rather than inline text. `JS` with that reference and a positive requested length returns a prefix, while -1 returns the entire stream again; it is stateless rather than a cursor. `SS` returns byte length. `SZ` sends handle, zero-based position, byte count, then raw bytes. `SX` sends handle and truncation length, and `CS` closes the handle. Their delayed acknowledgments are correlated and checked, while failures make the session unusable.

IRIS metadata can report type 1093 for timestamp data. For a positive encoded integer, XOR with `0x1000000000000000` yields signed Unix microseconds. For a nonpositive encoded integer, OR with `0x6000000000000000` performs the corresponding legacy-range decode. Seconds use Euclidean division by one million and the remaining microseconds are nonnegative. The live value decoded to `2026-11-09T13:14:15.123456Z` and round-tripped as a parameter.

Native operations also use success/result markers rather than ordinary SQL statuses: `0xCA00` is native success, `0xCDF1` represents an undefined global, and `0xCE7B` is a method-status result. These are handled only in the native operation path, not by weakening ordinary SQL error handling.

## Operation catalog

Observed means seen in this repository's live captures. Listed means present in the independent Go source but not behaviorally verified here.

| Code | Operation | Evidence |
|---|---|---|
| `HS` | Handshake | Observed ODBC and native |
| `CN` | Connect | Observed ODBC and native |
| `DC` | Disconnect | Observed ODBC and native |
| `PP` | Prepare | Observed ODBC and native |
| `PU` | Prepared update execute | Observed ODBC and native |
| `PQ` | Prepared query execute | Observed ODBC and native |
| `DQ` | Direct query | Observed native |
| `DU` | Direct update | Observed native experiment; final updates use `PP`/`PU` |
| `FD` | Fetch data | Observed ODBC and native |
| `OE` | Get server error | Observed native |
| `TC` | Commit | Observed native |
| `TR` | Rollback | Observed native |
| `JS` | Read stream | Observed native |
| `CU` | Close statement | Observed ODBC and native |
| `GG` | Get auto-generated keys | Observed native, unreliable |
| `DS` | Direct stored procedure | Observed ODBC and native |
| `SP` | Prepare stored procedure | Listed |
| `SU` | Stored-procedure update execute | Listed |
| `SQ` | Stored-procedure query execute | Listed |
| `SF` | Stored-procedure fetch data | Listed |
| `MR` | Get more results | Observed ODBC and native |
| `MD` | Multiple-result-set fetch data | Observed ODBC and native |
| `RC` | Read committed | Observed ODBC and native |
| `RU` | Read uncommitted | Observed ODBC and native |
| `IL` | Isolation level | Observed native; no response on protocol 69 |
| `AF` | Autocommit off | Observed ODBC and native |
| `AN` | Autocommit on | Observed native |
| `TS` | Toggle synchronous commit | Observed native, including status 409 |
| `SS` | Get stream size | Observed native |
| `RS` | Retrieve stream | Listed |
| `OS` | Open stream | Listed |
| `AS` | Read stream in ODBC form | Listed |
| `SB` | Store binary stream | Listed |
| `SM` | Store character stream | Listed |
| `GB` | Stream get bytes | Listed |
| `SZ` | Stream set bytes | Observed native |
| `SX` | Stream truncate | Observed native |
| `GP` | Stream get position | Observed native; no response on protocol 69 |
| `CS` | Close stream | Observed native |
| `41 c2` | Global get | Observed native |
| `42 c2` | Global set | Observed native |
| `43 c2` | Global kill | Observed native |
| `45 c2` | Global order | Observed native |
| `49 c2` | Global data | Observed native |
| `4b c2` | Class method returning value | Observed native |
| `4c c2` | Void class method | Listed |
| `5b c2` | Method returning value | Observed native |
| `5c c2` | Void method | Listed |
| `5d c2` | Property get | Observed native |
| `5e c2` | Property set | Observed native |

Feature flags seen in the independent implementation include fast select 1, fast insert 2, durable transactions 4, not-nullable 8, and redirect output 32. Their exact server negotiation and payload changes require per-feature captures.

## Open protocol questions

The remaining unknowns are successful IRIS-over-TLS configurations, Kerberos and access-token login, protocol-version downgrade behavior, prepared procedures and output parameters, reliable generated keys, fast-insert partial-failure details, additional stream lifecycle operations, cancellation, server redirects, and non-SQL APIs such as XEP, documents, and vectors. These are blocking research items for full support, not details to paper over with aliases to SQL strings.
