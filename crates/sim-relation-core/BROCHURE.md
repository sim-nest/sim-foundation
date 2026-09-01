# sim-relation-core

In one line: Open relational domain, row, and canonical identity records for SIM.

## What it gives you

`sim-relation-core` gives tables, domains, cells, and rows stable vocabulary while retaining SIM's ordinary data and content identity. Providers can add a logical domain without changing this crate, and every record remains visible to Card and Lisp surfaces as a canonical `Datum`. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-relation-core owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
