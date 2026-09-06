//! Deterministic implementation-packet construction and return admission.

use std::collections::{BTreeMap, BTreeSet};

use sim_conformance_core::{
    CheckScopeId, CommandId, ConformanceError, DependencyUseSet, IdKind, InputPort, OwnerBinding,
    OwnerBindingId, SemanticId, StorageId, SurfaceKey, SurfaceStatus, SurfaceUseRole,
};
use sim_kernel::{Datum, NumberLiteral, Symbol};

use crate::{DescentCertificate, InputBudget, SemanticInputId, SemanticInputKind, WorkError};

/// Packet identity kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PacketKind;
impl IdKind for PacketKind {
    const DOMAIN: &'static str = "work/implementation-packet-v1";
}
/// Canonical identity of one implementation packet.
pub type PacketId = SemanticId<PacketKind>;

macro_rules! packet_id {
    ($(($kind:ident, $alias:ident, $domain:literal, $doc:literal)),+ $(,)?) => {$ (
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $kind;
        impl IdKind for $kind { const DOMAIN: &'static str = $domain; }
        #[doc = $doc]
        pub type $alias = SemanticId<$kind>;
    )+};
}

packet_id! {
    (BehaviorClaimKind, BehaviorClaimId, "work/behavior-claim-v1", "Identity of one behavior claim."),
    (FalsifierKind, FalsifierId, "work/falsifier-v1", "Identity of one concrete falsifier."),
    (TypeKind, TypeId, "work/type-v1", "Identity of one allowed public type."),
    (OutputContractKind, OutputContractId, "work/output-contract-v1", "Identity of the proposal return contract."),
    (ProofDefinitionKind, ProofDefinitionId, "work/proof-definition-v1", "Identity of one tests-first proof definition."),
    (FacetPlanKind, FacetPlanId, "work/facet-plan-v1", "Identity of a checked pure facet plan."),
    (ReceiptKind, ReceiptId, "work/receipt-v1", "Identity of supporting packet evidence."),
}

/// Whether this packet changes discoverable source facts or authored Index guidance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexImpact {
    /// No Index input changes.
    None,
    /// Source discovery changes but authored guidance does not.
    SourceFacts,
    /// Authored feature guidance or relationships change.
    AuthoredFeature,
}

/// Explicit condition that ends this bounded packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StopCondition {
    /// Stable condition name.
    pub condition: Symbol,
    /// Maximum proposal/revision attempts.
    pub max_attempts: u32,
}

/// A forbidden dependency or authority edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForbiddenEdge {
    /// Edge source.
    pub from: String,
    /// Edge target.
    pub to: String,
}

/// Exact source requested by a packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketInputSpec {
    /// Declared surface that owns the source.
    pub surface: SurfaceKey,
    /// Replaceable byte location.
    pub location: StorageId,
    /// Deterministic token count supplied by the selected tokenizer contract.
    pub tokens: u64,
}

/// Verified input descriptor retained in the packet envelope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketInput {
    /// Declared source surface.
    pub surface: SurfaceKey,
    /// Exact semantic byte value.
    pub semantic: SemanticInputId,
    /// Replaceable byte location, excluded from packet identity.
    pub location: StorageId,
    /// Exact byte count.
    pub bytes: u64,
    /// Deterministic token count.
    pub tokens: u64,
}

/// Current evidence for a used surface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceEvidence {
    /// Declaration key.
    pub key: SurfaceKey,
    /// Owning binding.
    pub owner: OwnerBindingId,
    /// Current implementation state.
    pub state: SurfaceEvidenceState,
}

/// Exact implementation/qualification state used at packet admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurfaceEvidenceState {
    /// Declared target that has not yet been produced.
    Planned {
        /// Sole activated producing phase.
        producing_phase: String,
    },
    /// Released dependency with an exact qualification scope.
    Released {
        /// Exact use set whose dependency scope was qualified.
        dependency_uses: sim_conformance_core::DependencyUseSetId,
    },
}

