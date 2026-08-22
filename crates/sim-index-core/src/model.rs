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
    /// Source units examined while discovering anchors and declaration facts.
    pub source_units: Vec<SourceUnit>,
    /// Bounded public declaration facts attached to discovered anchors.
    pub declarations: Vec<DeclarationFact>,
    /// Protocol implementation relations attached to discovered anchors.
    pub protocol_relations: Vec<ProtocolRelation>,
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
            source_units: Vec::new(),
            declarations: Vec::new(),
            protocol_relations: Vec::new(),
            surfaces: Vec::new(),
            specimens: Vec::new(),
            drafts: Vec::new(),
            features: Vec::new(),
            routes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

/// Durable evidence for one repository-relative source unit considered by discovery.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SourceUnit {
    /// Subject whose discovery traversal considered this unit.
    pub subject: SubjectId,
    /// Stable repository-relative source path and source-unit identity.
    pub path: String,
    /// Whether this unit can contribute facts reachable from the public graph.
    pub reachability: SourceReachability,
    /// Outcome of the bounded source scan.
    pub completeness: SourceCompleteness,
    /// Bounded diagnostic explaining a non-complete outcome.
    pub reason: String,
    /// Bound applied while retaining source for inspection.
    pub retained_bound: SyntaxBound,
    /// Number of declaration positions visited in deterministic source order.
    pub declaration_count: usize,
}

/// Whether a scanned source unit can contribute public graph evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SourceReachability {
    /// The unit is reachable from a discovered public subject.
    Reachable,
    /// The unit is known but not reachable from the public graph.
    Unreachable,
}

/// Closed source-scan outcome vocabulary shared by graph producers and consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum SourceCompleteness {
    /// The entire unit was read and understood within its bound.
    Complete,
    /// The unit was readable but syntactically malformed.
    Malformed,
    /// The unit could not be read.
    Unreadable,
    /// The retained input exceeded its explicit bound.
    Truncated,
    /// The unit uses a source form the scanner does not support.
    Unsupported,
    /// The scanner could not resolve the unit to a stable source identity.
    Unresolved,
}

impl SourceCompleteness {
    /// Returns the stable codec label for this outcome.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Malformed => "malformed",
            Self::Unreadable => "unreadable",
            Self::Truncated => "truncated",
            Self::Unsupported => "unsupported",
            Self::Unresolved => "unresolved",
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

/// A bounded public declaration fact attached to an existing source anchor.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct DeclarationFact {
    /// Existing anchor that owns this declaration identity.
    pub anchor: AnchorId,
    /// Public source role of the declaration.
    pub role: DeclarationRole,
    /// Canonical module path relative to the crate root.
    pub module_path: String,
    /// Normalized generic declaration syntax.
    pub generics: String,
    /// Normalized public fields, variants, or alias target.
    pub members: Vec<String>,
    /// Stable repository-relative source location.
    pub location: SourceLocation,
    /// Bound applied to normalized syntax in this fact.
    pub syntax_bound: SyntaxBound,
}

/// Public source role represented by a declaration fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum DeclarationRole {
    /// Public constant.
    Const,
    /// Public enum.
    Enum,
    /// Public function.
    Function,
    /// Public module.
    Module,
    /// Public re-export.
    ReExport,
    /// Public static.
    Static,
    /// Public struct.
    Struct,
    /// Public trait.
    Trait,
    /// Public type alias.
    TypeAlias,
}

impl DeclarationRole {
    /// Returns the stable discovery label for this source role.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Enum => "enum",
            Self::Function => "function",
            Self::Module => "module",
            Self::ReExport => "re-export",
            Self::Static => "static",
            Self::Struct => "struct",
            Self::Trait => "trait",
            Self::TypeAlias => "type-alias",
        }
    }
}

/// Stable source location for a discovered declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SourceLocation {
    /// Repository-relative source file.
    pub file: String,
    /// Zero-based declaration ordinal in the scanner traversal.
    pub declaration: usize,
}

/// A semantic binding from SIM source to a host-provided capability.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct HostBindingFact {
    /// Existing source anchor that owns the binding.
    pub anchor: AnchorId,
    /// Host operation or dependency form that was discovered.
    pub kind: HostBindingKind,
    /// Structurally derived role of the source containing the binding.
    pub role: HostSourceRole,
    /// Cargo or foreign-language build target containing the source.
    pub target: String,
    /// Stable source location of the bound span.
    pub location: SourceLocation,
    /// True when the module graph places the binding below `cfg(test)`.
    pub test_member: bool,
    /// Canonical provider package or platform id, when resolution succeeded.
    pub provider: String,
    /// Resolved call, dependency edge, import, or artifact evidence.
    pub evidence: String,
    /// Required architectural move that removes or encapsulates the binding.
    pub normalization_move: String,
}

