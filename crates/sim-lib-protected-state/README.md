# sim-lib-protected-state

Bounded XChaCha20-Poly1305 envelopes for opaque caller-serialized state, authenticated against purpose, audience, subject, context digest, and expiry.

The crate injects its read-only key ring, current-key selection, secure nonce source, and platform wall clock. It generates and persists no production key. AEAD is not replay prevention: callers that require single use must claim a stable opaque identifier through `ConsumptionLedger`, whose supplied `TableConsumptionLedger` uses the canonical kernel `Table::compare_exchange` operation.

Opened plaintext and copied key material are zeroized on drop. Rust allocators, caller-owned input buffers, AEAD internals, and copies made before or after this API may retain bytes; callers must minimize copies and own typed serialization and canonical request digests.
