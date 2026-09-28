// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Immutable checker receipts and revocation verification.

use sim_kernel::Datum;

use crate::{
    CheckInvocation, CheckInvocationId, CheckScopeId, CheckedSubjectId, CheckerBinding,
    CheckerBindingId, CheckerReceiptId, CheckerResultId, CheckerRevocationKeyId,
    CheckerRevocationObservation, CheckerRevocationSelection, ConformanceError, ConformancePackId,
    EvidenceProvenanceId, EvidenceSetId, OwnerBindingId, PolicyId, ProofCodeId, RevocationSourceId,
    RevocationStatus, SemanticId, field, text,
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
    revocation_issuer: OwnerBindingId,
    revocation_source: RevocationSourceId,
    revocation_key: CheckerRevocationKeyId,
}

impl CheckerReceipt {
    /// Constructs and verifies a receipt against its exact invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn passing(
        binding: &CheckerBinding,
        invocation: &CheckInvocation,
        result: CheckerResultId,
        grade: EvidenceGrade,
        provenance: EvidenceProvenanceId,
        policy: PolicyId,
        support: EvidenceSetId,
        revocation: &CheckerRevocationSelection,
    ) -> Result<Self, ConformanceError> {
        if revocation.status() != RevocationStatus::Current {
            return Err(ConformanceError::RevocationUnknownOrActive);
        }
        if invocation.binding() != binding.id() {
            return Err(ConformanceError::InvocationMismatch("binding"));
        }
        if revocation.issuer() != binding.owner() {
            return Err(ConformanceError::InvocationMismatch("revocation issuer"));
        }
        if revocation.source() != binding.revocation_source()
            || revocation.key().source() != binding.revocation_source()
        {
            return Err(ConformanceError::InvocationMismatch("revocation source"));
        }
        if revocation.key().binding() != invocation.binding()
            || revocation.key().subject() != invocation.subject()
            || revocation.key().checker_code() != invocation.checker_code()
            || revocation.key().pack() != invocation.pack()
            || revocation.key().scope() != invocation.scope()
            || revocation.key().input_closure() != invocation.input_closure()
        {
            return Err(ConformanceError::InvocationMismatch("revocation key"));
        }
        if revocation.policy() != &policy {
            return Err(ConformanceError::InvocationMismatch("revocation policy"));
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
            field("revocation-issuer", revocation.issuer().to_datum())?,
            field("revocation-source", revocation.source().to_datum())?,
            field("revocation-key", revocation.key().id().to_datum())?,
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
            revocation_issuer: revocation.issuer().clone(),
            revocation_source: revocation.source().clone(),
            revocation_key: revocation.key().id().clone(),
        })
    }

    /// Verifies every copied invocation field and exact scope.
    pub fn verify(
        &self,
        binding: &CheckerBinding,
        invocation: &CheckInvocation,
        revocation: &CheckerRevocationObservation,
    ) -> Result<(), ConformanceError> {
        if revocation.status() != RevocationStatus::Current {
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
        if revocation.issuer() != binding.owner() || &self.revocation_issuer != revocation.issuer()
        {
            return Err(ConformanceError::InvocationMismatch("revocation issuer"));
        }
        if revocation.source() != binding.revocation_source()
            || &self.revocation_source != revocation.source()
        {
            return Err(ConformanceError::InvocationMismatch("revocation source"));
        }
        if revocation.receipt() != &self.id {
            return Err(ConformanceError::InvocationMismatch("revocation receipt"));
        }
        if revocation.key().id() != &self.revocation_key
            || revocation.key().binding() != invocation.binding()
            || revocation.key().subject() != invocation.subject()
            || revocation.key().checker_code() != invocation.checker_code()
            || revocation.key().pack() != invocation.pack()
            || revocation.key().scope() != invocation.scope()
            || revocation.key().input_closure() != invocation.input_closure()
        {
            return Err(ConformanceError::InvocationMismatch("revocation key"));
        }
        if revocation.policy() != &self.policy {
            return Err(ConformanceError::InvocationMismatch("revocation policy"));
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

    /// Returns the checker-owner binding that issued the selected revocation set.
    pub const fn revocation_issuer(&self) -> &OwnerBindingId {
        &self.revocation_issuer
    }

    /// Returns the exact selected revocation source.
    pub const fn revocation_source(&self) -> &RevocationSourceId {
        &self.revocation_source
    }

    /// Returns the pre-receipt lookup-key identity.
    pub const fn revocation_key(&self) -> &CheckerRevocationKeyId {
        &self.revocation_key
    }
}
