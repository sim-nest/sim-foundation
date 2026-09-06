# sim-conformance-core

`sim-conformance-core` is the neutral record and verification boundary for SIM
conformance. It owns role-safe canonical identities, immutable owner bindings,
separately scoped qualification, typed checker templates, exact invocations,
receipts, revocation state, digest-construction accountability, acyclic support
graphs, and deterministic fake ports.

The crate performs no filesystem, process, Git, network, release, or approval
operation. A checker result gains authority only when its binding, code, pack,
subject, scope, call, policy, provenance, support, and revocation source agree.
Activation evidence cannot stand in for produced or final qualification.

The `descriptor` recipe-policy applies because the recipe is machine-indexed
contract data rather than an executable product. The public Rust examples and
integration tests exercise the behavior.
