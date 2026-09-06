# sim-conformance-core

In one line: Keep every conformance claim tied to its exact owner, subject, scope, evidence, and revocation authority.

## What it gives you

`sim-conformance-core` gives systems a durable vocabulary for declaring who owns a rule, which checks may judge it, and what a passing result actually covers. It keeps design activation separate from delivered behavior, prevents narrow evidence from being reused as a broader claim, and detects circular support before work begins. Exact identities make repeated checks comparable while preserving the difference between semantic meaning and stored bytes.

## Why you will be glad

- Reviewers can trace a result to one declaration, one subject, one scope, and one exact invocation.
- Declared capabilities stay visibly unavailable until their own evidence exists.
- Revocation and support relationships fail closed instead of silently trusting stale results.

## Where it fits

Within SIM, sim-conformance-core is the effect-free foundation beneath checker packs, maintenance tooling, proof catalogs, and bounded work. It owns shared evidence structure while domain libraries retain their own behavior and policies.