/// Fields authored before source materialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketDraft {
    /// Producing phase.
    pub phase: String,
    /// Singular owner binding.
    pub owner: OwnerBindingId,
    /// One behavior claim.
    pub behavior: BehaviorClaimId,
    /// One concrete falsifier.
    pub falsifier: FalsifierId,
    /// Public types the proposal may use.
    pub allowed_api: Vec<TypeId>,
    /// Hard input/output ceilings.
    pub input_budget: InputBudget,
    /// Expected hostile-return contract.
    pub output_contract: OutputContractId,
    /// Explicit forbidden edges.
    pub forbidden_edges: Vec<ForbiddenEdge>,
    /// Tests authored before implementation.
    pub tests_first: Vec<ProofDefinitionId>,
    /// Exact owner validation command.
    pub validation: CommandId,
    /// Exact owner docs command.
    pub docs: CommandId,
    /// Index impact declaration.
    pub index_impact: IndexImpact,
    /// Strictly decreasing retry measure.
    pub descent: DescentCertificate,
    /// Hard terminal condition.
    pub stop: StopCondition,
    /// Optional commit subject, never an authority grant.
    pub commit_subject: Option<String>,
}

/// Fully admitted deterministic packet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplementationPacket {
    id: PacketId,
    /// Packet fields independent of source location.
    pub draft: PacketDraft,
    /// Verified exact sources.
    pub inputs: Vec<PacketInput>,
    /// Declared dependencies and funded targets.
    pub dependency_uses: sim_conformance_core::DependencyUseSetId,
    /// Exact funded-target set.
    pub funded_targets: Vec<SurfaceKey>,
}

impl ImplementationPacket {
    /// Returns the location-independent packet identity.
    pub const fn id(&self) -> &PacketId {
        &self.id
    }
}

/// Deterministic, effect-free packet constructor.
pub struct PacketBuilder;

impl PacketBuilder {
    /// Verifies architecture, materializes exact declared source, and computes packet identity.
    pub fn build(
        draft: PacketDraft,
        binding: &OwnerBinding,
        uses: &DependencyUseSet,
        evidence: &[SurfaceEvidence],
        mut input_specs: Vec<PacketInputSpec>,
        input_port: &mut dyn InputPort,
    ) -> Result<ImplementationPacket, WorkError> {
        if &draft.owner != binding.id() || &uses.owner != binding.id() {
            return Err(WorkError::Qualification("wrong-owner".into()));
        }
        if draft.phase.is_empty() || draft.phase != uses.phase {
            return Err(WorkError::InvalidPacket("phase"));
        }
        draft.descent.verify()?;
        if draft.tests_first.is_empty() || draft.stop.max_attempts == 0 {
            return Err(WorkError::InvalidPacket("proof or stop condition"));
        }
        validate_uses(binding, uses, evidence).map_err(map_conformance)?;

        input_specs.sort_by(|a, b| a.surface.cmp(&b.surface));
        if input_specs
            .windows(2)
            .any(|pair| pair[0].surface == pair[1].surface)
        {
            return Err(WorkError::InvalidPacket("duplicate input"));
        }
        let declared = uses
            .uses
            .iter()
            .map(|item| &item.surface)
            .collect::<BTreeSet<_>>();
        let mut inputs = Vec::with_capacity(input_specs.len());
        let mut total_bytes = 0u64;
        let mut total_tokens = 0u64;
        for spec in input_specs {
            if !declared.contains(&spec.surface) {
                return Err(WorkError::UndeclaredInput(spec.surface.as_str().into()));
            }
            let bytes = input_port
                .read(&spec.location)
                .map_err(|_| WorkError::UndeclaredInput(spec.surface.as_str().into()))?;
            spec.location
                .verify(&bytes)
                .map_err(|_| WorkError::SourceDigest)?;
            let byte_len = bytes.len() as u64;
            total_bytes = total_bytes
                .checked_add(byte_len)
                .ok_or(WorkError::ByteBudget)?;
            total_tokens = total_tokens
                .checked_add(spec.tokens)
                .ok_or(WorkError::TokenBudget)?;
            let semantic = SemanticId::<SemanticInputKind>::from_fields(vec![
                cfield("surface", Datum::String(spec.surface.as_str().into()))
                    .map_err(map_conformance)?,
                cfield("bytes", Datum::Bytes(bytes)).map_err(map_conformance)?,
            ])
            .map_err(map_conformance)?;
            inputs.push(PacketInput {
                surface: spec.surface,
                semantic,
                location: spec.location,
                bytes: byte_len,
                tokens: spec.tokens,
            });
        }
        let file_count = u32::try_from(inputs.len()).map_err(|_| WorkError::FileBudget)?;
        if file_count > draft.input_budget.files {
            return Err(WorkError::FileBudget);
        }
        if total_bytes > draft.input_budget.bytes {
            return Err(WorkError::ByteBudget);
        }
        if total_tokens > draft.input_budget.tokens {
            return Err(WorkError::TokenBudget);
        }
        let funded_targets = uses
            .uses
            .iter()
            .filter(|item| item.role == SurfaceUseRole::FundedTarget)
            .map(|item| item.surface.clone())
            .collect::<Vec<_>>();
        let id =
            packet_identity(&draft, &inputs, uses, &funded_targets).map_err(map_conformance)?;
        Ok(ImplementationPacket {
            id,
            draft,
            inputs,
            dependency_uses: uses.id().clone(),
            funded_targets,
        })
    }
}

