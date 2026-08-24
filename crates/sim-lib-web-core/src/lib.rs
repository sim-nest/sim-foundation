//! Pure, effect-free web evidence records.
//!
//! Capturing, decoding, and policy execution live elsewhere. This crate only
//! names immutable exchange facts and proves selectors against normalized text.

#![forbid(unsafe_code)]

use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};
use sim_lib_net_core::RetrievalUri;
use std::{error::Error, fmt};

/// Network-free cookbook descriptors embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

/// Conservative public decode limits for canonical record projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_text_bytes: usize,
    pub max_body_bytes: usize,
    pub max_items: usize,
}
impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_text_bytes: 1_048_576,
            max_body_bytes: 16_777_216,
            max_items: 4_096,
        }
    }
}

/// Validation or bounded decoding refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebRecordError {
    BoundExceeded(&'static str),
    InvalidSelector,
    ContentIdentity,
    MissingDecision(PolicyKind),
    InvalidRecord(&'static str),
}
impl fmt::Display for WebRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for WebRecordError {}

/// Raw retrieved bytes with their own content identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebCapture {
    pub retrieval_uri: RetrievalUri,
    pub content_id: ContentId,
    pub body: Vec<u8>,
    pub exchange: WebExchange,
}
impl WebCapture {
    pub fn checked(
        retrieval_uri: RetrievalUri,
        content_id: ContentId,
        body: Vec<u8>,
        exchange: WebExchange,
        limits: DecodeLimits,
    ) -> Result<Self, WebRecordError> {
        if body.len() > limits.max_body_bytes {
            return Err(WebRecordError::BoundExceeded("raw body"));
        }
        let actual = Datum::Bytes(body.clone())
            .content_id()
            .map_err(|_| WebRecordError::ContentIdentity)?;
        if actual != content_id {
            return Err(WebRecordError::ContentIdentity);
        }
        Ok(Self {
            retrieval_uri,
            content_id,
            body,
            exchange,
        })
    }
}

/// Provider-neutral request/response exchange facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebExchange {
    pub method: String,
    pub status: u16,
    pub final_uri: String,
    pub media_type: Option<String>,
    pub received_bytes: u64,
}

/// Immutable normalized text and the provenance required to interpret it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebRepresentation {
    pub content_id: ContentId,
    pub raw_source_id: ContentId,
    pub text: String,
    pub codec: String,
    pub codec_version: String,
    pub media_type: String,
    pub charset: Option<String>,
    pub language: Option<String>,
    pub fidelity_warnings: Vec<String>,
}
/// Codec and fidelity provenance for one normalized representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationMetadata {
    pub codec: String,
    pub codec_version: String,
    pub media_type: String,
    pub charset: Option<String>,
    pub language: Option<String>,
    pub fidelity_warnings: Vec<String>,
}
impl WebRepresentation {
    pub fn checked(
        raw_source_id: ContentId,
        text: String,
        metadata: RepresentationMetadata,
        limits: DecodeLimits,
    ) -> Result<Self, WebRecordError> {
        if text.len() > limits.max_text_bytes || metadata.fidelity_warnings.len() > limits.max_items
        {
            return Err(WebRecordError::BoundExceeded("representation"));
        }
        let identity = representation_identity(&raw_source_id, &text, &metadata);
        let content_id = identity
            .content_id()
            .map_err(|_| WebRecordError::ContentIdentity)?;
        if content_id == raw_source_id {
            return Err(WebRecordError::ContentIdentity);
        }
        Ok(Self {
            content_id,
            raw_source_id,
            text,
            codec: metadata.codec,
            codec_version: metadata.codec_version,
            media_type: metadata.media_type,
            charset: metadata.charset,
            language: metadata.language,
            fidelity_warnings: metadata.fidelity_warnings,
        })
    }
    /// Select Unicode scalar offsets from this immutable representation.
    pub fn select(&self, start: u32, end: u32) -> Result<EvidenceSelector, WebRecordError> {
        let count = self.text.chars().count();
        if start > end || end as usize > count {
            return Err(WebRecordError::InvalidSelector);
        }
        let exact = self
            .text
            .chars()
            .skip(start as usize)
            .take((end - start) as usize)
            .collect();
        EvidenceSelector::checked(self.content_id.clone(), start, end, exact, &self.text)
    }
}

