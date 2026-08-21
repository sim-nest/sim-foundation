# sim-host-core

`sim-host-core` defines the neutral vocabulary shared by loadable host ports:
open provider and service identities, declared mechanical limits, sanitized
provenance, runtime-mechanics refusals, and lexical environment binding.

It contains no operating-system calls, provider implementations, evidence
grading, or product policy. Domain crates define their own opaque `HostPort`
objects and bind them into a child `Env`; platform libraries choose and realize
those objects.

Rustdoc and unit tests are the executable contract lane. There is no recipe
directory because concrete host scenarios belong to the domain libraries that
realize these neutral contracts.
