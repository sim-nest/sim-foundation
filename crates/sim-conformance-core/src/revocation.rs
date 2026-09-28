// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Canonical owner-issued checker revocation data and lookup semantics.

use std::collections::BTreeMap;

use sim_kernel::Datum;

use crate::{
    CheckInputClosureId, CheckInvocation, CheckScopeId, CheckedSubjectId, CheckerBindingId,
    CheckerReceiptId, CheckerRevocationHeadId, CheckerRevocationKeyId, CheckerRevocationSetId,
    ConformanceError, ConformancePackId, OwnerBindingId, PolicyId, ProofCodeId, RevocationSourceId,
    SemanticId, field, text,
};

/// Canonical revocation state transported from the checker's owner policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevocationStatus {
    /// The selected owner set contains no decision for the exact key.
    Unknown,
    /// The selected owner set explicitly marks the exact key current.
    Current,
    /// The selected owner set explicitly revokes the exact key.
    Revoked,
}

impl RevocationStatus {
    fn datum(self) -> Datum {
        text(match self {
            Self::Unknown => "unknown",
            Self::Current => "current",
            Self::Revoked => "revoked",
        })
    }
}

/// Exact pre-receipt key selected by a checker owner's revocation policy.
///
/// The key deliberately excludes a receipt id. It can therefore be resolved
/// before receipt construction without creating an identity cycle. It
/// includes every field an owner might reasonably revoke by (binding,
/// subject, checker code, pack, scope and input closure) so that two
/// invocations differing only in scope or input never collide on the same
/// key: an owner decision made against one invocation must never be visible
/// to a distinct invocation that merely shares a subject and checker.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckerRevocationKey {
    id: CheckerRevocationKeyId,
    source: RevocationSourceId,
    binding: CheckerBindingId,
    subject: CheckedSubjectId,
    checker_code: ProofCodeId,
    pack: ConformancePackId,
    scope: CheckScopeId,
    input_closure: CheckInputClosureId,
}

impl CheckerRevocationKey {
    /// Constructs the exact key for an invocation and owner-selected source.
    pub fn for_invocation(
        source: RevocationSourceId,
        invocation: &CheckInvocation,
    ) -> Result<Self, ConformanceError> {
        Self::new(
            source,
            invocation.binding().clone(),
            invocation.subject().clone(),
            invocation.checker_code().clone(),
            invocation.pack().clone(),
            invocation.scope().clone(),
            invocation.input_closure().clone(),
        )
    }

    /// Constructs an exact key without requiring a concrete invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: RevocationSourceId,
        binding: CheckerBindingId,
        subject: CheckedSubjectId,
        checker_code: ProofCodeId,
        pack: ConformancePackId,
        scope: CheckScopeId,
        input_closure: CheckInputClosureId,
    ) -> Result<Self, ConformanceError> {
        let id = SemanticId::from_fields(vec![
            field("revocation-source", source.to_datum())?,
            field("binding", binding.to_datum())?,
            field("subject", subject.to_datum())?,
            field("checker-code", checker_code.to_datum())?,
            field("pack", pack.to_datum())?,
            field("scope", scope.to_datum())?,
            field("input-closure", input_closure.to_datum())?,
        ])?;
        Ok(Self {
            id,
            source,
            binding,
            subject,
            checker_code,
            pack,
            scope,
            input_closure,
        })
    }

    /// Returns this key's canonical identity.
    pub const fn id(&self) -> &CheckerRevocationKeyId {
        &self.id
    }

    /// Returns the authoritative source named by this key.
    pub const fn source(&self) -> &RevocationSourceId {
        &self.source
    }

    /// Returns the exact checker binding.
    pub const fn binding(&self) -> &CheckerBindingId {
        &self.binding
    }

    /// Returns the exact checked subject.
    pub const fn subject(&self) -> &CheckedSubjectId {
        &self.subject
    }

    /// Returns the exact checker implementation.
    pub const fn checker_code(&self) -> &ProofCodeId {
        &self.checker_code
    }

    /// Returns the exact conformance pack.
    pub const fn pack(&self) -> &ConformancePackId {
        &self.pack
    }

    /// Returns the exact authorized scope.
    pub const fn scope(&self) -> &CheckScopeId {
        &self.scope
    }

    /// Returns the exact input closure.
    pub const fn input_closure(&self) -> &CheckInputClosureId {
        &self.input_closure
    }

    fn datum(&self) -> Result<Datum, ConformanceError> {
        Ok(Datum::Node {
            tag: crate::qualified("conformance/checker-revocation-entry-v1")?,
            fields: vec![
                field("key", self.id.to_datum())?,
                field("revocation-source", self.source.to_datum())?,
                field("binding", self.binding.to_datum())?,
                field("subject", self.subject.to_datum())?,
                field("checker-code", self.checker_code.to_datum())?,
                field("pack", self.pack.to_datum())?,
                field("scope", self.scope.to_datum())?,
                field("input-closure", self.input_closure.to_datum())?,
            ],
        })
    }
}

