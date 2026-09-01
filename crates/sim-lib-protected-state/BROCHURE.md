# sim-lib-protected-state

In one line: Bounded, binding-authenticated opaque state envelopes for SIM protocols.

## What it gives you

Carry opaque continuation state without inventing protocol-specific cryptography. Stable, strictly bounded, versioned XChaCha20-Poly1305 envelopes. Exact purpose/audience/subject/context/expiry binding. Injected key ring, rotation policy, cryptographic RNG, and platform clock. Uniform rejection diagnostics that disclose no key, binding, or parse detail. Optional atomic single-use claims over the canonical Table compare-exchange contract. Serialization stays with the caller. Keys stay with the host. Replay guarantees exist only when the consumption ledger is used. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-lib-protected-state owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
