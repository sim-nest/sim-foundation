# sim-lib-net-core

`sim-lib-net-core` is a side-effect-free parser set for web-facing SIM
libraries. It parses URLs, HTTP response heads, body delivery modes,
chunked-transfer bodies, capped lines, SSE events, NDJSON records, and small text
encodings used by shared wire formats.

The crate opens no sockets, performs no TLS, retries nothing, and attaches no
application policy to the bytes it parses. Callers own transport, credentials,
timeouts, and event meaning.

## What it provides

- URL parsing for HTTP, HTTPS, WebSocket, and scheme-specific callers.
- HTTP request-head construction and response-head parsing.
- Body-mode classification for content length, chunked transfer, and EOF bodies.
- Streaming line framing with partial-line buffering.
- SSE and NDJSON decoders for incremental service responses.
- Build-time cookbook recipe embedding for the crate's network parsing lesson.

## Example

```rust
use sim_lib_net_core::LineDecoder;

let mut decoder = LineDecoder::new();
assert_eq!(decoder.push(b"one\nt"), vec![b"one".to_vec()]);
assert_eq!(decoder.push(b"wo\n"), vec![b"two".to_vec()]);
```

## Recipes, Docs, And Validation

The `recipes/` directory includes a response-head walkthrough that surfaces can
show through the SIM cookbook. Rustdoc examples cover the low-level parser
contracts.

- API docs: <https://docs.rs/sim-lib-net-core>
- Repository guide: <https://github.com/sim-nest/sim-foundation>

From the `sim-foundation` checkout:

```bash
cargo test -p sim-lib-net-core
RUSTDOCFLAGS="-D warnings" cargo doc -p sim-lib-net-core --no-deps
cargo run -p xtask -- check-recipes
cargo run -p xtask -- check-package-floors
cargo run -p xtask -- simdoc --check
```
