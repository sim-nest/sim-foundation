//! Implementations of the pure study record contract.
//!
//! All identity is a canonical kernel [`Datum`]. Operational context is a
//! separate, identity-neutral type. Callers install whatever general-purpose
//! SIM codec they need for `Datum`; this crate creates no competing syntax or
//! JSON authority.

use crate::encoding::*;
use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};
use std::{collections::BTreeSet, error::Error, fmt};

pub(crate) const MAX_TEXT: usize = 4_096;
pub(crate) const MAX_ITEMS: usize = 16_384;
pub(crate) const VERSION: u32 = 1;

/// A validation, transition, decode, or export refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StudyError {
    UnknownSchemaVersion(u32),
    NoncanonicalRecord(&'static str),
    DuplicateField(String),
    MissingField(&'static str),
    WrongField(&'static str),
    BoundExceeded(&'static str),
    IncompatibleNumberDomain,
    MissingProvenance(ContentId),
    ConflictingTerminalOutcome,
    AlreadyTerminal,
    EvidenceStrengthened,
    PrivateExport,
    SecretForbidden,
    UnsafePath,
}

impl fmt::Display for StudyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for StudyError {}

/// Evidence strength. Later variants are weaker disclosure/proof classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceClass {
    Publishable,
    DeveloperBootstrap,
    PrivateLocal,
    ReportOnly,
}

impl EvidenceClass {
    /// Combines evidence without ever strengthening it.
    pub fn derive(items: impl IntoIterator<Item = Self>) -> Self {
        items.into_iter().max().unwrap_or(Self::ReportOnly)
    }
    /// Checks an explicitly requested derivation class.
    pub fn permits_derivation(self, derived: Self) -> Result<(), StudyError> {
        if derived >= self {
            Ok(())
        } else {
            Err(StudyError::EvidenceStrengthened)
        }
    }
    fn datum(self) -> Datum {
        Datum::Symbol(sym(match self {
            Self::Publishable => "publishable",
            Self::DeveloperBootstrap => "developer-bootstrap",
            Self::PrivateLocal => "private-local",
            Self::ReportOnly => "report-only",
        }))
    }
}

/// Disclosure rule attached to each record field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldClass {
    Public,
    PrivateLocal,
    SecretForbidden,
    DigestOnly,
}

/// An opaque revision identity. The subject payload cannot be obtained here.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubjectRevision(ContentId);
impl SubjectRevision {
    pub const fn new(id: ContentId) -> Self {
        Self(id)
    }
    pub const fn content_id(&self) -> &ContentId {
        &self.0
    }
}

/// The complete identity-bearing coordinate of one study sample.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StudyCoordinate {
    subject: SubjectRevision,
    task: ContentId,
    harness: ContentId,
    request: ContentId,
    treatment: ContentId,
    sample_index: u32,
}

impl StudyCoordinate {
    pub const fn new(
        subject: SubjectRevision,
        task: ContentId,
        harness: ContentId,
        request: ContentId,
        treatment: ContentId,
        sample_index: u32,
    ) -> Self {
        Self {
            subject,
            task,
            harness,
            request,
            treatment,
            sample_index,
        }
    }
    pub const fn subject(&self) -> &SubjectRevision {
        &self.subject
    }
    pub const fn task(&self) -> &ContentId {
        &self.task
    }
    pub const fn harness(&self) -> &ContentId {
        &self.harness
    }
    pub const fn request(&self) -> &ContentId {
        &self.request
    }
    pub const fn treatment(&self) -> &ContentId {
        &self.treatment
    }
    pub const fn sample_index(&self) -> u32 {
        self.sample_index
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "coordinate",
            vec![
                version(),
                field("subject", cid(self.subject.content_id())),
                field("task", cid(&self.task)),
                field("harness", cid(&self.harness)),
                field("request", cid(&self.request)),
                field("treatment", cid(&self.treatment)),
                field("sample-index", u32_datum(self.sample_index)),
            ],
        )
    }
    pub fn content_id(&self) -> Result<ContentId, StudyError> {
        self.to_datum()
            .content_id()
            .map_err(|_| StudyError::NoncanonicalRecord("coordinate"))
    }
    pub fn from_datum(value: &Datum) -> Result<Self, StudyError> {
        let fields = exact_fields(
            value,
            "coordinate",
            &[
                "v",
                "subject",
                "task",
                "harness",
                "request",
                "treatment",
                "sample-index",
            ],
        )?;
        Ok(Self::new(
            SubjectRevision::new(read_cid(fields[1], "subject")?),
            read_cid(fields[2], "task")?,
            read_cid(fields[3], "harness")?,
            read_cid(fields[4], "request")?,
            read_cid(fields[5], "treatment")?,
            read_u32(fields[6], "sample-index")?,
        ))
    }
}

