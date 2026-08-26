# sim-cookbook-build

In one line: Portable host build tool for embedding SIM cookbook recipes.

## What it gives you

A portable build-script helper that checks recipe books, setup files, and purpose notes before packaging their exact bytes for sim-cookbook. Invalid trees fail the build; runtime libraries consume supplied data and never acquire filesystem authority. Broken or incomplete lessons fail before release. Embedded recipes stay byte-identical to their reviewed sources. Host build mechanics remain outside product runtime code. This is foundation-owned build tooling. It prepares cookbook assets at compile time; sim-cookbook owns their runtime model and projection. The contract keeps inputs, outputs, limits, and refusal cases explicit, so callers can compose the capability without acquiring unrelated host, transport, or product authority. Stable records make the result suitable for tests, inspection, and deterministic integration.

## Why you will be glad

- The public contract makes supported behavior, limits, and typed failures visible before integration.
- One owning crate prevents neighboring libraries from growing competing copies of the same policy.
- Deterministic records and checked tests keep adapters reviewable when implementations evolve.

## Where it fits

Within SIM, sim-cookbook-build owns only the focused contract described above. Adjacent runtime libraries, platform adapters, codecs, and user surfaces can build around it while retaining their own policy. That boundary keeps the kernel small, avoids competing implementations, and lets this capability evolve without forcing unrelated components to change.
