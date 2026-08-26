# sim-lib-search-core

In one line: Pure open search, observation, ranking-evidence, and research bundle records for SIM.

## What it gives you

`sim-lib-search-core` keeps provider claims distinct from retrieved observations and exact citations. It carries ranking contributions without choosing a rank algorithm and exposes a pure object-safe wire boundary with stable identity. The records can cross any installed general-purpose Datum Lisp or JSON codec. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-lib-search-core owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