/// Operational information that is explicitly outside coordinate identity.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperationalContext {
    pub safe_path: Option<String>,
    pub timestamp: Option<String>,
    pub retry_policy: Option<String>,
    pub placement: Option<String>,
}
impl OperationalContext {
    pub fn validate(&self) -> Result<(), StudyError> {
        if let Some(path) = &self.safe_path {
            validate_relative_path(path)?;
        }
        for value in [&self.timestamp, &self.retry_policy, &self.placement]
            .into_iter()
            .flatten()
        {
            bounded(value, "operational text")?;
        }
        Ok(())
    }
}

/// Four exhaustive terminal outcome classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AttemptOutcome {
    Observed,
    Unsupported,
    Unresolved,
    Quarantined,
}
impl AttemptOutcome {
    fn datum(self) -> Datum {
        Datum::Symbol(sym(match self {
            Self::Observed => "observed",
            Self::Unsupported => "unsupported",
            Self::Unresolved => "unresolved",
            Self::Quarantined => "quarantined",
        }))
    }
}

/// An attempt before or after its single terminal transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub coordinate: StudyCoordinate,
    pub revision: u32,
    pub evidence: EvidenceClass,
    outcome: Option<AttemptOutcome>,
}
impl Attempt {
    pub const fn new(coordinate: StudyCoordinate, revision: u32, evidence: EvidenceClass) -> Self {
        Self {
            coordinate,
            revision,
            evidence,
            outcome: None,
        }
    }
    pub const fn outcome(&self) -> Option<AttemptOutcome> {
        self.outcome
    }
    pub fn transition(&mut self, outcome: AttemptOutcome) -> Result<(), StudyError> {
        match self.outcome {
            None => {
                self.outcome = Some(outcome);
                Ok(())
            }
            Some(old) if old == outcome => Err(StudyError::AlreadyTerminal),
            Some(_) => Err(StudyError::ConflictingTerminalOutcome),
        }
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "attempt",
            vec![
                version(),
                field("coordinate", self.coordinate.to_datum()),
                field("revision", u32_datum(self.revision)),
                field("evidence", self.evidence.datum()),
                field(
                    "outcome",
                    self.outcome.map_or(Datum::Nil, AttemptOutcome::datum),
                ),
            ],
        )
    }
}

/// A named observed facet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FacetObservation {
    pub coordinate: ContentId,
    pub facet: Symbol,
    pub value: Datum,
    pub evidence: EvidenceClass,
}
/// A resource quantity attributed to an attempt or observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceEvent {
    pub coordinate: ContentId,
    pub resource: Symbol,
    pub amount: NumberLiteral,
    pub evidence: EvidenceClass,
}
/// A sealed treatment description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreatmentRecord {
    pub treatment: ContentId,
    pub contract: ContentId,
    pub evidence: EvidenceClass,
}
/// A point and interval in exactly one number domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimateRecord {
    pub point: NumberLiteral,
    pub lower: NumberLiteral,
    pub upper: NumberLiteral,
    pub confidence: NumberLiteral,
    pub inference: ContentId,
    pub evidence: EvidenceClass,
}
impl EstimateRecord {
    pub fn new(
        point: NumberLiteral,
        lower: NumberLiteral,
        upper: NumberLiteral,
        confidence: NumberLiteral,
        inference: ContentId,
        evidence: EvidenceClass,
    ) -> Result<Self, StudyError> {
        let domain = &point.domain;
        if [&lower, &upper, &confidence]
            .iter()
            .any(|v| &v.domain != domain)
        {
            return Err(StudyError::IncompatibleNumberDomain);
        }
        for value in [&point, &lower, &upper, &confidence] {
            validate_number(value)?;
        }
        Ok(Self {
            point,
            lower,
            upper,
            confidence,
            inference,
            evidence,
        })
    }
    pub fn to_datum(&self) -> Datum {
        node(
            "estimate",
            vec![
                version(),
                field("point", Datum::Number(self.point.clone())),
                field("lower", Datum::Number(self.lower.clone())),
                field("upper", Datum::Number(self.upper.clone())),
                field("confidence", Datum::Number(self.confidence.clone())),
                field("inference", cid(&self.inference)),
                field("evidence-class", self.evidence.datum()),
            ],
        )
    }
}

