//! Static checker bindings, exact invocations, receipts, and revocation.

use std::collections::BTreeSet;

use sim_kernel::Datum;

use crate::{
    CheckInputClosureId, CheckInvocationId, CheckScopeId, CheckTemplateId, CheckedSubjectId,
    CheckerBindingId, CheckerReceiptId, CheckerResultId, CommandId, ConformanceError,
    ConformancePackId, EnvironmentPolicyId, EvidenceProvenanceId, EvidenceSetId, ExactCheckCallId,
    OutputShapeId, OwnerBindingId, PolicyId, ProofCodeId, RevocationSourceId, SemanticId,
    WorkingDirectoryPolicyId, field, ids, qualified, strings, text,
};

/// One typed argument in a reusable checker call template.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckArgument {
    /// Exact invariant text chosen by the checker owner.
    Literal(String),
    /// Static binding id, supplied during instantiation.
    BindingSlot,
    /// Checked subject id, supplied during instantiation.
    SubjectSlot,
    /// Authorized scope id, supplied during instantiation.
    ScopeSlot,
}

impl CheckArgument {
    fn datum(&self) -> Result<Datum, ConformanceError> {
        let (tag, value) = match self {
            Self::Literal(value) => ("literal", text(value.clone())),
            Self::BindingSlot => ("binding-slot", Datum::Nil),
            Self::SubjectSlot => ("subject-slot", Datum::Nil),
            Self::ScopeSlot => ("scope-slot", Datum::Nil),
        };
        Ok(Datum::Node {
            tag: qualified(&format!("conformance/check-argument-{tag}-v1"))?,
            fields: vec![field("value", value)?],
        })
    }
}

/// A typed checker template without any concrete binding, subject, scope, or result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckTemplate {
    id: CheckTemplateId,
    /// Stable callable entrypoint.
    pub entrypoint: String,
    /// Ordered literal and typed slot arguments.
    pub arguments: Vec<CheckArgument>,
    /// Exact working-directory policy.
    pub cwd: WorkingDirectoryPolicyId,
    /// Exact environment policy.
    pub environment: EnvironmentPolicyId,
    /// Expected result Shape.
    pub output_shape: OutputShapeId,
}

impl CheckTemplate {
    /// Validates exactly one binding, subject, and scope slot and identifies the template.
    pub fn new(
        entrypoint: String,
        arguments: Vec<CheckArgument>,
        cwd: WorkingDirectoryPolicyId,
        environment: EnvironmentPolicyId,
        output_shape: OutputShapeId,
    ) -> Result<Self, ConformanceError> {
        if entrypoint.is_empty() || arguments.is_empty() {
            return Err(ConformanceError::InvalidTemplate("empty template"));
        }
        for (name, count) in [
            (
                "binding slot",
                arguments
                    .iter()
                    .filter(|value| matches!(value, CheckArgument::BindingSlot))
                    .count(),
            ),
            (
                "subject slot",
                arguments
                    .iter()
                    .filter(|value| matches!(value, CheckArgument::SubjectSlot))
                    .count(),
            ),
            (
                "scope slot",
                arguments
                    .iter()
                    .filter(|value| matches!(value, CheckArgument::ScopeSlot))
                    .count(),
            ),
        ] {
            if count != 1 {
                return Err(ConformanceError::InvalidTemplate(name));
            }
        }
        let id = SemanticId::from_fields(vec![
            field("entrypoint", text(entrypoint.clone()))?,
            field(
                "arguments",
                Datum::Vector(
                    arguments
                        .iter()
                        .map(CheckArgument::datum)
                        .collect::<Result<_, _>>()?,
                ),
            )?,
            field("cwd", cwd.to_datum())?,
            field("environment", environment.to_datum())?,
            field("output-shape", output_shape.to_datum())?,
        ])?;
        Ok(Self {
            id,
            entrypoint,
            arguments,
            cwd,
            environment,
            output_shape,
        })
    }

    /// Returns the immutable template identity.
    pub const fn id(&self) -> &CheckTemplateId {
        &self.id
    }

