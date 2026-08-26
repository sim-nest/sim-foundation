# sim-relation-migrate

In one line: Checked schema schema evolution programs and adoption attestations for SIM.

## What it gives you

schema evolution programs whose every step names exact input and output schemas, plus adoption attestations for existing stores. Lossless additions may be derived; destructive or ambiguous changes must be authored and admitted explicitly. Schema drift becomes a typed refusal instead of a surprise. Providers advertise the transactional and introspection support a schema evolution needs. Canonical identities make schema evolution evidence reproducible. This foundation crate defines schema evolution data and admission. Relation sites and storage providers execute admitted programs; they do not reinterpret them. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-relation-migrate owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
