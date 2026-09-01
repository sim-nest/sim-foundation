# sim-study-core

`sim-study-core` is the domain-neutral record boundary for repeatable studies.
It defines opaque subject revisions, exact study coordinates, attempts and
terminal outcomes, observations, resources, treatments, estimates, decisions,
selections, evidence strength, privacy policy, canonical `Datum` projections,
and strict record shapes.

The crate deliberately does no arithmetic, ordering, storage, execution, or
subject decoding. Operational paths, timestamps, retry policy, placement,
credentials, and private payloads cannot enter coordinate identity or public
exports.

This pure record crate uses the rustdoc recipe-policy exception: its checked
API examples and conformance tests are the teaching surface, so it has no recipe
directory.
