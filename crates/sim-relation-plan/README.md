# sim-relation-plan

Provider-neutral relational query and mutation algebra. Admission resolves every
binding and type against a schema and domain catalog, producing opaque checked
plans whose canonical identities are safe provider request keys.

The checked examples live in rustdoc and integration tests. This foundational
admission crate intentionally has no recipe directory because it does not own a
runtime execution surface.
