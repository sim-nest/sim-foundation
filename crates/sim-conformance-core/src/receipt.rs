//! Immutable checker receipts and revocation verification.

use sim_kernel::Datum;

use crate::{
    CheckInvocation, CheckInvocationId, CheckScopeId, CheckedSubjectId, CheckerBinding,
    CheckerBindingId, CheckerReceiptId, CheckerResultId, ConformanceError, ConformancePackId,
    EvidenceProvenanceId, EvidenceSetId, PolicyId, ProofCodeId, SemanticId, field, text,
};

/// Strength of one immutable checker result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EvidenceGrade {
    /// Deterministic local bootstrap evidence without cross-world reuse authority.
    Bootstrap,
    /// Independently checked reproducible evidence.
    Reproducible,
    /// Evidence accepted for release.
    Release,
}

impl EvidenceGrade {
    fn datum(self) -> Datum {
        text(match self {
            Self::Bootstrap => "bootstrap",
            Self::Reproducible => "reproducible",
            Self::Release => "release",
        })
    }
}

/// Canonical revocation state supplied by the checker's owner policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevocationStatus {
    /// No authoritative revocation state was available.
    Unknown,
    /// The exact checker code and pack version remain current.
    Current,
    /// The result is revoked.
    Revoked,
}

/// Immutable receipt tied to one exact invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerReceipt {
    id: CheckerReceiptId,
    binding: CheckerBindingId,
    invocation: CheckInvocationId,
    checker_code: ProofCodeId,
    pack: ConformancePackId,
    scope: CheckScopeId,
    subject: CheckedSubjectId,
    result: CheckerResultId,
    grade: EvidenceGrade,
    provenance: EvidenceProvenanceId,
    policy: PolicyId,
    support: EvidenceSetId,
}

impl CheckerReceipt {
    /// Constructs and verifies a receipt against its exact invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn passing(
        invocation: &CheckInvocation,
        result: CheckerResultId,
        grade: EvidenceGrade,
        provenance: EvidenceProvenanceId,
        policy: PolicyId,
        support: EvidenceSetId,
        revocation: RevocationStatus,
    ) -> Result<Self, ConformanceError> {
        if revocation != RevocationStatus::Current {
            return Err(ConformanceError::RevocationUnknownOrActive);
        }
        let id = SemanticId::from_fields(vec![
            field("binding", invocation.binding().to_datum())?,
            field("invocation", invocation.id().to_datum())?,
            field("checker-code", invocation.checker_code().to_datum())?,
            field("pack", invocation.pack().to_datum())?,
            field("scope", invocation.scope().to_datum())?,
            field("subject", invocation.subject().to_datum())?,
            field("result", result.to_datum())?,
            field("grade", grade.datum())?,
            field("provenance", provenance.to_datum())?,
            field("policy", policy.to_datum())?,
            field("support", support.to_datum())?,
        ])?;
        Ok(Self {
            id,
            binding: invocation.binding().clone(),
            invocation: invocation.id().clone(),
            checker_code: invocation.checker_code().clone(),
            pack: invocation.pack().clone(),
            scope: invocation.scope().clone(),
            subject: invocation.subject().clone(),
            result,
            grade,
            provenance,
            policy,
            support,
        })
    }

    /// Verifies every copied invocation field and exact scope.
    pub fn verify(
        &self,
        binding: &CheckerBinding,
        invocation: &CheckInvocation,
        revocation: RevocationStatus,
    ) -> Result<(), ConformanceError> {
        if revocation != RevocationStatus::Current {
            return Err(ConformanceError::RevocationUnknownOrActive);
        }
        if &self.binding != binding.id() || &self.invocation != invocation.id() {
            return Err(ConformanceError::InvocationMismatch(
                "binding or invocation",
            ));
        }
        if &self.checker_code != invocation.checker_code()
            || &self.pack != invocation.pack()
            || &self.scope != invocation.scope()
            || &self.subject != invocation.subject()
        {
            return Err(ConformanceError::InvocationMismatch(
                "copied invocation fields",
            ));
        }
        if !binding.allowed_scopes().contains(&self.scope) {
            return Err(ConformanceError::UnauthorizedScope);
        }
        Ok(())
    }

    /// Returns the receipt identity.
    pub const fn id(&self) -> &CheckerReceiptId {
        &self.id
    }

    /// Returns the static checker binding identity.
    pub const fn binding(&self) -> &CheckerBindingId {
        &self.binding
    }

    /// Returns the exact invocation identity.
    pub const fn invocation(&self) -> &CheckInvocationId {
        &self.invocation
    }

    /// Returns the exact checker code identity.
    pub const fn checker_code(&self) -> &ProofCodeId {
        &self.checker_code
    }

    /// Returns the conformance pack identity.
    pub const fn pack(&self) -> &ConformancePackId {
        &self.pack
    }

    /// Returns the authorized scope identity.
    pub const fn scope(&self) -> &CheckScopeId {
        &self.scope
    }

    /// Returns the checked subject identity.
    pub const fn subject(&self) -> &CheckedSubjectId {
        &self.subject
    }

    /// Returns the checker's passing result identity.
    pub const fn result(&self) -> &CheckerResultId {
        &self.result
    }

    /// Returns the evidence grade.
    pub const fn grade(&self) -> EvidenceGrade {
        self.grade
    }

    /// Returns the execution provenance identity.
    pub const fn provenance(&self) -> &EvidenceProvenanceId {
        &self.provenance
    }

    /// Returns the checker-owner policy identity.
    pub const fn policy(&self) -> &PolicyId {
        &self.policy
    }

    /// Returns the acyclic supporting-evidence set identity.
    pub const fn support(&self) -> &EvidenceSetId {
        &self.support
    }
}
