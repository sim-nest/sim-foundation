# sim-work-core

In one line: Turn a bounded change request into deterministic work whose inputs, limits, dependencies, and finish condition are explicit.

## What it gives you

`sim-work-core` gives orchestrators one precise envelope for local code work, model work, and other bounded tasks. It verifies exact declared inputs, keeps intended targets distinct from trusted dependencies, measures full input before dispatch, and records what counts as completion. Host authority stays outside the envelope, so an implementation proposal remains reviewable data until an authorized adapter acts on it.

## Why you will be glad

- Oversize input is refined visibly instead of being silently cut off.
- Missing dependencies and wrong-phase targets fail before expensive work starts.
- Repeated construction over the same meaning produces the same packet identity.

## Where it fits

Within SIM, sim-work-core sits below maintenance tooling and agent adapters. It shares neutral conformance identities while leaving model selection, execution policy, process control, and persistence to their owning libraries.
