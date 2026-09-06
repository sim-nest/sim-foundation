//! General bounded-work values.

use sim_conformance_core::{ConformanceError, EvidenceSetId, IdKind, SemanticId, StorageId};
use sim_kernel::{Datum, NumberLiteral, Symbol};

/// Semantic work identity kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkKind;
impl IdKind for WorkKind {
    const DOMAIN: &'static str = "work/envelope-v1";
}
/// Semantic identity of one work envelope.
pub type WorkId = SemanticId<WorkKind>;

/// Semantic input identity kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticInputKind;
impl IdKind for SemanticInputKind {
    const DOMAIN: &'static str = "work/semantic-input-v1";
}
/// Identity of one exact domain-tagged input value.
pub type SemanticInputId = SemanticId<SemanticInputKind>;

/// Explicit input and output ceilings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputBudget {
    /// Maximum encoded input bytes.
    pub bytes: u64,
    /// Maximum source files or objects.
    pub files: u32,
    /// Maximum model-token estimate where relevant.
    pub tokens: u64,
    /// Maximum encoded output bytes.
    pub output_bytes: u64,
}

impl InputBudget {
    /// Rejects a measured input without truncating it.
    pub fn admits(&self, bytes: u64, files: u32, tokens: u64) -> bool {
        bytes <= self.bytes && files <= self.files && tokens <= self.tokens
    }
}

/// One typed resource prerequisite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceNeed {
    /// Open resource class.
    pub kind: Symbol,
    /// Exact amount or capability detail.
    pub requirement: Datum,
}

/// Capabilities explicitly available to the worker.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilityGrant {
    /// Ordered, duplicate-free capability names.
    pub capabilities: Vec<Symbol>,
}

/// Required progress publication contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressContract {
    /// Maximum work units between progress beats.
    pub beat_work_units: u64,
    /// Maximum number of beats.
    pub max_beats: u32,
}

/// Retry policy with a hard attempt ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptPolicy {
    /// Total attempts including the first.
    pub max_attempts: u32,
    /// Whether one malformed return may receive a fresh narrower attempt.
    pub retry_malformed_once: bool,
}

/// Proof that a retry strictly narrows unresolved work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescentCertificate {
    /// Measure used for comparison.
    pub measure: Symbol,
    /// Previous unresolved measure.
    pub before: u64,
    /// New unresolved measure.
    pub after: u64,
}

impl DescentCertificate {
    /// Accepts only strict descent.
    pub fn verify(&self) -> Result<(), WorkError> {
        if self.after < self.before {
            Ok(())
        } else {
            Err(WorkError::NoDescent)
        }
    }
}

/// Pure bounded-work declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkEnvelope {
    /// Canonical work identity.
    id: WorkId,
    /// Exact semantic inputs.
    semantic_inputs: Vec<SemanticInputId>,
    /// Input/output ceilings.
    input_budget: InputBudget,
    /// Output Shape identity.
    output_shape: sim_conformance_core::OutputShapeId,
    /// Explicit resources.
    resources: Vec<ResourceNeed>,
    /// Explicit capability grant.
    effects: CapabilityGrant,
    /// Progress contract.
    progress: ProgressContract,
    /// Attempt policy.
    attempts: AttemptPolicy,
    /// Strict descent evidence.
    descent: DescentCertificate,
}

