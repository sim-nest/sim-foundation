# sim-relation-schema

In one line: Validated logical and normalized physical relational schemas for SIM.

## What it gives you

`sim-relation-schema` captures portable relational intent separately from a normalized observation of a provider catalog. It preserves meaningful column and key order, canonicalizes unordered collections, and rejects broken schema graphs before they cross a codec or provider boundary. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-relation-schema owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
