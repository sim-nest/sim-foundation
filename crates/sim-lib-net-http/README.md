# sim-lib-net-http

The reusable blocking HTTP policy boundary for SIM. It composes the pure parsers
in `sim-lib-net-core` with injected capsule DNS and socket ports. Callers own an
explicit `Policy`; ambient redirects, cookies, credentials, and proxies are off.

See crate rustdoc and `recipes/01-basics/README.md` for the four documentation
lanes: concept, API reference, runnable recipe, and operations/security policy.