/// Closed semantic kinds of host binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HostBindingKind {
    /// A call to an operating-system or hardware API.
    Call,
    /// A dependency whose implementation can reach the host.
    Dependency,
    /// A native or foreign ABI declaration without a call.
    AbiDeclaration,
    /// A native ABI implementation exported by this source.
    ForeignImplementation,
    /// An imported symbol present in a built artifact.
    ArtifactImport,
    /// A process-command fallback.
    Subprocess,
}

impl HostBindingKind {
    /// Returns the stable generated-ledger label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::Dependency => "dependency",
            Self::AbiDeclaration => "abi-declaration",
            Self::ForeignImplementation => "foreign-implementation",
            Self::ArtifactImport => "artifact-import",
            Self::Subprocess => "subprocess",
        }
    }
}

/// Structurally derived relationship between source and host behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HostSourceRole {
    /// Product code expected to remain host-independent.
    Pure,
    /// A declared host capsule or concrete backend.
    Capsule,
    /// The single process/bootstrap boundary.
    Bootstrap,
    /// Build, development, or repository tooling.
    Tool,
    /// Test-only source derived from the module/target graph.
    Test,
}

impl HostSourceRole {
    /// Returns the stable generated-ledger label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pure => "pure",
            Self::Capsule => "capsule",
            Self::Bootstrap => "bootstrap",
            Self::Tool => "tool",
            Self::Test => "test",
        }
    }
}

/// Applied bound and truncation state for normalized declaration syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SyntaxBound {
    /// Maximum combined byte length of generics and members.
    pub max_bytes: usize,
    /// True when normalized syntax exceeded the bound and was omitted.
    pub truncated: bool,
}

/// A protocol implementation relation attached to an existing source anchor.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProtocolRelation {
    /// Existing anchor for the implementation reviewed by a human.
    pub anchor: AnchorId,
    /// Normalized implementing type spelling.
    pub implementor: String,
    /// Trait spelling found at the implementation site.
    pub source_spelling: String,
    /// Bounded normalized implementation-body fingerprint.
    pub body_fingerprint: String,
    /// Bound applied to the implementation-body fingerprint.
    pub body_bound: SyntaxBound,
    /// Honest lexical resolution result for the protocol.
    pub resolution: ProtocolResolution,
}

/// Resolution state for a protocol implementation relation.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ProtocolResolution {
    /// The protocol path resolved uniquely.
    Resolved {
        /// Canonical resolved protocol path.
        protocol: String,
    },
    /// Resolution was not unique or required unavailable metadata.
    Unresolved {
        /// Stable reason resolution could not be completed.
        reason: UnresolvedReason,
        /// Sorted unique candidates, when ambiguity produced candidates.
        candidates: Vec<String>,
    },
}

/// Stable reason a protocol relation remains unresolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum UnresolvedReason {
    /// Glob imports made the name ambiguous.
    AmbiguousGlobImport,
    /// More than one explicit candidate remained.
    AmbiguousName,
    /// Resolving the name requires external metadata.
    ExternalMetadataAbsent,
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
    /// Reader audiences this route is intended for.
    pub audiences: Vec<String>,
    /// Ordered route steps.
    pub steps: Vec<RouteStep>,
    /// Optional discovered documentation anchor for this route.
    pub doc_anchor: Option<AnchorId>,
}

/// One route step, targeting either a feature or an executable specimen.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RouteStep {
    /// Step targets a feature.
    Feature {
        /// Target feature id.
        id: FeatureId,
        /// Why this step belongs in the route.
        why: String,
    },
    /// Step targets a specimen.
    Specimen {
        /// Target specimen id.
        id: SpecimenId,
        /// Why this step belongs in the route.
        why: String,
    },
}

impl RouteStep {
    /// Returns the target id.
    pub fn id(&self) -> &str {
        match self {
            Self::Feature { id, .. } => id.as_str(),
            Self::Specimen { id, .. } => id.as_str(),
        }
    }

    /// Returns the target kind label.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Feature { .. } => "feature",
            Self::Specimen { .. } => "specimen",
        }
    }

    /// Returns the step rationale.
    pub fn why(&self) -> &str {
        match self {
            Self::Feature { why, .. } | Self::Specimen { why, .. } => why,
        }
    }
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
