use crate::{DecodeLimits, RepresentationMetadata, WebRecordError};
use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};

/// Stable runtime descriptor shared by Shape/Citizen installers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordDescriptor {
    pub symbol: &'static str,
    pub version: u32,
}
pub const RECORD_DESCRIPTORS: &[RecordDescriptor] = &[
    RecordDescriptor {
        symbol: "web/Capture",
        version: 1,
    },
    RecordDescriptor {
        symbol: "web/Exchange",
        version: 1,
    },
    RecordDescriptor {
        symbol: "web/Representation",
        version: 1,
    },
    RecordDescriptor {
        symbol: "web/EvidenceSelector",
        version: 1,
    },
    RecordDescriptor {
        symbol: "web/PolicyReceipt",
        version: 1,
    },
];

pub(crate) fn sym(s: &str) -> Symbol {
    Symbol::qualified("web", s)
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
pub(crate) fn cid(v: &ContentId) -> Datum {
    node(
        "content-id",
        vec![
            field("algorithm", Datum::Symbol(v.algorithm.clone())),
            field("digest", Datum::Bytes(v.bytes.to_vec())),
        ],
    )
}
pub(crate) fn read_cid(v: &Datum) -> Result<ContentId, WebRecordError> {
    let Datum::Node { tag, fields } = v else {
        return Err(WebRecordError::InvalidRecord("content id"));
    };
    if tag != &sym("content-id") || fields.len() != 2 {
        return Err(WebRecordError::InvalidRecord("content id"));
    }
    let (Datum::Symbol(a), Datum::Bytes(b)) = (&fields[0].1, &fields[1].1) else {
        return Err(WebRecordError::InvalidRecord("content id"));
    };
    let bytes: [u8; 32] = b
        .as_slice()
        .try_into()
        .map_err(|_| WebRecordError::InvalidRecord("digest"))?;
    Ok(ContentId::from_bytes(a.clone(), bytes))
}
pub(crate) fn read_u32(v: &Datum) -> Result<u32, WebRecordError> {
    match v {
        Datum::Number(n) if n.domain == Symbol::qualified("numbers", "u32") => n
            .canonical
            .parse()
            .map_err(|_| WebRecordError::InvalidRecord("u32")),
        _ => Err(WebRecordError::InvalidRecord("u32")),
    }
}
pub(crate) fn opt_text(v: &Option<String>) -> Datum {
    v.clone().map_or(Datum::Nil, Datum::String)
}
pub(crate) fn read_opt_text(v: &Datum, l: DecodeLimits) -> Result<Option<String>, WebRecordError> {
    match v {
        Datum::Nil => Ok(None),
        Datum::String(s) if s.len() <= l.max_text_bytes => Ok(Some(s.clone())),
        Datum::String(_) => Err(WebRecordError::BoundExceeded("context")),
        _ => Err(WebRecordError::InvalidRecord("context")),
    }
}
pub(crate) fn representation_identity(
    raw: &ContentId,
    text: &str,
    metadata: &RepresentationMetadata,
) -> Datum {
    node(
        "representation",
        vec![
            field("raw", cid(raw)),
            field("text", Datum::String(text.into())),
            field("codec", Datum::String(metadata.codec.clone())),
            field(
                "codec-version",
                Datum::String(metadata.codec_version.clone()),
            ),
            field("media-type", Datum::String(metadata.media_type.clone())),
            field(
                "charset",
                metadata.charset.clone().map_or(Datum::Nil, Datum::String),
            ),
            field(
                "language",
                metadata.language.clone().map_or(Datum::Nil, Datum::String),
            ),
            field(
                "warnings",
                Datum::Vector(
                    metadata
                        .fidelity_warnings
                        .iter()
                        .cloned()
                        .map(Datum::String)
                        .collect(),
                ),
            ),
        ],
    )
}