/// A quote anchored to Unicode scalar offsets and optional context/path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceSelector {
    pub representation_id: ContentId,
    pub start: u32,
    pub end: u32,
    pub exact: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub structural_path: Option<Vec<String>>,
}
impl EvidenceSelector {
    pub fn checked(
        representation_id: ContentId,
        start: u32,
        end: u32,
        exact: String,
        text: &str,
    ) -> Result<Self, WebRecordError> {
        if start > end || end as usize > text.chars().count() {
            return Err(WebRecordError::InvalidSelector);
        }
        let observed: String = text
            .chars()
            .skip(start as usize)
            .take((end - start) as usize)
            .collect();
        if observed != exact {
            return Err(WebRecordError::InvalidSelector);
        }
        Ok(Self {
            representation_id,
            start,
            end,
            exact,
            prefix: None,
            suffix: None,
            structural_path: None,
        })
    }
    pub fn with_context(
        mut self,
        prefix: Option<String>,
        suffix: Option<String>,
        structural_path: Option<Vec<String>>,
    ) -> Self {
        self.prefix = prefix;
        self.suffix = suffix;
        self.structural_path = structural_path;
        self
    }
    pub fn verify(&self, rep: &WebRepresentation) -> Result<(), WebRecordError> {
        if self.representation_id != rep.content_id {
            return Err(WebRecordError::InvalidSelector);
        }
        Self::checked(
            self.representation_id.clone(),
            self.start,
            self.end,
            self.exact.clone(),
            &rep.text,
        )
        .map(|_| ())
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "selector",
            vec![
                field("representation", cid(&self.representation_id)),
                field("start", u32d(self.start)),
                field("end", u32d(self.end)),
                field("exact", Datum::String(self.exact.clone())),
                field("prefix", opt_text(&self.prefix)),
                field("suffix", opt_text(&self.suffix)),
                field(
                    "path",
                    Datum::Vector(
                        self.structural_path
                            .clone()
                            .unwrap_or_default()
                            .into_iter()
                            .map(Datum::String)
                            .collect(),
                    ),
                ),
            ],
        )
    }
    pub fn from_datum(
        value: &Datum,
        rep: &WebRepresentation,
        limits: DecodeLimits,
    ) -> Result<Self, WebRecordError> {
        let Datum::Node { tag, fields } = value else {
            return Err(WebRecordError::InvalidRecord("selector"));
        };
        if tag != &sym("selector") || fields.len() != 7 {
            return Err(WebRecordError::InvalidRecord("selector"));
        }
        let get = |i: usize, name: &str| {
            if fields[i].0 == sym(name) {
                Ok(&fields[i].1)
            } else {
                Err(WebRecordError::InvalidRecord("selector ordering"))
            }
        };
        let representation_id = read_cid(get(0, "representation")?)?;
        let start = read_u32(get(1, "start")?)?;
        let end = read_u32(get(2, "end")?)?;
        let Datum::String(exact) = get(3, "exact")? else {
            return Err(WebRecordError::InvalidRecord("exact"));
        };
        if exact.len() > limits.max_text_bytes {
            return Err(WebRecordError::BoundExceeded("exact"));
        }
        let mut selector = Self::checked(representation_id, start, end, exact.clone(), &rep.text)?;
        selector.prefix = read_opt_text(get(4, "prefix")?, limits)?;
        selector.suffix = read_opt_text(get(5, "suffix")?, limits)?;
        let Datum::Vector(path) = get(6, "path")? else {
            return Err(WebRecordError::InvalidRecord("path"));
        };
        if path.len() > limits.max_items {
            return Err(WebRecordError::BoundExceeded("path"));
        }
        selector.structural_path = if path.is_empty() {
            None
        } else {
            Some(
                path.iter()
                    .map(|v| match v {
                        Datum::String(s) => Ok(s.clone()),
                        _ => Err(WebRecordError::InvalidRecord("path")),
                    })
                    .collect::<Result<_, _>>()?,
            )
        };
        selector.verify(rep)?;
        Ok(selector)
    }
}