    fn instantiate(
        &self,
        binding: &CheckerBindingId,
        subject: &CheckedSubjectId,
        scope: &CheckScopeId,
    ) -> Result<ExactCheckCall, ConformanceError> {
        let arguments = self
            .arguments
            .iter()
            .map(|argument| match argument {
                CheckArgument::Literal(value) => value.clone(),
                CheckArgument::BindingSlot => render(binding.content_id()),
                CheckArgument::SubjectSlot => render(subject.content_id()),
                CheckArgument::ScopeSlot => render(scope.content_id()),
            })
            .collect::<Vec<_>>();
        ExactCheckCall::new(
            self.entrypoint.clone(),
            arguments,
            self.cwd.clone(),
            self.environment.clone(),
            self.output_shape.clone(),
        )
    }
}

/// One exact instantiated native or command call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactCheckCall {
    id: ExactCheckCallId,
    /// Entrypoint name.
    pub entrypoint: String,
    /// Fully substituted arguments.
    pub arguments: Vec<String>,
    /// Working-directory policy.
    pub cwd: WorkingDirectoryPolicyId,
    /// Environment policy.
    pub environment: EnvironmentPolicyId,
    /// Expected output Shape.
    pub output_shape: OutputShapeId,
}

impl ExactCheckCall {
    fn new(
        entrypoint: String,
        arguments: Vec<String>,
        cwd: WorkingDirectoryPolicyId,
        environment: EnvironmentPolicyId,
        output_shape: OutputShapeId,
    ) -> Result<Self, ConformanceError> {
        let id = SemanticId::from_fields(vec![
            field("entrypoint", text(entrypoint.clone()))?,
            field("arguments", strings(arguments.clone()))?,
            field("cwd", cwd.to_datum())?,
            field("environment", environment.to_datum())?,
            field("output-shape", output_shape.to_datum())?,
        ])?;
        Ok(Self {
            id,
            entrypoint,
            arguments,
            cwd,
            environment,
            output_shape,
        })
    }

    /// Returns the exact call identity.
    pub const fn id(&self) -> &ExactCheckCallId {
        &self.id
    }
}

/// Immutable declaration for one checker and its allowed scopes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerBinding {
    id: CheckerBindingId,
    /// Stable checker name.
    pub checker: String,
    /// Owning module binding.
    pub owner: OwnerBindingId,
    /// Stable public symbol.
    pub symbol: String,
    /// Allowed conformance packs.
    pub packs: Vec<ConformancePackId>,
    /// Receipt Shape.
    pub receipt_shape: OutputShapeId,
    /// Canonical revocation source.
    pub revocation_source: RevocationSourceId,
    /// Exact owner validation command.
    pub owner_validation: CommandId,
    /// Exact owner docs command.
    pub owner_docs: CommandId,
    /// Exact allowed scopes.
    pub allowed_scopes: BTreeSet<CheckScopeId>,
    /// Reusable typed invocation template.
    pub template: CheckTemplate,
}

impl CheckerBinding {
    /// Constructs a static binding that contains no concrete invocation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        checker: String,
        owner: OwnerBindingId,
        symbol: String,
        packs: Vec<ConformancePackId>,
        receipt_shape: OutputShapeId,
        revocation_source: RevocationSourceId,
        owner_validation: CommandId,
        owner_docs: CommandId,
        allowed_scopes: BTreeSet<CheckScopeId>,
        template: CheckTemplate,
    ) -> Result<Self, ConformanceError> {
        if checker.is_empty() || symbol.is_empty() || packs.is_empty() || allowed_scopes.is_empty()
        {
            return Err(ConformanceError::UnresolvedBinding);
        }
        let id = SemanticId::from_fields(vec![
            field("checker", text(checker.clone()))?,
            field("owner", owner.to_datum())?,
            field("symbol", text(symbol.clone()))?,
            field("packs", ids(&packs))?,
            field("receipt-shape", receipt_shape.to_datum())?,
            field("revocation-source", revocation_source.to_datum())?,
            field("owner-validation", owner_validation.to_datum())?,
            field("owner-docs", owner_docs.to_datum())?,
            field(
                "allowed-scopes",
                ids(&allowed_scopes.iter().cloned().collect::<Vec<_>>()),
            )?,
            field("template", template.id().to_datum())?,
        ])?;
        Ok(Self {
            id,
            checker,
            owner,
            symbol,
            packs,
            receipt_shape,
            revocation_source,
            owner_validation,
            owner_docs,
            allowed_scopes,
            template,
        })
    }

    /// Returns the static binding identity.
    pub const fn id(&self) -> &CheckerBindingId {
        &self.id
    }

    /// Instantiates the exact call only after subject and scope exist.
    pub fn instantiate(
        &self,
        checker_code: ProofCodeId,
        pack: ConformancePackId,
        subject: CheckedSubjectId,
        scope: CheckScopeId,
        input_closure: CheckInputClosureId,
    ) -> Result<CheckInvocation, ConformanceError> {
        if !self.packs.contains(&pack) {
            return Err(ConformanceError::InvocationMismatch("pack"));
        }
        if !self.allowed_scopes.contains(&scope) {
            return Err(ConformanceError::UnauthorizedScope);
        }
        let execution = self.template.instantiate(&self.id, &subject, &scope)?;
        let id = SemanticId::from_fields(vec![
            field("binding", self.id.to_datum())?,
            field("checker-code", checker_code.to_datum())?,
            field("pack", pack.to_datum())?,
            field("subject", subject.to_datum())?,
            field("scope", scope.to_datum())?,
            field("execution", execution.id().to_datum())?,
            field("input-closure", input_closure.to_datum())?,
        ])?;
        Ok(CheckInvocation {
            id,
            binding: self.id.clone(),
            checker_code,
            pack,
            subject,
            scope,
            execution,
            input_closure,
        })
    }
}

