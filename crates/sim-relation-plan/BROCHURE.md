# sim-relation-plan

In one line: Admission and sealed checked logical relational plans for SIM.

## What it gives you

A typed algebra for queries and mutations whose admission resolves every field, binding, domain, and operation against an exact schema. Successful admission yields opaque plans with canonical identities suitable for provider requests and evidence. Invalid bindings and type mismatches fail before provider execution. One logical plan works across installed relation providers. Canonical plan identity supports caching, receipts, and replay. This crate owns pure relational planning and admission. Schemas and domains come from foundation records; relation sites execute only checked plans. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-relation-plan owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
