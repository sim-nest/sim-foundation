// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Live, owner-selected currentness for an exact checker binding.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use sim_kernel::{ContentId, Datum, Symbol};

use crate::{
    CheckInvocation, CheckerBinding, CheckerReceipt, CheckerResultId, CheckerRevocationDecision,
    CheckerRevocationHeadId, CheckerRevocationKey, CheckerRevocationKeyId,
    CheckerRevocationObservation, CheckerRevocationSelection, CheckerRevocationSet,
    ConformanceError, EvidenceGrade, EvidenceProvenanceId, EvidenceSetId, PolicyId,
    RevocationStatus,
};

/// Failure while issuing or freshly observing a live checker receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveCheckerError {
    /// A canonical conformance record was invalid.
    Conformance(ConformanceError),
    /// The live owner cannot be reached safely.
    Unavailable,
    /// The handle or receipt belongs to another owner generation.
    ForeignAuthority,
    /// The invocation does not belong to the owner's exact binding.
    BindingMismatch,
}

impl fmt::Display for LiveCheckerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conformance(error) => error.fmt(formatter),
            Self::Unavailable => formatter.write_str("checker owner is not live"),
            Self::ForeignAuthority => formatter.write_str("foreign checker-owner generation"),
            Self::BindingMismatch => formatter.write_str("invocation uses another checker binding"),
        }
    }
}

impl std::error::Error for LiveCheckerError {}

impl From<ConformanceError> for LiveCheckerError {
    fn from(error: ConformanceError) -> Self {
        Self::Conformance(error)
    }
}

#[derive(Debug)]
struct LiveCheckerState {
    generation: ContentId,
    binding: CheckerBinding,
    policy: PolicyId,
    revision: u64,
    decisions: BTreeMap<CheckerRevocationKeyId, (CheckerRevocationKey, RevocationStatus)>,
}

/// Sole strong owner of one exact installed checker binding and generation.
///
/// This neutral owner does not execute a checker or choose evidence. The
/// installed checker wrapper does both and uses the [`LiveCheckerIssuer`]
/// returned alongside this owner (never reconstructible afterward) to issue
/// a receipt, and freely-distributed [`LiveCheckerAuthority`] handles only to
/// reobserve one under the binding's authoritative revocation source.
#[derive(Debug)]
pub struct LiveCheckerOwner {
    state: Arc<RwLock<LiveCheckerState>>,
}

/// Sole capability to issue a receipt for one live checker-owner generation.
///
/// [`LiveCheckerOwner::boot`] returns exactly one of these, and this type is
/// not [`Clone`]: possession of an issuer is possession of minting authority,
/// so it cannot be re-derived from a [`LiveCheckerAuthority`] or duplicated
/// once issued. A verifier that only needs to reobserve currentness -- the
/// common case, and the only capability that should be handed to an admitted
/// consumer -- must use [`LiveCheckerAuthority`] instead, which cannot mint.
#[derive(Debug)]
pub struct LiveCheckerIssuer {
    state: Weak<RwLock<LiveCheckerState>>,
    generation: ContentId,
}

/// Weak, freely-distributable verification handle for one live checker-owner
/// generation.
///
/// This handle can reobserve currentness ([`LiveCheckerReceipt::verify_current`],
/// [`LiveCheckerReceipt::while_current`]) but cannot issue a receipt: minting
/// authority lives only in [`LiveCheckerIssuer`]. Clones cannot keep the
/// owning service alive.
#[derive(Clone, Debug)]
pub struct LiveCheckerAuthority {
    state: Weak<RwLock<LiveCheckerState>>,
    generation: ContentId,
}

/// Opaque receipt joined to the exact live checker generation that issued it.
#[derive(Clone, Debug)]
pub struct LiveCheckerReceipt {
    authority: Weak<RwLock<LiveCheckerState>>,
    generation: ContentId,
    binding: CheckerBinding,
    invocation: CheckInvocation,
    selection: CheckerRevocationSelection,
    receipt: CheckerReceipt,
    issue_observation: CheckerRevocationObservation,
}

impl LiveCheckerOwner {
    /// Starts an installed checker generation with one exact binding and policy.
    ///
    /// Returns the owner together with the sole [`LiveCheckerIssuer`] for this
    /// generation: minting authority is handed out exactly once, at boot, and
    /// there is no later method that can produce a second one.
    #[must_use]
    pub fn boot(
        generation: ContentId,
        binding: CheckerBinding,
        policy: PolicyId,
    ) -> (Self, LiveCheckerIssuer) {
        let state = Arc::new(RwLock::new(LiveCheckerState {
            generation: generation.clone(),
            binding,
            policy,
            revision: 0,
            decisions: BTreeMap::new(),
        }));
        let issuer = LiveCheckerIssuer {
            state: Arc::downgrade(&state),
            generation,
        };
        (Self { state }, issuer)
    }