/// Immutable result of hostile-return decoding and pure facet checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedProposal {
    /// Packet that bounded the proposal.
    pub packet: PacketId,
    /// Raw opaque return bytes.
    pub raw: StorageId,
    /// Decoded checked value.
    pub decoded: Datum,
    /// Pure facet plan identity.
    pub facet_plan: FacetPlanId,
    /// Supporting pure receipts.
    pub receipts: Vec<ReceiptId>,
}

/// Decodes hostile bytes under a caller-supplied parser and exact Shape predicate.
pub fn decode_proposal(
    packet: &ImplementationPacket,
    bytes: &[u8],
    decode: impl FnOnce(&[u8]) -> Result<Datum, String>,
    shape_matches: impl FnOnce(&Datum, &OutputContractId) -> Result<(), String>,
    facet_plan: FacetPlanId,
    receipts: Vec<ReceiptId>,
) -> Result<CheckedProposal, WorkError> {
    if bytes.len() as u64 > packet.draft.input_budget.output_bytes {
        return Err(WorkError::OutputBudget);
    }
    let raw = StorageId::for_bytes(bytes);
    let decoded = decode(bytes).map_err(|error| WorkError::MalformedReturn(vec![error]))?;
    shape_matches(&decoded, &packet.draft.output_contract)
        .map_err(|error| WorkError::MalformedReturn(vec![error]))?;
    Ok(CheckedProposal {
        packet: packet.id.clone(),
        raw,
        decoded,
        facet_plan,
        receipts,
    })
}

fn validate_uses(
    binding: &OwnerBinding,
    uses: &DependencyUseSet,
    evidence: &[SurfaceEvidence],
) -> Result<(), ConformanceError> {
    let catalog = evidence
        .iter()
        .map(|item| (item.key.clone(), item))
        .collect::<BTreeMap<_, _>>();
    if catalog.len() != evidence.len() {
        return Err(ConformanceError::DuplicateField("surface evidence".into()));
    }
    for item in &uses.uses {
        let Some(observed) = catalog.get(&item.surface) else {
            return Err(ConformanceError::MissingSurface(
                item.surface.as_str().into(),
            ));
        };
        match item.role {
            SurfaceUseRole::ReleasedDependency => match &observed.state {
                SurfaceEvidenceState::Released { dependency_uses }
                    if dependency_uses == uses.id() => {}
                SurfaceEvidenceState::Planned { .. } => {
                    return Err(ConformanceError::UnqualifiedDependency(
                        item.surface.as_str().into(),
                    ));
                }
                _ => {
                    return Err(ConformanceError::UnqualifiedDependency(
                        item.surface.as_str().into(),
                    ));
                }
            },
            SurfaceUseRole::FundedTarget => {
                if &observed.owner != binding.id() {
                    return Err(ConformanceError::WrongOwner);
                }
                let Some(surface) = binding.surface(&item.surface) else {
                    return Err(ConformanceError::MissingSurface(
                        item.surface.as_str().into(),
                    ));
                };
                match (&surface.status, &observed.state) {
                    (
                        SurfaceStatus::Planned { producing_phase },
                        SurfaceEvidenceState::Planned {
                            producing_phase: seen,
                        },
                    ) if producing_phase == &uses.phase && seen == &uses.phase => {}
                    (SurfaceStatus::Existing, _) => {
                        return Err(ConformanceError::WrongProducingPhase);
                    }
                    _ => return Err(ConformanceError::WrongProducingPhase),
                }
            }
        }
    }
    Ok(())
}

