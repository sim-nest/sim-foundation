//! Provider-neutral, effect-free search and research records.
//!
//! Providers may make claims. Only verified web selectors become citations;
//! no conversion exists from [`ProviderClaim`] to [`Citation`]. Ranking is
//! represented as contributions and never implemented here.

#![forbid(unsafe_code)]

use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};
use sim_lib_net_core::normalize_retrieval_uri;
use sim_lib_web_core::{DecodeLimits, EvidenceSelector, WebRecordError, WebRepresentation};
use std::{error::Error, fmt};

/// Network-free cookbook descriptors embedded at build time.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchError {
    EmptyQuery,
    BoundExceeded(&'static str),
    InvalidRecord(&'static str),
    Citation(WebRecordError),
    Wire(String),
}
impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for SearchError {}
impl From<WebRecordError> for SearchError {
    fn from(value: WebRecordError) -> Self {
        Self::Citation(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub sites: Vec<SearchSite>,
    pub language: Option<String>,
    pub limit: u32,
}
impl SearchQuery {
    pub fn checked(
        text: String,
        sites: Vec<SearchSite>,
        language: Option<String>,
        limit: u32,
    ) -> Result<Self, SearchError> {
        if text.trim().is_empty() {
            return Err(SearchError::EmptyQuery);
        }
        if text.len() > 16_384 || sites.len() > 256 || limit == 0 || limit > 10_000 {
            return Err(SearchError::BoundExceeded("query"));
        }
        Ok(Self {
            text,
            sites,
            language,
            limit,
        })
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "query",
            vec![
                field("text", Datum::String(self.text.clone())),
                field(
                    "sites",
                    Datum::Vector(self.sites.iter().map(SearchSite::to_datum).collect()),
                ),
                field(
                    "language",
                    self.language.clone().map_or(Datum::Nil, Datum::String),
                ),
                field("limit", u32d(self.limit)),
            ],
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchSite {
    pub domain: String,
    pub include_subdomains: bool,
}
impl SearchSite {
    fn to_datum(&self) -> Datum {
        node(
            "site",
            vec![
                field("domain", Datum::String(self.domain.clone())),
                field("include-subdomains", Datum::Bool(self.include_subdomains)),
            ],
        )
    }
}

/// A provider's unverified title/snippet statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderClaim {
    pub provider: String,
    pub uri: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub position: Option<u32>,
}
/// Retrieval identity observed independently of a provider claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchObservation {
    pub retrieval_uri: String,
    pub claim: Option<ProviderClaim>,
    pub capture_id: Option<ContentId>,
}
impl SearchObservation {
    pub fn checked(
        uri: &str,
        claim: Option<ProviderClaim>,
        capture_id: Option<ContentId>,
    ) -> Result<Self, SearchError> {
        Ok(Self {
            retrieval_uri: normalize_retrieval_uri(uri)
                .map_err(|e| SearchError::Wire(e.to_string()))?
                .as_str()
                .to_owned(),
            claim,
            capture_id,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchPage {
    pub query: SearchQuery,
    pub observations: Vec<SearchObservation>,
    pub continuation: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchNotice {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AliasEvidence {
    pub left_uri: String,
    pub right_uri: String,
    pub basis: String,
    pub evidence_id: ContentId,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RankContribution {
    pub observation: u32,
    pub contributor: String,
    pub score: NumberLiteral,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRun {
    pub query: SearchQuery,
    pub pages: Vec<SearchPage>,
    pub notices: Vec<SearchNotice>,
    pub aliases: Vec<AliasEvidence>,
    pub rank: Vec<RankContribution>,
}

/// A checked citation can only be built from a matching representation selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Citation {
    pub selector: EvidenceSelector,
}
impl Citation {
    pub fn checked(
        rep: &WebRepresentation,
        selector: EvidenceSelector,
    ) -> Result<Self, SearchError> {
        selector.verify(rep)?;
        Ok(Self { selector })
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "citation",
            vec![field("selector", self.selector.to_datum())],
        )
    }
    pub fn from_datum(
        value: &Datum,
        rep: &WebRepresentation,
        limits: DecodeLimits,
    ) -> Result<Self, SearchError> {
        let Datum::Node { tag, fields } = value else {
            return Err(SearchError::InvalidRecord("citation"));
        };
        if tag != &sym("citation") || fields.len() != 1 || fields[0].0 != sym("selector") {
            return Err(SearchError::InvalidRecord("citation"));
        }
        Self::checked(
            rep,
            EvidenceSelector::from_datum(&fields[0].1, rep, limits)?,
        )
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResearchBundle {
    pub run: SearchRun,
    pub representations: Vec<ContentId>,
    pub citations: Vec<Citation>,
    pub notices: Vec<SearchNotice>,
}

/// Pure object-safe provider wire boundary. Implementations own syntax only.
pub trait SearchWireCodec {
    fn codec_id(&self) -> &str;
    fn codec_version(&self) -> &str;
    fn encode_request(
        &self,
        request: &SearchQuery,
        limits: DecodeLimits,
    ) -> Result<Vec<u8>, SearchError>;
    fn decode_config(&self, input: &[u8], limits: DecodeLimits) -> Result<Datum, SearchError>;
    fn decode_response(
        &self,
        input: &[u8],
        request: &SearchQuery,
        limits: DecodeLimits,
    ) -> Result<SearchPage, SearchError>;
}

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

fn sym(s: &str) -> Symbol {
    Symbol::qualified("search", s)
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

#[cfg(test)]
mod tests {
    use super::*;
    fn raw(b: &[u8]) -> ContentId {
        Datum::Bytes(b.to_vec()).content_id().unwrap()
    }
    #[test]
    fn query_order_remains_distinct_observations() {
        let a = SearchObservation::checked("https://EXAMPLE.com/?a=1&b=2", None, None).unwrap();
        let b = SearchObservation::checked("https://example.com/?b=2&a=1", None, None).unwrap();
        assert_ne!(a, b)
    }
    #[test]
    fn citation_round_trip_requires_exact_quote() {
        let rep = WebRepresentation::checked(
            raw(b"raw"),
            "alpha beta".into(),
            sim_lib_web_core::RepresentationMetadata {
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
        let citation = Citation::checked(&rep, rep.select(6, 10).unwrap()).unwrap();
        assert_eq!(
            Citation::from_datum(&citation.to_datum(), &rep, DecodeLimits::default()).unwrap(),
            citation
        );
        let other = WebRepresentation::checked(
            raw(b"other"),
            "alpha zeta".into(),
            sim_lib_web_core::RepresentationMetadata {
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
        assert!(
            Citation::from_datum(&citation.to_datum(), &other, DecodeLimits::default()).is_err()
        )
    }
    #[test]
    fn provider_claim_is_not_a_citation() {
        let claim = ProviderClaim {
            provider: "p".into(),
            uri: "https://example.com".into(),
            title: None,
            snippet: Some("beta".into()),
            position: Some(1),
        };
        assert_eq!(claim.snippet.as_deref(), Some("beta"));
        assert_eq!(
            RECORD_DESCRIPTORS
                .iter()
                .filter(|d| d.symbol.ends_with("Citation"))
                .count(),
            1
        )
    }
}