    /// Issues a weak verification handle for an admitted consumer.
    ///
    /// This handle can reobserve currentness but cannot issue a receipt.
    #[must_use]
    pub fn authority(&self) -> LiveCheckerAuthority {
        let generation = self
            .state
            .read()
            .map(|state| state.generation.clone())
            .unwrap_or_else(|poisoned| poisoned.into_inner().generation.clone());
        LiveCheckerAuthority {
            state: Arc::downgrade(&self.state),
            generation,
        }
    }

    /// Selects `Current` for one exact invocation.
    pub fn mark_current(&self, invocation: &CheckInvocation) -> Result<(), LiveCheckerError> {
        self.record(invocation, RevocationStatus::Current)
    }

    /// Selects `Revoked` for one exact invocation.
    pub fn revoke(&self, invocation: &CheckInvocation) -> Result<(), LiveCheckerError> {
        self.record(invocation, RevocationStatus::Revoked)
    }

    fn record(
        &self,
        invocation: &CheckInvocation,
        status: RevocationStatus,
    ) -> Result<(), LiveCheckerError> {
        let mut state = write(&self.state)?;
        ensure_binding(&state, invocation)?;
        let key = CheckerRevocationKey::for_invocation(
            state.binding.revocation_source().clone(),
            invocation,
        )?;
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or(LiveCheckerError::Unavailable)?;
        state.decisions.insert(key.id().clone(), (key, status));
        Ok(())
    }
}

impl LiveCheckerIssuer {
    /// Returns the durable owner-generation identity.
    pub const fn generation(&self) -> &ContentId {
        &self.generation
    }

    /// Confirms that the selected generation remains live.
    pub fn ensure_live(&self) -> Result<(), LiveCheckerError> {
        let strong = self.state.upgrade().ok_or(LiveCheckerError::Unavailable)?;
        let state = read(&strong)?;
        if state.generation == self.generation {
            Ok(())
        } else {
            Err(LiveCheckerError::ForeignAuthority)
        }
    }

    /// Issues a receipt for a result produced by the installed checker wrapper.
    ///
    /// This is the sole minting operation in this crate. It is reachable only
    /// through the one [`LiveCheckerIssuer`] returned by [`LiveCheckerOwner::boot`];
    /// a [`LiveCheckerAuthority`] handed to an admitted consumer cannot reach it.
    #[allow(clippy::too_many_arguments)]
    pub fn issue(
        &self,
        invocation: CheckInvocation,
        result: CheckerResultId,
        grade: EvidenceGrade,
        provenance: EvidenceProvenanceId,
        support: EvidenceSetId,
    ) -> Result<LiveCheckerReceipt, LiveCheckerError> {
        let strong = self.state.upgrade().ok_or(LiveCheckerError::Unavailable)?;
        let state = read(&strong)?;
        if state.generation != self.generation {
            return Err(LiveCheckerError::ForeignAuthority);
        }
        ensure_binding(&state, &invocation)?;
        let set = snapshot(&state)?;
        let key = CheckerRevocationKey::for_invocation(
            state.binding.revocation_source().clone(),
            &invocation,
        )?;
        let selection = set.lookup(&key);
        let receipt = CheckerReceipt::passing(
            &state.binding,
            &invocation,
            result,
            grade,
            provenance,
            state.policy.clone(),
            support,
            &selection,
        )?;
        let issue_observation = selection.bind_receipt(receipt.id().clone());
        receipt.verify(&state.binding, &invocation, &issue_observation)?;
        Ok(LiveCheckerReceipt {
            authority: Arc::downgrade(&strong),
            generation: self.generation.clone(),
            binding: state.binding.clone(),
            invocation,
            selection,
            receipt,
            issue_observation,
        })
    }
}

impl LiveCheckerAuthority {
    /// Returns the durable owner-generation identity.
    pub const fn generation(&self) -> &ContentId {
        &self.generation
    }

    /// Returns whether both handles were issued by the same live-owner state.
    ///
    /// Equal generation identifiers are descriptive data and are insufficient:
    /// the weak owner allocation must also be identical. This comparison does
    /// not keep either owner alive.
    #[must_use]
    pub fn same_owner(&self, other: &Self) -> bool {
        self.generation == other.generation && Weak::ptr_eq(&self.state, &other.state)
    }

