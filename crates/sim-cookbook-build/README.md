# sim-cookbook-build

Portable host-side support for validating and embedding SIM cookbook recipe
trees from Cargo build scripts. Product code consumes the resulting supplied
bytes through `sim-cookbook` and never depends on this tool.

The API is documented in rustdoc. This host tool intentionally ships no recipe
directory because it is build infrastructure, not an installed runtime library.
