//! Separately scoped owner qualification and phase-gate declarations.

use sim_kernel::Datum;

use crate::{
    ActivationMapId, CheckScopeId, CheckedSubjectId, CheckerReceiptId, ConformanceError,
    DependencyUseSetId, OwnerBinding, OwnerBindingId, SurfaceKey, SurfaceSetId, SurfaceStatus,
    binding::unique, field, qualified, text,
};

/// Scope carried by both a binding qualification and its supporting receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QualificationScope {
    /// Architecture activation only.
    Activation {
        /// Activated architecture map.
        map: ActivationMapId,
    },
    /// The released dependencies consumed by one use set.
    Dependencies {
        /// Exact dependency-use declaration.
        uses: DependencyUseSetId,
    },
    /// Surfaces produced by one phase.
    Produced {
        /// Producing phase.
        phase: String,
        /// Exact produced surfaces.
        surfaces: SurfaceSetId,
    },
    /// Full selected-roadmap surface closure.
    RoadmapFinal {
        /// Exact selected roadmap subject.
        selected: CheckedSubjectId,
        /// Complete final surface set.
        surfaces: SurfaceSetId,
    },
}

impl QualificationScope {
    /// Projects the exact scope into its canonical record form.
    pub fn to_datum(&self) -> Result<Datum, ConformanceError> {
        let (name, fields) = match self {
            Self::Activation { map } => ("activation", vec![field("map", map.to_datum())?]),
            Self::Dependencies { uses } => ("dependencies", vec![field("uses", uses.to_datum())?]),
            Self::Produced { phase, surfaces } => (
                "produced",
                vec![
                    field("phase", text(phase.clone()))?,
                    field("surfaces", surfaces.to_datum())?,
                ],
            ),
            Self::RoadmapFinal { selected, surfaces } => (
                "roadmap-final",
                vec![
                    field("selected", selected.to_datum())?,
                    field("surfaces", surfaces.to_datum())?,
                ],
            ),
        };
        Ok(Datum::Node {
            tag: qualified(&format!("conformance/qualification-{name}-v1"))?,
            fields,
        })
    }

    /// Returns true only for exact scope equality.
    pub fn exactly(&self, other: &Self) -> bool {
        self == other
    }
}

/// Exact released evidence resolving a declaration key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedSurfaceRef {
    /// Declaration being resolved.
    pub key: SurfaceKey,
    /// Exact source identity.
    pub source: CheckedSubjectId,
    /// Installed package identity.
    pub package: CheckedSubjectId,
    /// Receipt that qualified this exact surface and scope.
    pub receipt: CheckerReceiptId,
}

/// Immutable qualification attachment; it never changes the binding id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingQualificationReceipt {
    binding: OwnerBindingId,
    scope: QualificationScope,
    resolved: Vec<CheckedSurfaceRef>,
    subject: CheckedSubjectId,
    checks: Vec<CheckerReceiptId>,
}

impl BindingQualificationReceipt {
    /// Constructs a qualification attachment only after checking its exact owner and scope.
    pub fn new(
        binding: &OwnerBinding,
        scope: QualificationScope,
        resolved: Vec<CheckedSurfaceRef>,
        subject: CheckedSubjectId,
        checks: Vec<CheckerReceiptId>,
    ) -> Result<Self, ConformanceError> {
        let receipt = Self {
            binding: binding.id().clone(),
            scope,
            resolved,
            subject,
            checks,
        };
        receipt.verify(binding)?;
        Ok(receipt)
    }

    /// Returns the binding being qualified.
    pub const fn binding(&self) -> &OwnerBindingId {
        &self.binding
    }

    /// Returns the exact allowed claim scope.
    pub const fn scope(&self) -> &QualificationScope {
        &self.scope
    }

    /// Returns every resolved surface reference.
    pub fn resolved(&self) -> &[CheckedSurfaceRef] {
        &self.resolved
    }

    /// Returns the subject checked by the supporting receipts.
    pub const fn subject(&self) -> &CheckedSubjectId {
        &self.subject
    }

    /// Returns every supporting checker receipt identity.
    pub fn checks(&self) -> &[CheckerReceiptId] {
        &self.checks
    }

    /// Validates that resolved surfaces belong to the binding and the scope is not inferred wider.
    pub fn verify(&self, binding: &OwnerBinding) -> Result<(), ConformanceError> {
        if &self.binding != binding.id() {
            return Err(ConformanceError::WrongOwner);
        }
        unique(
            self.resolved.iter().map(|item| item.key.as_str()),
            "resolved surface",
        )?;
        for resolved in &self.resolved {
            if binding.surface(&resolved.key).is_none() {
                return Err(ConformanceError::MissingSurface(
                    resolved.key.as_str().into(),
                ));
            }
        }
        if self.checks.is_empty() {
            return Err(ConformanceError::UnqualifiedDependency(
                binding.law().into(),
            ));
        }
        if matches!(self.scope, QualificationScope::Activation { .. })
            && self.resolved.iter().any(|surface| {
                binding
                    .surface(&surface.key)
                    .is_some_and(|item| matches!(item.status, SurfaceStatus::Planned { .. }))
            })
        {
            return Err(ConformanceError::ActivationIsNotProductionEvidence);
        }
        Ok(())
    }
}

/// One exact checker/scope tuple required to close a phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhaseCheck {
    /// Checker declaration.
    pub checker: String,
    /// Exact checker scope.
    pub scope: CheckScopeId,
}

/// Immutable checker requirements for a producing phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhaseGateSpec {
    /// Producing phase.
    pub phase: String,
    /// Exact required tuples.
    pub required: Vec<PhaseCheck>,
}
