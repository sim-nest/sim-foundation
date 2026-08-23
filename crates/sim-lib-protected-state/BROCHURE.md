# Protected State

Carry opaque continuation state without inventing protocol-specific cryptography.

- Stable, strictly bounded, versioned XChaCha20-Poly1305 envelopes.
- Exact purpose/audience/subject/context/expiry binding.
- Injected key ring, rotation policy, cryptographic RNG, and platform clock.
- Uniform rejection diagnostics that disclose no key, binding, or parse detail.
- Optional atomic single-use claims over the canonical Table compare-exchange contract.

Serialization stays with the caller. Keys stay with the host. Replay guarantees exist only when the consumption ledger is used.