    /// Confirms that the selected generation remains live.
    pub fn ensure_live(&self) -> Result<(), LiveCheckerError> {
        let strong = self.state.upgrade().ok_or(LiveCheckerError::Unavailable)?;
        let state = read(&strong)?;
        if state.generation == self.generation {
            Ok(())
        } else {
            Err(LiveCheckerError::ForeignAuthority)
        }
    }

    fn observe(
        &self,
        qualified: &LiveCheckerReceipt,
    ) -> Result<CheckerRevocationObservation, LiveCheckerError> {
        if self.generation != qualified.generation
            || !Weak::ptr_eq(&self.state, &qualified.authority)
        {
            return Err(LiveCheckerError::ForeignAuthority);
        }
        let strong = self.state.upgrade().ok_or(LiveCheckerError::Unavailable)?;
        let state = read(&strong)?;
        let set = snapshot(&state)?;
        Ok(set
            .lookup(qualified.selection.key())
            .bind_receipt(qualified.receipt.id().clone()))
    }
}

impl LiveCheckerReceipt {
    /// Returns the exact checker binding.
    pub const fn binding(&self) -> &CheckerBinding {
        &self.binding
    }

    /// Returns the exact checker invocation.
    pub const fn invocation(&self) -> &CheckInvocation {
        &self.invocation
    }

    /// Returns the immutable checker receipt.
    pub const fn receipt(&self) -> &CheckerReceipt {
        &self.receipt
    }

    /// Returns the receipt-bound issue-time observation.
    pub const fn issue_observation(&self) -> &CheckerRevocationObservation {
        &self.issue_observation
    }

    /// Verifies issue coherence and obtains fresh currentness from the owner.
    pub fn verify_current(
        &self,
        authority: &LiveCheckerAuthority,
    ) -> Result<CheckerRevocationObservation, LiveCheckerError> {
        if self.selection.bind_receipt(self.receipt.id().clone()) != self.issue_observation {
            return Err(ConformanceError::InvocationMismatch("issue observation").into());
        }
        self.receipt
            .verify(&self.binding, &self.invocation, &self.issue_observation)?;
        let current = authority.observe(self)?;
        self.receipt
            .verify(&self.binding, &self.invocation, &current)?;
        Ok(current)
    }

    /// Runs `action` while a read lease keeps this exact receipt current.
    ///
    /// Revocation requires the owner's write lock, so it cannot become visible
    /// between the fresh verification and the end of the protected action.
    pub fn while_current<T>(
        &self,
        authority: &LiveCheckerAuthority,
        action: impl FnOnce() -> T,
    ) -> Result<T, LiveCheckerError> {
        if self.selection.bind_receipt(self.receipt.id().clone()) != self.issue_observation {
            return Err(ConformanceError::InvocationMismatch("issue observation").into());
        }
        self.receipt
            .verify(&self.binding, &self.invocation, &self.issue_observation)?;
        if authority.generation != self.generation
            || !Weak::ptr_eq(&authority.state, &self.authority)
        {
            return Err(LiveCheckerError::ForeignAuthority);
        }
        let strong = authority
            .state
            .upgrade()
            .ok_or(LiveCheckerError::Unavailable)?;
        let state = read(&strong)?;
        let current = snapshot(&state)?
            .lookup(self.selection.key())
            .bind_receipt(self.receipt.id().clone());
        self.receipt
            .verify(&self.binding, &self.invocation, &current)?;
        let result = action();
        drop(state);
        Ok(result)
    }
}

fn ensure_binding(
    state: &LiveCheckerState,
    invocation: &CheckInvocation,
) -> Result<(), LiveCheckerError> {
    if invocation.binding() == state.binding.id()
        && state.binding.allowed_scopes().contains(invocation.scope())
    {
        Ok(())
    } else {
        Err(LiveCheckerError::BindingMismatch)
    }
}

fn snapshot(state: &LiveCheckerState) -> Result<CheckerRevocationSet, LiveCheckerError> {
    let head = CheckerRevocationHeadId::from_fields(vec![
        (
            Symbol::qualified("conformance", "owner-generation"),
            crate::content_id_datum(&state.generation),
        ),
        (
            Symbol::qualified("conformance", "owner-revision"),
            Datum::String(state.revision.to_string()),
        ),
    ])?;
    let decisions = state
        .decisions
        .values()
        .map(|(key, status)| CheckerRevocationDecision::new(key.clone(), *status))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CheckerRevocationSet::from_owner_snapshot(
        state.binding.owner().clone(),
        state.binding.revocation_source().clone(),
        state.policy.clone(),
        head,
        decisions,
    )?)
}