impl WorkEnvelope {
    /// Validates and identifies one bounded work declaration.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        semantic_inputs: Vec<SemanticInputId>,
        input_budget: InputBudget,
        output_shape: sim_conformance_core::OutputShapeId,
        resources: Vec<ResourceNeed>,
        effects: CapabilityGrant,
        progress: ProgressContract,
        attempts: AttemptPolicy,
        descent: DescentCertificate,
    ) -> Result<Self, WorkError> {
        if input_budget.output_bytes == 0
            || progress.beat_work_units == 0
            || progress.max_beats == 0
            || attempts.max_attempts == 0
        {
            return Err(WorkError::InvalidPacket("work bounds"));
        }
        if effects
            .capabilities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(WorkError::InvalidPacket("capability order"));
        }
        descent.verify()?;
        let id = SemanticId::from_fields(vec![
            wfield(
                "semantic-inputs",
                Datum::Vector(semantic_inputs.iter().map(SemanticId::to_datum).collect()),
            )?,
            wfield("input-budget", budget_datum(input_budget))?,
            wfield("output-shape", output_shape.to_datum())?,
            wfield(
                "resources",
                Datum::Vector(
                    resources
                        .iter()
                        .map(resource_datum)
                        .collect::<Result<_, _>>()?,
                ),
            )?,
            wfield(
                "effects",
                Datum::Vector(
                    effects
                        .capabilities
                        .iter()
                        .cloned()
                        .map(Datum::Symbol)
                        .collect(),
                ),
            )?,
            wfield(
                "progress",
                Datum::Vector(vec![
                    number(progress.beat_work_units),
                    number(u64::from(progress.max_beats)),
                ]),
            )?,
            wfield(
                "attempts",
                Datum::Vector(vec![
                    number(u64::from(attempts.max_attempts)),
                    Datum::Bool(attempts.retry_malformed_once),
                ]),
            )?,
            wfield(
                "descent",
                Datum::Vector(vec![
                    Datum::Symbol(descent.measure.clone()),
                    number(descent.before),
                    number(descent.after),
                ]),
            )?,
        ])
        .map_err(map_conformance)?;
        Ok(Self {
            id,
            semantic_inputs,
            input_budget,
            output_shape,
            resources,
            effects,
            progress,
            attempts,
            descent,
        })
    }

    /// Returns the canonical work identity.
    pub const fn id(&self) -> &WorkId {
        &self.id
    }

    /// Returns the exact semantic inputs in identity-bearing order.
    pub fn semantic_inputs(&self) -> &[SemanticInputId] {
        &self.semantic_inputs
    }

    /// Returns the input and output ceilings.
    pub const fn input_budget(&self) -> InputBudget {
        self.input_budget
    }

    /// Returns the output Shape identity.
    pub const fn output_shape(&self) -> &sim_conformance_core::OutputShapeId {
        &self.output_shape
    }

    /// Returns the explicit resource prerequisites.
    pub fn resources(&self) -> &[ResourceNeed] {
        &self.resources
    }

    /// Returns the explicit capability grant.
    pub const fn effects(&self) -> &CapabilityGrant {
        &self.effects
    }

    /// Returns the progress publication contract.
    pub const fn progress(&self) -> ProgressContract {
        self.progress
    }

    /// Returns the bounded attempt policy.
    pub const fn attempts(&self) -> AttemptPolicy {
        self.attempts
    }

    /// Returns the strict descent evidence.
    pub const fn descent(&self) -> &DescentCertificate {
        &self.descent
    }
}

/// Typed outcome of bounded work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkReturn {
    /// Output decoded and matched its exact Shape.
    Complete {
        /// Checked value.
        value: Datum,
        /// Evidence returned with the value.
        evidence: EvidenceSetId,
    },
    /// Honest partial progress with explicit remaining needs.
    Incomplete {
        /// Checked partial value.
        partial: Datum,
        /// Remaining typed needs.
        remaining: Vec<ResourceNeed>,
    },
    /// Opaque bytes that did not decode or match.
    Malformed {
        /// Raw byte address, deliberately not semantic identity.
        raw: StorageId,
        /// Stable violations.
        violations: Vec<String>,
    },
}

/// Typed packet or work refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkError {
    /// Input files exceeded the declared ceiling.
    FileBudget,
    /// Encoded input bytes exceeded the declared ceiling.
    ByteBudget,
    /// Estimated tokens exceeded the declared ceiling.
    TokenBudget,
    /// Output bytes exceeded the declared ceiling.
    OutputBudget,
    /// Retry scope did not strictly decrease.
    NoDescent,
    /// A source was absent or not declared.
    UndeclaredInput(String),
    /// A source location did not verify.
    SourceDigest,
    /// Dependency/target qualification failed.
    Qualification(String),
    /// One packet field was empty, duplicated, or inconsistent.
    InvalidPacket(&'static str),
    /// Return bytes could not be decoded.
    MalformedReturn(Vec<String>),
}

impl std::fmt::Display for WorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for WorkError {}

fn resource_datum(value: &ResourceNeed) -> Result<Datum, WorkError> {
    Ok(Datum::Node {
        tag: Symbol::qualified("work", "resource-need-v1"),
        fields: vec![
            wfield("kind", Datum::Symbol(value.kind.clone()))?,
            wfield("requirement", value.requirement.clone())?,
        ],
    })
}

fn budget_datum(value: InputBudget) -> Datum {
    Datum::Vector(vec![
        number(value.bytes),
        number(u64::from(value.files)),
        number(value.tokens),
        number(value.output_bytes),
    ])
}

fn number(value: u64) -> Datum {
    Datum::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "u64"),
        canonical: value.to_string(),
    })
}

fn wfield(name: &str, value: Datum) -> Result<(Symbol, Datum), WorkError> {
    Symbol::checked(name)
        .map(|name| (Symbol::qualified("work", name.name), value))
        .map_err(|_| WorkError::InvalidPacket("work identity field"))
}

fn map_conformance(error: ConformanceError) -> WorkError {
    WorkError::Qualification(error.to_string())
}
