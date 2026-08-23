# Open relational data without a parallel universe

`sim-relation-core` gives tables, domains, cells, and rows stable vocabulary
while retaining SIM's ordinary data and content identity. Providers can add a
logical domain without changing this crate, and every record remains visible to
Card and Lisp surfaces as a canonical `Datum`.