/// Every independent policy question; a complete decision set contains all ten.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicyKind {
    EgressZone,
    Robots,
    Method,
    Domain,
    Media,
    Bytes,
    Redirects,
    Rate,
    CacheMode,
    ResearchBudget,
}
impl PolicyKind {
    pub const ALL: [Self; 10] = [
        Self::EgressZone,
        Self::Robots,
        Self::Method,
        Self::Domain,
        Self::Media,
        Self::Bytes,
        Self::Redirects,
        Self::Rate,
        Self::CacheMode,
        Self::ResearchBudget,
    ];
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyVerdict {
    Allow,
    Deny,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyDecision {
    pub kind: PolicyKind,
    pub verdict: PolicyVerdict,
    pub rule: String,
    pub limit: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyReceipt {
    pub decisions: Vec<PolicyDecision>,
}
impl PolicyReceipt {
    pub fn checked(decisions: Vec<PolicyDecision>) -> Result<Self, WebRecordError> {
        for kind in PolicyKind::ALL {
            if decisions.iter().filter(|d| d.kind == kind).count() != 1 {
                return Err(WebRecordError::MissingDecision(kind));
            }
        }
        Ok(Self { decisions })
    }
    pub fn permits(&self) -> bool {
        PolicyKind::ALL.into_iter().all(|kind| {
            self.decisions
                .iter()
                .any(|d| d.kind == kind && d.verdict == PolicyVerdict::Allow)
        })
    }
}

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

fn sym(s: &str) -> Symbol {
    Symbol::qualified("web", s)
}
fn field(n: &str, v: Datum) -> (Symbol, Datum) {
    (sym(n), v)
}
fn node(n: &str, f: Vec<(Symbol, Datum)>) -> Datum {
    Datum::Node {
        tag: sym(n),
        fields: f,
    }
}
fn u32d(v: u32) -> Datum {
    Datum::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "u32"),
        canonical: v.to_string(),
    })
}
fn cid(v: &ContentId) -> Datum {
    node(
        "content-id",
        vec![
            field("algorithm", Datum::Symbol(v.algorithm.clone())),
            field("digest", Datum::Bytes(v.bytes.to_vec())),
        ],
    )
}
fn read_cid(v: &Datum) -> Result<ContentId, WebRecordError> {
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
fn read_u32(v: &Datum) -> Result<u32, WebRecordError> {
    match v {
        Datum::Number(n) if n.domain == Symbol::qualified("numbers", "u32") => n
            .canonical
            .parse()
            .map_err(|_| WebRecordError::InvalidRecord("u32")),
        _ => Err(WebRecordError::InvalidRecord("u32")),
    }
}
fn opt_text(v: &Option<String>) -> Datum {
    v.clone().map_or(Datum::Nil, Datum::String)
}
fn read_opt_text(v: &Datum, l: DecodeLimits) -> Result<Option<String>, WebRecordError> {
    match v {
        Datum::Nil => Ok(None),
        Datum::String(s) if s.len() <= l.max_text_bytes => Ok(Some(s.clone())),
        Datum::String(_) => Err(WebRecordError::BoundExceeded("context")),
        _ => Err(WebRecordError::InvalidRecord("context")),
    }
}
fn representation_identity(
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

#[cfg(test)]
mod tests {
    use super::*;
    fn raw(body: &[u8]) -> ContentId {
        Datum::Bytes(body.to_vec()).content_id().unwrap()
    }
    #[test]
    fn unicode_selectors_are_scalar_checked_and_round_trip() {
        let rep = WebRepresentation::checked(
            raw(b"raw"),
            "aé🦀z".into(),
            RepresentationMetadata {
                codec: "text".into(),
                codec_version: "1".into(),
                media_type: "text/plain".into(),
                charset: None,
                language: None,
                fidelity_warnings: vec![],
            },
            DecodeLimits::default(),
        )
        .unwrap();
        let s = rep.select(1, 3).unwrap().with_context(
            Some("a".into()),
            Some("z".into()),
            Some(vec!["p".into()]),
        );
        assert_eq!(s.exact, "é🦀");
        assert_eq!(
            EvidenceSelector::from_datum(&s.to_datum(), &rep, DecodeLimits::default()).unwrap(),
            s
        );
        assert!(
            EvidenceSelector::checked(rep.content_id.clone(), 1, 3, "bad".into(), &rep.text)
                .is_err()
        );
    }
    #[test]
    fn raw_and_representation_digests_are_separate() {
        let id = raw(b"hello");
        let rep = WebRepresentation::checked(
            id.clone(),
            "hello".into(),
            RepresentationMetadata {
                codec: "utf8".into(),
                codec_version: "1".into(),
                media_type: "text/plain".into(),
                charset: Some("utf-8".into()),
                language: None,
                fidelity_warnings: vec![],
            },
            DecodeLimits::default(),
        )
        .unwrap();
        assert_ne!(rep.content_id, id);
    }
    #[test]
    fn absent_policy_is_denial() {
        assert!(matches!(
            PolicyReceipt::checked(vec![]),
            Err(WebRecordError::MissingDecision(_))
        ));
    }
}
