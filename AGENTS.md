# Agent rules

Verify protocol behavior against a real IRIS server and, where possible, the vendor ODBC driver. Never claim support from opcode presence alone: mark behavior as observed, implemented, tested, or unverified.

Run `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets` before pushing. Live tests require `IRIS_DSN` and run with `cargo test --test live -- --ignored`; use only a disposable database.

Do not commit vendor driver binaries, IRIS databases, credentials, DSN files, packet captures, or generated logs. `.research/` is local-only. Treat every capture as sensitive because password encoding is reversible obfuscation, not encryption.

Keep markdown paragraphs on one line and let viewers soft-wrap. Do not add AI bylines or generated-with trailers to commits.

The initial protocol codec is an independent Rust rewrite informed by `caretdev/go-irisnative` under Apache-2.0. Preserve `NOTICE` and record the upstream commit when updating that attribution.

macOS builds target Apple Silicon. Changes must also compile on Linux and Windows; avoid platform-specific dependencies unless they are optional and feature-gated.

## Crate publishing

Use `.github/workflows/publish.yml` for releases. The workflow requires a GitHub release tag matching `Cargo.toml` or an explicit manual confirmation, and it reads the crates.io API token only from the `CARGO_REGISTRY_TOKEN` repository secret. Never print, commit, or write that token to a file. Do not publish directly from a development machine.
