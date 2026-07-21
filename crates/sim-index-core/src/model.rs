//! Plain data records for the SIM Index graph.

use std::fmt;

macro_rules! id_type {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Builds an id from raw text. Validation happens in the graph check
            /// so parsed input can report every malformed field through one path.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Borrows the raw id text.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Returns true when the id satisfies the index id grammar.
            pub fn is_valid(&self) -> bool {
                crate::shape::is_index_id(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(
    /// Stable id for a user-facing or code-facing feature.
    FeatureId
);
id_type!(
    /// Stable id for a package, crate, runtime lib, language, or other subject.
    SubjectId
);
id_type!(
    /// Stable id for a discovered source anchor.
    AnchorId
);
id_type!(
    /// Stable id for a surface by which a subject is invoked or read.
    SurfaceId
);
id_type!(
    /// Stable id for a runnable, checked specimen.
    SpecimenId
);
id_type!(
    /// Stable id for a task-oriented route through the index.
    RouteId
);

/// Top-level SIM Index document.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexDoc {
    /// Schema marker, normally `sim.index`.
    pub schema: String,
    /// Tool or crate that generated this document.
    pub generated_by: String,
    /// Visibility of the document.
    pub visibility: Visibility,
    /// Discovered package, crate, lib, language, grammar, and runtime subjects.
    pub subjects: Vec<SubjectRecord>,
    /// Discovered source anchors.
    pub anchors: Vec<DiscoveredAnchor>,
    /// Discovered invoke/read/write/view surfaces.
    pub surfaces: Vec<DiscoveredSurface>,
    /// Discovered runnable examples and conformance tests.
    pub specimens: Vec<DiscoveredSpecimen>,
    /// Authored feature drafts before materialization.
    pub drafts: Vec<FeatureDraft>,
    /// Materialized feature records.
    pub features: Vec<FeatureRecord>,
    /// Reader or agent routes through the graph.
    pub routes: Vec<RouteRecord>,
    /// Directed feature-to-feature relationships.
    pub edges: Vec<IndexEdge>,
}

impl IndexDoc {
    /// Builds an empty public index document.
    pub fn public(generated_by: impl Into<String>) -> Self {
        Self {
            schema: "sim.index".to_owned(),
            generated_by: generated_by.into(),
            visibility: Visibility::Public,
            subjects: Vec::new(),
            anchors: Vec::new(),
            surfaces: Vec::new(),
            specimens: Vec::new(),
            drafts: Vec::new(),
            features: Vec::new(),
            routes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

/// Visibility boundary for an index document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Visibility {
    /// Public, generated from public source facts.
    Public,
    /// Private local document that may include local-only facts.
    PrivateLocal,
}

/// A discovered subject that can own anchors, surfaces, specimens, or features.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubjectRecord {
    /// Stable subject id.
    pub id: SubjectId,
    /// Subject kind, such as `repo`, `crate`, `runtime-lib`, or `language`.
    pub kind: String,
    /// Human-facing title.
    pub title: String,
}

/// A discovered source anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredAnchor {
    /// Stable anchor id.
    pub id: AnchorId,
    /// Subject that owns the anchor.
    pub subject: SubjectId,
    /// Anchor kind, such as `export`, `cli-verb`, or `doc`.
    pub kind: String,
}

/// A discovered surface by which a subject is addressed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredSurface {
    /// Stable surface id.
    pub id: SurfaceId,
    /// Subject that owns the surface.
    pub subject: SubjectId,
    /// Surface kind, such as `cli`, `view`, `syntax`, or `wire`.
    pub kind: String,
}

/// A runnable, checked description of a feature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredSpecimen {
    /// Stable specimen id.
    pub id: SpecimenId,
    /// Subject that owns the specimen.
    pub subject: SubjectId,
    /// Specimen kind, such as `recipe` or `spec-test`.
    pub kind: String,
    /// Stable source path that identifies the specimen in its owning repo.
    pub path: String,
    /// Optional language or surface exercised by the specimen.
    pub language: Option<String>,
    /// True when the specimen can be executed by its owning repo.
    pub runnable: bool,
    /// True when validation confirms the specimen ran.
    pub checked: bool,
    /// Optional harness that validates the specimen.
    pub checked_by: Option<String>,
    /// Optional discovered documentation anchor for the specimen.
    pub doc_anchor: Option<AnchorId>,
}

