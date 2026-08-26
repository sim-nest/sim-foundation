# sim-host-core

In one line: Neutral host-port contracts for SIM domain libraries.

## What it gives you

Open provider and service identities, explicit mechanical limits, sanitized provenance, stable refusals, and lexical environment binding. Domain crates can request opaque ports as data while platform libraries choose and realize the concrete service. Domain behavior stays portable and testable. Missing or over-capable host services fail explicitly. Provenance can be inspected without exposing native handles or secrets. This crate is the shared vocabulary between portable libraries and platform adapters. It performs no operating-system calls and owns no provider or product policy. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-host-core owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
