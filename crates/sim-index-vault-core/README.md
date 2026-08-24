# sim-index-vault-core

`sim-index-vault-core` is the syntax-free projection law between a complete
public SIM Index and vault-shaped notes. It retains every input record as the
canonical `IndexRow`, assigns exactly one primary claim site, and proves exact
set equality with a `ClaimCertificate`. Derived navigation is recorded
separately and must name a row that already has a primary claim.

The crate performs no parsing, rendering, filesystem access, process work,
environment inspection, clock access, or application-profile selection. Its
only dependency is `sim-index-core`.

## Compose a projection

`IndexDoc::inventory()` is the canonical row inventory. A future projection
must route every `IndexRowRef` through that inventory rather than repeat the
eleven `IndexDoc` collections:

```rust
let (_metadata, rows) = index.inventory();
for row in rows {
    let owned = row.to_owned();
    // Choose a note and make exactly one primary claim for `owned`.
}
```

`VaultProjection::from_complete` performs that composition and closes its
`ClaimCertificate`. A note target is valid only when the certificate contains
the exact canonical row; derived backlinks cannot substitute for a primary
claim. The `projection` conformance specimen includes a deliberately missing
claim and proves closure fails.

The crate uses the `rustdoc` recipe-policy exception: rustdoc and the checked
`tests/projection.rs` teaching specimen cover exact substitution failure and
the pure data law, so it ships no recipe directory or duplicate package.
