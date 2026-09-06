//! General bounded-work values.

use sim_conformance_core::{EvidenceSetId, IdKind, SemanticId, StorageId};
use sim_kernel::{Datum, Symbol};

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
    pub id: WorkId,
    /// Exact semantic inputs.
    pub semantic_inputs: Vec<SemanticInputId>,
    /// Input/output ceilings.
    pub input_budget: InputBudget,
    /// Output Shape identity.
    pub output_shape: sim_conformance_core::OutputShapeId,
    /// Explicit resources.
    pub resources: Vec<ResourceNeed>,
    /// Explicit capability grant.
    pub effects: CapabilityGrant,
    /// Progress contract.
    pub progress: ProgressContract,
    /// Attempt policy.
    pub attempts: AttemptPolicy,
    /// Strict descent evidence.
    pub descent: DescentCertificate,
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