/// A policy decision over a closed set of evidence identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionRecord {
    pub policy: ContentId,
    pub evidence_ids: Vec<ContentId>,
    pub frontier: Vec<ContentId>,
    pub verdict: Symbol,
    pub decisive_dimensions: Vec<Symbol>,
    pub expiry: Option<ContentId>,
    pub evidence: EvidenceClass,
}
impl DecisionRecord {
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the sealed DecisionRecord data contract plus its provenance closure"
    )]
    pub fn new(
        policy: ContentId,
        evidence_ids: Vec<ContentId>,
        frontier: Vec<ContentId>,
        verdict: Symbol,
        decisive_dimensions: Vec<Symbol>,
        expiry: Option<ContentId>,
        evidence: EvidenceClass,
        provenance_closure: &BTreeSet<ContentId>,
    ) -> Result<Self, StudyError> {
        bounded_items(&evidence_ids, "decision evidence")?;
        bounded_items(&frontier, "frontier")?;
        bounded_items(&decisive_dimensions, "dimensions")?;
        for id in evidence_ids.iter().chain(frontier.iter()) {
            if !provenance_closure.contains(id) {
                return Err(StudyError::MissingProvenance(id.clone()));
            }
        }
        Ok(Self {
            policy,
            evidence_ids,
            frontier,
            verdict,
            decisive_dimensions,
            expiry,
            evidence,
        })
    }
}
/// A selected subject under a named decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectionRecord {
    pub decision: ContentId,
    pub subject: SubjectRevision,
    pub role: Symbol,
    pub evidence: EvidenceClass,
}

/// Canonical record projection and public export enforcement.
pub trait StudyRecord {
    fn to_datum(&self) -> Datum;
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)];
    fn evidence_class(&self) -> EvidenceClass;
    fn content_id(&self) -> Result<ContentId, StudyError> {
        self.to_datum()
            .content_id()
            .map_err(|_| StudyError::NoncanonicalRecord("record"))
    }
    fn export_public(&self) -> Result<Datum, StudyError> {
        if self.evidence_class() == EvidenceClass::PrivateLocal
            || self
                .field_classes()
                .iter()
                .any(|(_, class)| *class == FieldClass::PrivateLocal)
        {
            return Err(StudyError::PrivateExport);
        }
        if contains_secret_shape(&self.to_datum()) {
            return Err(StudyError::SecretForbidden);
        }
        Ok(self.to_datum())
    }
}

