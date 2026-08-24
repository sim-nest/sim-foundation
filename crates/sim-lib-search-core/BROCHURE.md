# Search records without provider capture

`sim-lib-search-core` keeps provider claims distinct from retrieved observations
and exact citations. It carries ranking contributions without choosing a rank
algorithm and exposes a pure object-safe wire boundary with stable identity.
The records can cross any installed general-purpose Datum Lisp or JSON codec.
