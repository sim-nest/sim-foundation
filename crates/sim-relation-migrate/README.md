# sim-relation-migrate

Provider-neutral schema evolution as an admitted, identity-checked program.
Every step names its exact input and output schema; destructive or ambiguous
changes must be authored, while derivation is limited to lossless additions.
Providers attest live normalized state and advertise the capabilities required
for transactional application and post-apply introspection.

The checked examples live in rustdoc and integration tests. This foundational
admission crate intentionally has no recipe directory because it does not own a
runtime execution surface.
