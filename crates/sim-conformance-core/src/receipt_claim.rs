// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Exact consumer claims joined with neutral checker receipts.

use crate::{
    CheckInvocation, CheckerBinding, CheckerReceipt, CheckerResultId, CheckerRevocationHeadId,
    CheckerRevocationSetId, ConformanceError, EvidenceGrade, EvidenceProvenanceId, EvidenceSetId,
    PolicyId,
};

/// Expected result support selected independently of a submitted receipt.
///
/// This is a pure matching contract, not an execution attestation or authority
/// token. The consuming owner must derive it from its authorized checker and
/// held output observation, not copy fields from the receipt being checked.
/// Source-review and loaded-implementation authority remain separate obligations.
pub struct CheckerResultClaim<'a> {
    /// Content identity of the exact independently accepted passing result.
    pub result: &'a CheckerResultId,
    /// Minimum strength authorized by the consumer's policy.
    pub minimum_grade: EvidenceGrade,
    /// Exact execution provenance accepted by the observation owner.
    pub provenance: &'a EvidenceProvenanceId,
    /// Exact consumer-selected checker policy.
    pub policy: &'a PolicyId,
    /// Exact accepted supporting evidence set.
    pub support: &'a EvidenceSetId,
    /// Exact owner-selected current revocation set.
    pub revocation_set: &'a CheckerRevocationSetId,
    /// Exact owner-selected current revocation head.
    pub revocation_head: &'a CheckerRevocationHeadId,
}

impl CheckerReceipt {
    /// Joins a receipt with the independently expected invocation and result.
    ///
    /// The invocation binds checker code, pack, subject, scope, exact template
    /// expansion and input closure. The result claim additionally binds output,
    /// provenance, policy, support and evidence grade. The receipt-bound
    /// observation must come from the exact owner set independently selected by
    /// the consumer; consumers must obtain a fresh observation from the
    /// authoritative owner whenever they reuse a receipt. The receipt does not
    /// freeze a global set head, so an unrelated revocation need not invalidate
    /// an otherwise-current receipt.
    pub fn verify_claim(
        &self,
        binding: &CheckerBinding,
        expected_invocation: &CheckInvocation,
        expected_result: &CheckerResultClaim<'_>,
        revocation: &crate::CheckerRevocationObservation,
    ) -> Result<(), ConformanceError> {
        self.verify(binding, expected_invocation, revocation)?;
        if self.result() != expected_result.result {
            return Err(ConformanceError::InvocationMismatch("result"));
        }
        if self.grade() < expected_result.minimum_grade {
            return Err(ConformanceError::InvocationMismatch("evidence grade"));
        }
        if self.provenance() != expected_result.provenance {
            return Err(ConformanceError::InvocationMismatch("provenance"));
        }
        if self.policy() != expected_result.policy {
            return Err(ConformanceError::InvocationMismatch("policy"));
        }
        if self.support() != expected_result.support {
            return Err(ConformanceError::InvocationMismatch("support"));
        }
        if revocation.set() != expected_result.revocation_set {
            return Err(ConformanceError::InvocationMismatch("revocation set"));
        }
        if revocation.head() != expected_result.revocation_head {
            return Err(ConformanceError::InvocationMismatch("revocation head"));
        }
        Ok(())
    }
}
