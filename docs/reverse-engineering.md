# Reverse-engineering guide

Use interoperability evidence from systems and drivers you are authorized to run. Do not bypass licensing, authentication, encryption, or server restrictions, and do not redistribute vendor binaries or proprietary code in this repository. Commit behavioral notes and clean-room test fixtures, not disassembled vendor source.

## Evidence sources

1. [InterSystems ODBC documentation](https://docs.intersystems.com/irislatest/csp/docbook/DocBook.UI.Page.cls?KEY=BNETODBC_intro) identifies ODBC 3.5, supported driver managers, default superserver port 1972, and client library variants.
2. [ODBC connection parameters](https://docs.intersystems.com/irislatest/csp/docbook/DocBook.UI.Page.cls?KEY=BNETODBC_parms) documents password, Kerberos, TLS, OAuth access tokens, query timeout, static cursors, stream prefetch, SQL dialect, and Unicode SQL type options.
3. [InterSystems SQL features](https://docs.intersystems.com/irislatest/csp/docbook/DocBook.UI.Page.cls?KEY=GSQL_intro) documents relational, transaction, procedure, type, object-relational, and interoperability behavior.
4. [InterSystems SQL data types](https://docs.intersystems.com/irislatest/csp/docbook/DocBook.UI.Page.cls?KEY=RSQL_datatype) supplies DDL and ODBC type mappings.
5. [iris-driver-distribution](https://github.com/intersystems-community/iris-driver-distribution) is the official community index for client artifacts. The baseline used `ODBC/mac/ODBC-2026.2.0.221.0-macos.tar.gz`; the repository also carried Linux ARM64/x64, Windows x64, and AIX packages at research time.
6. [go-irisnative](https://github.com/caretdev/go-irisnative) is an Apache-2.0 independent Go implementation. Commit `97ff78dcce8a0a3fcb8cf7a677bc1d8b3504e3e2` supplied a second implementation to compare against ODBC traffic.
7. [Tiberius](https://github.com/prisma/tiberius) is the API and engineering baseline: asynchronous network I/O, direct protocol implementation, streaming rows, transactions, TLS choices, authentication options, bulk insert, type integrations, and broad platform CI. It is a target for driver quality, not evidence about IRIS behavior.

## Local IRIS instance

The baseline uses the publicly documented `intersystems/iris-community:latest-em` image. Bind research ports to localhost and choose a non-default host port to avoid colliding with an existing instance:

```bash
docker run --name sqlly-iris-research \
  --publish 127.0.0.1:21972:1972 \
  --publish 127.0.0.1:52773:52773 \
  --detach intersystems/iris-community:latest-em
```

New images require a first-login password change. Set a unique disposable password using an authorized administration route, then use that password only in the ignored `.research/` configuration or your shell environment. Do not place it in committed examples, CI logs, URLs in bug reports, or captures intended for sharing.

Run the native live test:

```bash
IRIS_DSN='iris://_SYSTEM:password@127.0.0.1:21972/USER' cargo test --test live -- --ignored --nocapture
```

## Vendor driver inspection

Download an authorized driver package into `.research/` and unpack it there. On macOS the useful first-pass tools are `file`, `otool -L`, `nm -gU`, `strings`, and `dwarfdump`; on Linux use `file`, `ldd`, `readelf`, `nm`, `objdump`, and `strings`; on Windows use `dumpbin`, Dependencies, or equivalent inspection tools. Ghidra, IDA, Binary Ninja, and radare2 can help with deeper analysis, but no disassembler is a substitute for a behavioral capture.

The macOS 2026.2 package contains ARM64 Mach-O ODBC 3.5 variants for iODBC, unixODBC, Unicode, and 64-bit `SQLULEN`. `libirisodbcur6435.so` exports the standard ODBC calls plus native messenger functions for handshake, secure connect, send, receive, locale conversion, statement ids, protocol version, and two-factor tokens. Exported names tell us where to probe; they do not prove semantic behavior.

`odbcinst -j` discovers the active unixODBC configuration and integer sizes. `isql` and `iusql` provide simple live clients. Keep `ODBCINI` and `ODBCINSTINI` pointed at files under `.research/` so inspection never changes system configuration.

## Wire capture without elevated privileges

Start the tap between the client and local IRIS:

```bash
cargo run --bin iris-tap -- \
  --listen 127.0.0.1:21973 \
  --target 127.0.0.1:21972 \
  --output .research/session.tap
```

Point either the vendor DSN or `IRIS_DSN` at port 21973, run the smallest scenario that exercises one behavior, then stop the tap. Reconstruct frames:

```bash
cargo run --bin iris-dissect -- --items .research/session.tap > .research/session.frames
```

Each tap record contains connection id, direction, elapsed microseconds, chunk length, and hexadecimal bytes. The dissector joins chunks per connection and direction, then applies the 14-byte frame envelope. Captures can contain SQL, result data, tokens, and reversibly obfuscated passwords; keep them local and sanitize any excerpt before sharing.

The initial captures established the `PP`/`PU` update split, prepared statement id behavior, `FD` statement ids, empty numeric metadata, decimal encoding, compact-float alignment, stream references, and POSIX time representation. Each finding has a corresponding live or unit regression test.

## ODBC capability probe

Install unixODBC with development headers and a C compiler. On macOS with Homebrew, `odbc_config` supplies the flags used by the build script:

```bash
sh tools/build-odbc-capabilities.sh
ODBCINI="$PWD/.research/odbc.ini" \
ODBCINSTINI="$PWD/.research/odbcinst.ini" \
IRIS_ODBC_CONNECTION_STRING='DSN=IRISRESEARCH;UID=_SYSTEM;PWD=password' \
  target/odbc-capabilities > .research/odbc-capabilities.txt
```

The probe uses `SQLDriverConnect`, calls `SQLGetFunctions` for 65 individual entry points, reads connection and cursor information with the correct string or integer result type, and dumps every `SQLGetTypeInfo(SQL_ALL_TYPES)` row. Extend it for `SQLTables`, `SQLColumns`, `SQLPrimaryKeys`, `SQLForeignKeys`, `SQLStatistics`, `SQLProcedures`, `SQLProcedureColumns`, `SQLSpecialColumns`, privilege catalogs, attributes, and actual cursor behavior before claiming parity in those areas.

## Differential method

1. Write one hypothesis and the smallest SQL or API scenario that tests it.
2. Capture the vendor driver against a known server version and record client version, driver manager, platform, DSN options, and locale.
3. Run the independent native implementation on the same fresh state.
4. Compare operation order, headers, typed values, status transitions, and externally visible results.
5. Check errors as well as success. Several layout facts appeared only because DDL and continuation responses differed from simple inserts and single-row queries.
6. Add an offline codec regression and a live integration assertion. Mark unverified alternatives explicitly in `docs/protocol.md` and `docs/compatibility.md`.
7. Repeat on another IRIS release, platform, authentication mode, and locale before generalizing the behavior.

Never use one successful opcode capture as proof of full feature support. A driver's `SQLGetFunctions` answer is an advertisement to test, just as an exported symbol is a location to investigate, not a compatibility guarantee.
