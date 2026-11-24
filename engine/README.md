# Chronicles Rust crate

The `chronicles` binary collects usage with tokscale-core, serves the embedded web dashboard, exports standalone HTML, and syncs another Mac over SSH. See the [project README](../README.md) for setup and data locations.

## Build and run

Requires Rust 1.98. The local `rust-toolchain.toml` selects it through rustup.

```sh
cd engine
cargo build --release --locked
./target/release/chronicles serve
```

```text
chronicles serve [--port 8768] [--demo] [--offline] [--web-dir DIR]
chronicles export PATH [--demo] [--offline]
chronicles sync [--host macmini] [--offline]
chronicles report [--offline] [--clients a,b] [--exclude a,b] [--home DIR]
chronicles --list-clients
```

`report` prints `{"engine", "tokscaleCore", "clients": [{id, name}], "messages": [...]}`. Each message has `client`, `model`, `provider`, `session`, `dedup`, `timestamp` (epoch milliseconds), token counts, `messages`, `cost` (USD or null), and `costSource` (`estimated`, `partial`, `reported`, or `unpriced`). `--clients` selects harnesses and `--exclude` removes harnesses from that selection. Cursor appears in the list of upstream clients. Local and remote scans omit its history.

The in-process collector and sync import share report conversion and event ID generation. The server captures tokscale stderr during serialized scans on Unix and surfaces lines beginning with `Warning:`. The raw `report` command leaves diagnostics on stderr for the sync caller. `--offline` uses cached pricing, and reports use `TOKSCALE_CONFIG_DIR` when set. tokscale-core initializes pricing once per process. Restart a running server to reload its pricing catalog.

## tokscale-core dependency

Parsing and pricing come from `tokscale-core`, the MIT-licensed library of the [tokscale](https://github.com/junhoyeo/tokscale) project by Junho Yeo. `Cargo.toml` pins it at commit `b265e83`. To update it, change the revision and the version string in `src/report.rs`, then rebuild and run the parity tests.