/// One explicit owner policy decision. `Unknown` is represented by absence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerRevocationDecision {
    key: CheckerRevocationKey,
    status: RevocationStatus,
}

impl CheckerRevocationDecision {
    /// Records an explicit owner decision for one exact key.
    pub fn new(
        key: CheckerRevocationKey,
        status: RevocationStatus,
    ) -> Result<Self, ConformanceError> {
        if status == RevocationStatus::Unknown {
            return Err(ConformanceError::InvalidRevocationDecision);
        }
        Ok(Self { key, status })
    }

    /// Returns the exact key selected by this decision.
    pub const fn key(&self) -> &CheckerRevocationKey {
        &self.key
    }

    /// Returns the explicit owner-selected status.
    pub const fn status(&self) -> RevocationStatus {
        self.status
    }

    fn datum(&self) -> Result<Datum, ConformanceError> {
        let mut datum = self.key.datum()?;
        let Datum::Node { fields, .. } = &mut datum else {
            unreachable!("revocation key projection is always a node")
        };
        fields.push(field("status", self.status.datum())?);
        Ok(datum)
    }
}

/// Immutable canonical snapshot supplied by a qualified checker owner.
///
/// This neutral record does not authenticate the issuer or invent policy
/// decisions. Admission owners must select a snapshot from the authoritative
/// checker owner and re-select it whenever a receipt is reused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerRevocationSet {
    id: CheckerRevocationSetId,
    issuer: OwnerBindingId,
    source: RevocationSourceId,
    policy: PolicyId,
    head: CheckerRevocationHeadId,
    decisions: BTreeMap<CheckerRevocationKeyId, CheckerRevocationDecision>,
}

impl CheckerRevocationSet {
    /// Canonicalizes one complete immutable owner snapshot.
    pub fn from_owner_snapshot(
        issuer: OwnerBindingId,
        source: RevocationSourceId,
        policy: PolicyId,
        head: CheckerRevocationHeadId,
        decisions: Vec<CheckerRevocationDecision>,
    ) -> Result<Self, ConformanceError> {
        let mut indexed = BTreeMap::new();
        for decision in decisions {
            if decision.key.source != source {
                return Err(ConformanceError::InvocationMismatch(
                    "revocation decision source",
                ));
            }
            let key = decision.key.id.clone();
            if indexed.insert(key.clone(), decision).is_some() {
                return Err(ConformanceError::DuplicateConstruction(format!(
                    "revocation decision {key:?}"
                )));
            }
        }
        let encoded = indexed
            .values()
            .map(CheckerRevocationDecision::datum)
            .collect::<Result<Vec<_>, _>>()?;
        let id = SemanticId::from_fields(vec![
            field("issuer", issuer.to_datum())?,
            field("revocation-source", source.to_datum())?,
            field("policy", policy.to_datum())?,
            field("head", head.to_datum())?,
            field("decisions", Datum::Vector(encoded))?,
        ])?;
        Ok(Self {
            id,
            issuer,
            source,
            policy,
            head,
            decisions: indexed,
        })
    }