/// Exact invocation of a static checker binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckInvocation {
    id: CheckInvocationId,
    /// Static checker binding.
    pub binding: CheckerBindingId,
    /// Exact checker code.
    pub checker_code: ProofCodeId,
    /// Selected pack.
    pub pack: ConformancePackId,
    /// Checked subject.
    pub subject: CheckedSubjectId,
    /// Exact allowed scope.
    pub scope: CheckScopeId,
    /// Fully instantiated call.
    pub execution: ExactCheckCall,
    /// Exact input closure.
    pub input_closure: CheckInputClosureId,
}

impl CheckInvocation {
    /// Returns the invocation identity.
    pub const fn id(&self) -> &CheckInvocationId {
        &self.id
    }
}

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
    /// Static binding.
    pub binding: CheckerBindingId,
    /// Exact invocation.
    pub invocation: CheckInvocationId,
    /// Exact checker code.
    pub checker_code: ProofCodeId,
    /// Pack identity.
    pub pack: ConformancePackId,
    /// Scope identity.
    pub scope: CheckScopeId,
    /// Checked subject.
    pub subject: CheckedSubjectId,
    /// Typed passing result.
    pub result: CheckerResultId,
    /// Evidence grade.
    pub grade: EvidenceGrade,
    /// Execution provenance.
    pub provenance: EvidenceProvenanceId,
    /// Checker-owner policy.
    pub policy: PolicyId,
    /// Acyclic supporting evidence.
    pub support: EvidenceSetId,
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
            field("binding", invocation.binding.to_datum())?,
            field("invocation", invocation.id.to_datum())?,
            field("checker-code", invocation.checker_code.to_datum())?,
            field("pack", invocation.pack.to_datum())?,
            field("scope", invocation.scope.to_datum())?,
            field("subject", invocation.subject.to_datum())?,
            field("result", result.to_datum())?,
            field("grade", grade.datum())?,
            field("provenance", provenance.to_datum())?,
            field("policy", policy.to_datum())?,
            field("support", support.to_datum())?,
        ])?;
        Ok(Self {
            id,
            binding: invocation.binding.clone(),
            invocation: invocation.id.clone(),
            checker_code: invocation.checker_code.clone(),
            pack: invocation.pack.clone(),
            scope: invocation.scope.clone(),
            subject: invocation.subject.clone(),
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
        if self.checker_code != invocation.checker_code
            || self.pack != invocation.pack
            || self.scope != invocation.scope
            || self.subject != invocation.subject
        {
            return Err(ConformanceError::InvocationMismatch(
                "copied invocation fields",
            ));
        }
        if !binding.allowed_scopes.contains(&self.scope) {
            return Err(ConformanceError::UnauthorizedScope);
        }
        Ok(())
    }

    /// Returns the receipt identity.
    pub const fn id(&self) -> &CheckerReceiptId {
        &self.id
    }
}

fn render(id: &sim_kernel::ContentId) -> String {
    let mut out = format!("{}:", id.algorithm.as_qualified_str());
    for byte in id.bytes {
        use std::fmt::Write as _;
        write!(out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}
