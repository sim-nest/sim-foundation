# Relational schemas that refuse ambiguity

`sim-relation-schema` captures portable relational intent separately from a
normalized observation of a provider catalog. It preserves meaningful column
and key order, canonicalizes unordered collections, and rejects broken schema
graphs before they cross a codec or provider boundary.