/// Authored feature overlay before discovered claims are materialized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureDraft {
    /// Feature id being authored.
    pub id: FeatureId,
    /// Discovered subject the feature describes.
    pub subject: SubjectId,
    /// Human-facing title.
    pub title: String,
    /// Human-facing summary.
    pub summary: String,
    /// Discovered anchors claimed by id.
    pub claims_anchors: Vec<AnchorId>,
    /// Discovered surfaces claimed by id.
    pub claims_surfaces: Vec<SurfaceId>,
    /// Discovered specimens claimed by id.
    pub claims_specimens: Vec<SpecimenId>,
    /// Literal anchor claims rejected by the checker.
    pub literal_anchors: Vec<String>,
    /// Literal surface claims rejected by the checker.
    pub literal_surfaces: Vec<String>,
    /// Literal specimen bodies rejected by the checker.
    pub literal_specimens: Vec<String>,
    /// Grammar contracts authored for this feature.
    pub grammar_contracts: Vec<GrammarContract>,
    /// Optional discovered documentation anchor for this feature.
    pub doc_anchor: Option<AnchorId>,
}

/// Materialized feature row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureRecord {
    /// Stable feature id.
    pub id: FeatureId,
    /// Canonical key that prevents one feature from being projected twice.
    pub key: crate::CanonicalFeatureKey,
    /// Discovered subject the feature describes.
    pub subject: SubjectId,
    /// Human-facing title.
    pub title: String,
    /// Human-facing summary.
    pub summary: String,
    /// Claimed discovered anchors.
    pub anchors: Vec<AnchorId>,
    /// Claimed discovered surfaces.
    pub surfaces: Vec<SurfaceId>,
    /// Claimed runnable specimens.
    pub specimens: Vec<SpecimenId>,
    /// Grammar contracts attached to this feature.
    pub grammar_contracts: Vec<GrammarContract>,
    /// Optional discovered documentation anchor for this feature.
    pub doc_anchor: Option<AnchorId>,
}

/// Contract tying a grammar to its discovered codec anchors and surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GrammarContract {
    /// Stable grammar id, such as `grammar/lisp`.
    pub id: String,
    /// Decoder anchor, when the grammar is readable.
    pub decoder: Option<AnchorId>,
    /// Encoder anchor, when the grammar is writable.
    pub encoder: Option<AnchorId>,
    /// Surface that exposes the grammar.
    pub surface: Option<SurfaceId>,
    /// True when the grammar has a closed round-trip proof.
    pub round_trip: bool,
}

impl GrammarContract {
    /// Returns true when this grammar contract has the minimum closed shape.
    pub fn is_valid(&self) -> bool {
        crate::shape::is_index_id(&self.id)
            && self.surface.is_some()
            && (self.decoder.is_some() || self.encoder.is_some())
            && self.round_trip
    }
}

/// A route through features and specimens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteRecord {
    /// Stable route id.
    pub id: RouteId,
    /// Human-facing title.
    pub title: String,
    /// Ordered route steps.
    pub steps: Vec<RouteStep>,
    /// Optional discovered documentation anchor for this route.
    pub doc_anchor: Option<AnchorId>,
}

/// One route step, targeting either a feature or an executable specimen.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RouteStep {
    /// Step targets a feature.
    Feature(FeatureId),
    /// Step targets a specimen.
    Specimen(SpecimenId),
}

/// Directed relationship between two index records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexEdge {
    /// Source index id.
    pub from: String,
    /// Relationship label, such as `contains`, `supports`, or `demonstrates`.
    pub rel: String,
    /// Target index id.
    pub to: String,
}

impl IndexEdge {
    /// Builds an edge from raw endpoint ids and a relationship label.
    pub fn new(from: impl Into<String>, rel: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            rel: rel.into(),
            to: to.into(),
        }
    }

    /// Builds a subject containment edge.
    pub fn contains(from: SubjectId, to: SubjectId) -> Self {
        Self::new(from.to_string(), "contains", to.to_string())
    }

    /// Builds a feature-to-feature relationship edge.
    pub fn relates(from: FeatureId, rel: impl Into<String>, to: FeatureId) -> Self {
        Self::new(from.to_string(), rel, to.to_string())
    }
}
