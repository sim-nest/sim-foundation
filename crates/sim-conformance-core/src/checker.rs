//! Static checker bindings, exact invocations, receipts, and revocation.

use std::collections::BTreeSet;

use sim_kernel::Datum;

use crate::{
    CheckInputClosureId, CheckInvocationId, CheckScopeId, CheckTemplateId, CheckedSubjectId,
    CheckerBindingId, CommandId, ConformanceError, ConformancePackId, EnvironmentPolicyId,
    ExactCheckCallId, OutputShapeId, OwnerBindingId, ProofCodeId, RevocationSourceId, SemanticId,
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
    entrypoint: String,
    /// Ordered literal and typed slot arguments.
    arguments: Vec<CheckArgument>,
    /// Exact working-directory policy.
    cwd: WorkingDirectoryPolicyId,
    /// Exact environment policy.
    environment: EnvironmentPolicyId,
    /// Expected result Shape.
    output_shape: OutputShapeId,
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

    /// Returns the stable callable entrypoint.
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// Returns the ordered literal and typed-slot arguments.
    pub fn arguments(&self) -> &[CheckArgument] {
        &self.arguments
    }

    /// Returns the working-directory policy.
    pub const fn cwd(&self) -> &WorkingDirectoryPolicyId {
        &self.cwd
    }

    /// Returns the sealed environment policy.
    pub const fn environment(&self) -> &EnvironmentPolicyId {
        &self.environment
    }

    /// Returns the expected output Shape.
    pub const fn output_shape(&self) -> &OutputShapeId {
        &self.output_shape
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
    entrypoint: String,
    /// Fully substituted arguments.
    arguments: Vec<String>,
    /// Working-directory policy.
    cwd: WorkingDirectoryPolicyId,
    /// Environment policy.
    environment: EnvironmentPolicyId,
    /// Expected output Shape.
    output_shape: OutputShapeId,
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

    /// Returns the exact entrypoint.
    pub fn entrypoint(&self) -> &str {
        &self.entrypoint
    }

    /// Returns the fully substituted argument vector.
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// Returns the working-directory policy.
    pub const fn cwd(&self) -> &WorkingDirectoryPolicyId {
        &self.cwd
    }

    /// Returns the sealed environment policy.
    pub const fn environment(&self) -> &EnvironmentPolicyId {
        &self.environment
    }

    /// Returns the expected output Shape.
    pub const fn output_shape(&self) -> &OutputShapeId {
        &self.output_shape
    }
}

/// Immutable declaration for one checker and its allowed scopes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckerBinding {
    id: CheckerBindingId,
    /// Stable checker name.
    checker: String,
    /// Owning module binding.
    owner: OwnerBindingId,
    /// Stable public symbol.
    symbol: String,
    /// Allowed conformance packs.
    packs: Vec<ConformancePackId>,
    /// Receipt Shape.
    receipt_shape: OutputShapeId,
    /// Canonical revocation source.
    revocation_source: RevocationSourceId,
    /// Exact owner validation command.
    owner_validation: CommandId,
    /// Exact owner docs command.
    owner_docs: CommandId,
    /// Exact allowed scopes.
    allowed_scopes: BTreeSet<CheckScopeId>,
    /// Reusable typed invocation template.
    template: CheckTemplate,
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

    /// Returns the stable checker name.
    pub fn checker(&self) -> &str {
        &self.checker
    }

    /// Returns the owning module binding.
    pub const fn owner(&self) -> &OwnerBindingId {
        &self.owner
    }

    /// Returns the public checker symbol.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    /// Returns every allowed pack identity.
    pub fn packs(&self) -> &[ConformancePackId] {
        &self.packs
    }

    /// Returns the receipt Shape identity.
    pub const fn receipt_shape(&self) -> &OutputShapeId {
        &self.receipt_shape
    }

    /// Returns the canonical revocation source.
    pub const fn revocation_source(&self) -> &RevocationSourceId {
        &self.revocation_source
    }

    /// Returns the exact owner validation command identity.
    pub const fn owner_validation(&self) -> &CommandId {
        &self.owner_validation
    }

    /// Returns the exact owner documentation command identity.
    pub const fn owner_docs(&self) -> &CommandId {
        &self.owner_docs
    }

    /// Returns every allowed checker scope.
    pub const fn allowed_scopes(&self) -> &BTreeSet<CheckScopeId> {
        &self.allowed_scopes
    }

    /// Returns the reusable invocation template.
    pub const fn template(&self) -> &CheckTemplate {
        &self.template
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
    binding: CheckerBindingId,
    /// Exact checker code.
    checker_code: ProofCodeId,
    /// Selected pack.
    pack: ConformancePackId,
    /// Checked subject.
    subject: CheckedSubjectId,
    /// Exact allowed scope.
    scope: CheckScopeId,
    /// Fully instantiated call.
    execution: ExactCheckCall,
    /// Exact input closure.
    input_closure: CheckInputClosureId,
}

impl CheckInvocation {
    /// Returns the invocation identity.
    pub const fn id(&self) -> &CheckInvocationId {
        &self.id
    }

    /// Returns the static binding identity.
    pub const fn binding(&self) -> &CheckerBindingId {
        &self.binding
    }

    /// Returns the exact checker code identity.
    pub const fn checker_code(&self) -> &ProofCodeId {
        &self.checker_code
    }

    /// Returns the selected pack identity.
    pub const fn pack(&self) -> &ConformancePackId {
        &self.pack
    }

    /// Returns the checked subject identity.
    pub const fn subject(&self) -> &CheckedSubjectId {
        &self.subject
    }

    /// Returns the exact allowed scope.
    pub const fn scope(&self) -> &CheckScopeId {
        &self.scope
    }

    /// Returns the fully instantiated call.
    pub const fn execution(&self) -> &ExactCheckCall {
        &self.execution
    }

    /// Returns the exact input closure.
    pub const fn input_closure(&self) -> &CheckInputClosureId {
        &self.input_closure
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
