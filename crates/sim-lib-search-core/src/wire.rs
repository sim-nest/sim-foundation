use sim_kernel::{Datum, NumberLiteral, Symbol};

/// Stable Shape/Citizen descriptor inventory for general-purpose codecs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordDescriptor {
    pub symbol: &'static str,
    pub version: u32,
}
pub const RECORD_DESCRIPTORS: &[RecordDescriptor] = &[
    RecordDescriptor {
        symbol: "search/Query",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/Site",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/ProviderClaim",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/Observation",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/Page",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/Notice",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/AliasEvidence",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/RankContribution",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/Run",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/ResearchBundle",
        version: 1,
    },
    RecordDescriptor {
        symbol: "search/Citation",
        version: 1,
    },
];

pub(crate) fn sym(s: &str) -> Symbol {
    Symbol::qualified("search", s)
}
pub(crate) fn field(n: &str, v: Datum) -> (Symbol, Datum) {
    (sym(n), v)
}
pub(crate) fn node(n: &str, f: Vec<(Symbol, Datum)>) -> Datum {
    Datum::Node {
        tag: sym(n),
        fields: f,
    }
}
pub(crate) fn u32d(v: u32) -> Datum {
    Datum::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "u32"),
        canonical: v.to_string(),
    })
}
