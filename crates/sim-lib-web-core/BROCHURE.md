# sim-lib-web-core

In one line: Pure web capture, representation, selector, policy, and receipt records for SIM.

## What it gives you

`sim-lib-web-core` separates retrieved bytes from normalized text and gives each its own canonical identity. A quote is usable only when its Unicode-scalar range and exact text still match that immutable representation. Complete typed policy receipts make missing authorization a denial, never an ambient default. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-lib-web-core owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
