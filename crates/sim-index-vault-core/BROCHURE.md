# Exact Index vault projection

Project every canonical SIM Index fact into a deterministic note plan without
copying its schema. Exact claim certificates distinguish missing, duplicate,
unknown, and derived-only claims, including future row families exposed by the
canonical inventory.

Build future projections from `IndexDoc::inventory()` and `IndexRowRef`; admit
future note targets only through the closed claim certificate. Syntax, profile
selection, decoding, and filesystem writes remain separate owner layers.
