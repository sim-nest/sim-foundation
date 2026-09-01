# sim-relation-core

Open relational vocabulary over SIM's kernel `Datum`, `Symbol`, `Ref`, and
`ContentId`. It defines data contracts and canonical projections only; runtime
Shape resolution and storage providers live in downstream libraries.

The API and custom-domain specimen are checked in rustdoc. This data-contract
crate intentionally ships no recipe directory because it has no runtime
behavior to configure or execute.
