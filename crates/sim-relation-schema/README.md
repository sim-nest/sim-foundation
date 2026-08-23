# sim-relation-schema

Provider-neutral relational schema intent and normalized observations of live
provider catalogs. Construction validates names, domains, constraints,
generated/default expressions, foreign keys, and view dependency graphs before
a codec or storage provider sees the schema.

The logical and physical records deliberately have different types and Datum
tags. Both identities use `sim-relation-core`'s canonical kernel-backed digest.

The crate uses the `rustdoc` recipe-policy exception: exact store-schema
fixtures are checked by rustdoc and integration tests, so it intentionally
ships no recipe directory.