fn packet_identity(
    draft: &PacketDraft,
    inputs: &[PacketInput],
    uses: &DependencyUseSet,
    funded_targets: &[SurfaceKey],
) -> Result<PacketId, ConformanceError> {
    SemanticId::from_fields(vec![
        cfield("phase", Datum::String(draft.phase.clone()))?,
        cfield("owner", draft.owner.to_datum())?,
        cfield("behavior", draft.behavior.to_datum())?,
        cfield("falsifier", draft.falsifier.to_datum())?,
        cfield(
            "allowed-api",
            Datum::Vector(draft.allowed_api.iter().map(SemanticId::to_datum).collect()),
        )?,
        cfield(
            "inputs",
            Datum::Vector(inputs.iter().map(|item| item.semantic.to_datum()).collect()),
        )?,
        cfield("budget", budget_datum(draft.input_budget))?,
        cfield("output-contract", draft.output_contract.to_datum())?,
        cfield(
            "forbidden-edges",
            Datum::Vector(
                draft
                    .forbidden_edges
                    .iter()
                    .map(|edge| {
                        Datum::Vector(vec![
                            Datum::String(edge.from.clone()),
                            Datum::String(edge.to.clone()),
                        ])
                    })
                    .collect(),
            ),
        )?,
        cfield(
            "tests-first",
            Datum::Vector(draft.tests_first.iter().map(SemanticId::to_datum).collect()),
        )?,
        cfield("validation", draft.validation.to_datum())?,
        cfield("docs", draft.docs.to_datum())?,
        cfield(
            "index-impact",
            Datum::String(
                match draft.index_impact {
                    IndexImpact::None => "none",
                    IndexImpact::SourceFacts => "source-facts",
                    IndexImpact::AuthoredFeature => "authored-feature",
                }
                .into(),
            ),
        )?,
        cfield(
            "descent",
            Datum::Vector(vec![
                Datum::Symbol(draft.descent.measure.clone()),
                number(draft.descent.before),
                number(draft.descent.after),
            ]),
        )?,
        cfield(
            "stop",
            Datum::Vector(vec![
                Datum::Symbol(draft.stop.condition.clone()),
                number(u64::from(draft.stop.max_attempts)),
            ]),
        )?,
        cfield("dependency-uses", uses.id().to_datum())?,
        cfield(
            "funded-targets",
            Datum::Vector(
                funded_targets
                    .iter()
                    .map(|value| Datum::String(value.as_str().into()))
                    .collect(),
            ),
        )?,
        cfield(
            "commit-subject",
            draft
                .commit_subject
                .clone()
                .map_or(Datum::Nil, Datum::String),
        )?,
    ])
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

fn cfield(name: &str, value: Datum) -> Result<(Symbol, Datum), ConformanceError> {
    Symbol::checked(name)
        .map(|name| (Symbol::qualified("work", name.name), value))
        .map_err(|_| ConformanceError::InvalidSymbol)
}

fn map_conformance(error: ConformanceError) -> WorkError {
    WorkError::Qualification(error.to_string())
}

#[allow(dead_code)]
fn _scope_anchor(_: &CheckScopeId) {}
