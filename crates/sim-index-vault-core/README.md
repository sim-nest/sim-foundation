# sim-index-vault-core

`sim-index-vault-core` is the syntax-free projection law between a complete
public SIM Index and vault-shaped notes. It retains every input record as the
canonical `IndexRow`, assigns exactly one primary claim site, and proves exact
set equality with a `ClaimCertificate`. Derived navigation is recorded
separately and must name a row that already has a primary claim.

The crate performs no parsing, rendering, filesystem access, process work,
environment inspection, clock access, or application-profile selection. Its
only dependency is `sim-index-core`.

The crate uses the `rustdoc` recipe-policy exception: rustdoc and the
property-oriented fixture tests teach its pure data law, so it ships no recipe
directory.