    /// Resolves an exact pre-receipt key against this selected snapshot.
    ///
    /// An absent or wrong-source key is `Unknown`; the neutral layer never
    /// upgrades absence into `Current`.
    pub fn lookup(&self, key: &CheckerRevocationKey) -> CheckerRevocationSelection {
        let status = if key.source == self.source {
            self.decisions
                .get(key.id())
                .map_or(RevocationStatus::Unknown, CheckerRevocationDecision::status)
        } else {
            RevocationStatus::Unknown
        };
        CheckerRevocationSelection {
            issuer: self.issuer.clone(),
            source: self.source.clone(),
            key: key.clone(),
            set: self.id.clone(),
            policy: self.policy.clone(),
            head: self.head.clone(),
            status,
        }
    }

    /// Returns the immutable set identity.
    pub const fn id(&self) -> &CheckerRevocationSetId {
        &self.id
    }

    /// Returns the checker-owner binding that supplied the snapshot.
    pub const fn issuer(&self) -> &OwnerBindingId {
        &self.issuer
    }

    /// Returns the authoritative revocation source.
    pub const fn source(&self) -> &RevocationSourceId {
        &self.source
    }

    /// Returns the policy identity applied by the owner.
    pub const fn policy(&self) -> &PolicyId {
        &self.policy
    }

    /// Returns the selected source head.
    pub const fn head(&self) -> &CheckerRevocationHeadId {
        &self.head
    }
}

/// Pre-receipt result of looking up an exact key in an owner-selected set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerRevocationSelection {
    issuer: OwnerBindingId,
    source: RevocationSourceId,
    key: CheckerRevocationKey,
    set: CheckerRevocationSetId,
    policy: PolicyId,
    head: CheckerRevocationHeadId,
    status: RevocationStatus,
}

impl CheckerRevocationSelection {
    /// Binds this pre-receipt selection to the resulting receipt identity.
    pub fn bind_receipt(&self, receipt: CheckerReceiptId) -> CheckerRevocationObservation {
        CheckerRevocationObservation {
            issuer: self.issuer.clone(),
            source: self.source.clone(),
            key: self.key.clone(),
            set: self.set.clone(),
            policy: self.policy.clone(),
            head: self.head.clone(),
            receipt,
            status: self.status,
        }
    }

    /// Returns the selected owner binding.
    pub const fn issuer(&self) -> &OwnerBindingId {
        &self.issuer
    }

    /// Returns the selected source.
    pub const fn source(&self) -> &RevocationSourceId {
        &self.source
    }

    /// Returns the queried key.
    pub const fn key(&self) -> &CheckerRevocationKey {
        &self.key
    }

    /// Returns the selected immutable set.
    pub const fn set(&self) -> &CheckerRevocationSetId {
        &self.set
    }

    /// Returns the selected policy.
    pub const fn policy(&self) -> &PolicyId {
        &self.policy
    }

    /// Returns the selected source head.
    pub const fn head(&self) -> &CheckerRevocationHeadId {
        &self.head
    }

    /// Returns the exact explicit-or-absent lookup result.
    pub const fn status(&self) -> RevocationStatus {
        self.status
    }
}

/// Receipt-bound currentness observation from the selected owner snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerRevocationObservation {
    issuer: OwnerBindingId,
    source: RevocationSourceId,
    key: CheckerRevocationKey,
    set: CheckerRevocationSetId,
    policy: PolicyId,
    head: CheckerRevocationHeadId,
    receipt: CheckerReceiptId,
    status: RevocationStatus,
}

impl CheckerRevocationObservation {
    /// Returns the checker-owner binding that supplied this observation.
    pub const fn issuer(&self) -> &OwnerBindingId {
        &self.issuer
    }

    /// Returns the selected revocation source.
    pub const fn source(&self) -> &RevocationSourceId {
        &self.source
    }

    /// Returns the exact queried key.
    pub const fn key(&self) -> &CheckerRevocationKey {
        &self.key
    }

    /// Returns the selected set identity.
    pub const fn set(&self) -> &CheckerRevocationSetId {
        &self.set
    }

    /// Returns the selected policy identity.
    pub const fn policy(&self) -> &PolicyId {
        &self.policy
    }

    /// Returns the selected source head.
    pub const fn head(&self) -> &CheckerRevocationHeadId {
        &self.head
    }

    /// Returns the receipt bound after lookup.
    pub const fn receipt(&self) -> &CheckerReceiptId {
        &self.receipt
    }

    /// Returns the exact lookup result.
    pub const fn status(&self) -> RevocationStatus {
        self.status
    }
}