fn read(
    state: &Arc<RwLock<LiveCheckerState>>,
) -> Result<RwLockReadGuard<'_, LiveCheckerState>, LiveCheckerError> {
    state.read().map_err(|_| LiveCheckerError::Unavailable)
}

fn write(
    state: &Arc<RwLock<LiveCheckerState>>,
) -> Result<RwLockWriteGuard<'_, LiveCheckerState>, LiveCheckerError> {
    state.write().map_err(|_| LiveCheckerError::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeSet,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
        time::Duration,
    };

    use super::*;
    use crate::{
        CheckArgument, CheckTemplate, CommandId, ConformancePackId, EnvironmentPolicyId,
        EvidenceProvenanceId, EvidenceSetId, IdKind, OutputShapeId, OwnerBindingId, ProofCodeId,
        SemanticId, WorkingDirectoryPolicyId,
    };

    fn sid<K: IdKind>(value: &str) -> SemanticId<K> {
        SemanticId::from_text(value).unwrap()
    }

    fn binding() -> CheckerBinding {
        let scope = sid("scope/test");
        CheckerBinding::new(
            "checker/test".into(),
            OwnerBindingId::from_text("owner/test").unwrap(),
            "test::check".into(),
            vec![ConformancePackId::from_text("pack/test").unwrap()],
            OutputShapeId::from_text("shape/receipt").unwrap(),
            sid("revocation/test"),
            CommandId::from_text("command/test").unwrap(),
            CommandId::from_text("command/docs").unwrap(),
            BTreeSet::from([scope]),
            CheckTemplate::new(
                "check".into(),
                vec![
                    CheckArgument::BindingSlot,
                    CheckArgument::SubjectSlot,
                    CheckArgument::ScopeSlot,
                ],
                WorkingDirectoryPolicyId::from_text("cwd/test").unwrap(),
                EnvironmentPolicyId::from_text("env/test").unwrap(),
                OutputShapeId::from_text("shape/result").unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn authority_identity_requires_the_same_owner_not_an_equal_generation() {
        let generation = Datum::String("generation/test".into())
            .content_id()
            .unwrap();
        let (first, _first_issuer) =
            LiveCheckerOwner::boot(generation.clone(), binding(), sid("policy/test"));
        let (second, _second_issuer) =
            LiveCheckerOwner::boot(generation, binding(), sid("policy/test"));
        let first_authority = first.authority();
        let first_clone = first_authority.clone();
        let second_authority = second.authority();

        assert!(first_authority.same_owner(&first_clone));
        assert!(!first_authority.same_owner(&second_authority));
    }

    #[test]
    fn currentness_read_lease_blocks_revocation_until_callback_returns() {
        let binding = binding();
        let invocation = binding
            .instantiate(
                ProofCodeId::from_text("code/test").unwrap(),
                ConformancePackId::from_text("pack/test").unwrap(),
                sid("subject/test"),
                sid("scope/test"),
                sid("input/test"),
            )
            .unwrap();
        let (owner, issuer) = LiveCheckerOwner::boot(
            Datum::String("generation/test".into())
                .content_id()
                .unwrap(),
            binding,
            sid("policy/test"),
        );
        let owner = Arc::new(owner);
        owner.mark_current(&invocation).unwrap();
        let authority = owner.authority();
        let receipt = issuer
            .issue(
                invocation.clone(),
                sid("result/test"),
                EvidenceGrade::Bootstrap,
                EvidenceProvenanceId::from_text("provenance/test").unwrap(),
                EvidenceSetId::from_text("support/test").unwrap(),
            )
            .unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let lease = thread::spawn(move || {
            receipt
                .while_current(&authority, || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                })
                .unwrap();
        });
        entered_rx.recv().unwrap();
        let revoked = Arc::new(AtomicBool::new(false));
        let revoker_owner = owner.clone();
        let revoker_done = revoked.clone();
        let (started_tx, started_rx) = mpsc::channel();
        let revoker = thread::spawn(move || {
            started_tx.send(()).unwrap();
            revoker_owner.revoke(&invocation).unwrap();
            revoker_done.store(true, Ordering::SeqCst);
        });
        started_rx.recv().unwrap();
        thread::sleep(Duration::from_millis(30));
        assert!(!revoked.load(Ordering::SeqCst));
        release_tx.send(()).unwrap();
        lease.join().unwrap();
        revoker.join().unwrap();
        assert!(revoked.load(Ordering::SeqCst));
    }
}