impl StudyRecord for FacetObservation {
    fn to_datum(&self) -> Datum {
        node(
            "facet",
            vec![
                version(),
                field("coordinate", cid(&self.coordinate)),
                field("facet", Datum::Symbol(self.facet.clone())),
                field("value", self.value.clone()),
                field("evidence-class", self.evidence.datum()),
            ],
        )
    }
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)] {
        &[
            ("coordinate", FieldClass::DigestOnly),
            ("facet", FieldClass::Public),
            ("value", FieldClass::Public),
            ("evidence-class", FieldClass::Public),
        ]
    }
    fn evidence_class(&self) -> EvidenceClass {
        self.evidence
    }
}
impl StudyRecord for ResourceEvent {
    fn to_datum(&self) -> Datum {
        node(
            "resource",
            vec![
                version(),
                field("coordinate", cid(&self.coordinate)),
                field("resource", Datum::Symbol(self.resource.clone())),
                field("amount", Datum::Number(self.amount.clone())),
                field("evidence-class", self.evidence.datum()),
            ],
        )
    }
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)] {
        &[
            ("coordinate", FieldClass::DigestOnly),
            ("resource", FieldClass::Public),
            ("amount", FieldClass::Public),
            ("evidence-class", FieldClass::Public),
        ]
    }
    fn evidence_class(&self) -> EvidenceClass {
        self.evidence
    }
}
impl StudyRecord for TreatmentRecord {
    fn to_datum(&self) -> Datum {
        node(
            "treatment",
            vec![
                version(),
                field("treatment", cid(&self.treatment)),
                field("contract", cid(&self.contract)),
                field("evidence-class", self.evidence.datum()),
            ],
        )
    }
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)] {
        &[
            ("treatment", FieldClass::DigestOnly),
            ("contract", FieldClass::DigestOnly),
            ("evidence-class", FieldClass::Public),
        ]
    }
    fn evidence_class(&self) -> EvidenceClass {
        self.evidence
    }
}
impl StudyRecord for EstimateRecord {
    fn to_datum(&self) -> Datum {
        EstimateRecord::to_datum(self)
    }
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)] {
        &[
            ("point", FieldClass::Public),
            ("lower", FieldClass::Public),
            ("upper", FieldClass::Public),
            ("confidence", FieldClass::Public),
            ("inference", FieldClass::DigestOnly),
            ("evidence-class", FieldClass::Public),
        ]
    }
    fn evidence_class(&self) -> EvidenceClass {
        self.evidence
    }
}
impl StudyRecord for DecisionRecord {
    fn to_datum(&self) -> Datum {
        node(
            "decision",
            vec![
                version(),
                field("policy", cid(&self.policy)),
                field(
                    "evidence",
                    Datum::Vector(self.evidence_ids.iter().map(cid).collect()),
                ),
                field(
                    "frontier",
                    Datum::Vector(self.frontier.iter().map(cid).collect()),
                ),
                field("verdict", Datum::Symbol(self.verdict.clone())),
                field(
                    "decisive-dimensions",
                    Datum::Vector(
                        self.decisive_dimensions
                            .iter()
                            .cloned()
                            .map(Datum::Symbol)
                            .collect(),
                    ),
                ),
                field("expiry", self.expiry.as_ref().map_or(Datum::Nil, cid)),
                field("evidence-class", self.evidence.datum()),
            ],
        )
    }
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)] {
        &[
            ("policy", FieldClass::DigestOnly),
            ("evidence", FieldClass::DigestOnly),
            ("frontier", FieldClass::DigestOnly),
            ("verdict", FieldClass::Public),
            ("decisive-dimensions", FieldClass::Public),
            ("expiry", FieldClass::DigestOnly),
            ("evidence-class", FieldClass::Public),
        ]
    }
    fn evidence_class(&self) -> EvidenceClass {
        self.evidence
    }
}
impl StudyRecord for SelectionRecord {
    fn to_datum(&self) -> Datum {
        node(
            "selection",
            vec![
                version(),
                field("decision", cid(&self.decision)),
                field("subject", cid(self.subject.content_id())),
                field("role", Datum::Symbol(self.role.clone())),
                field("evidence-class", self.evidence.datum()),
            ],
        )
    }
    fn field_classes(&self) -> &'static [(&'static str, FieldClass)] {
        &[
            ("decision", FieldClass::DigestOnly),
            ("subject", FieldClass::DigestOnly),
            ("role", FieldClass::Public),
            ("evidence-class", FieldClass::Public),
        ]
    }
    fn evidence_class(&self) -> EvidenceClass {
        self.evidence
    }
}

/// Strict shape for canonical study nodes, suitable for codec admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordShape {
    pub name: &'static str,
    pub fields: &'static [&'static str],
}
impl RecordShape {
    pub fn check(&self, datum: &Datum) -> Result<(), StudyError> {
        exact_fields(datum, self.name, self.fields).map(|_| ())
    }
}
pub const COORDINATE_SHAPE: RecordShape = RecordShape {
    name: "coordinate",
    fields: &[
        "v",
        "subject",
        "task",
        "harness",
        "request",
        "treatment",
        "sample-index",
    ],
};
pub const ESTIMATE_SHAPE: RecordShape = RecordShape {
    name: "estimate",
    fields: &[
        "v",
        "point",
        "lower",
        "upper",
        "confidence",
        "inference",
        "evidence-class",
    ],
};
