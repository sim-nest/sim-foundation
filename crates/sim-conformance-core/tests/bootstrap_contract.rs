use std::collections::BTreeSet;

// conformance: neutral checker binding, identity, support graph, and receipt contract.

use sim_conformance_core::*;
use sim_kernel::{Datum, NumberLiteral, Symbol};

fn sid<K: IdKind>(value: &str) -> SemanticId<K> {
    SemanticId::from_text(value).unwrap()
}

fn command(value: &str) -> OwnerCommand {
    OwnerCommand {
        id: sid(value),
        cwd: "repo".into(),
        argv: vec![value.into()],
        environment: "sealed".into(),
    }
}

fn owner_binding() -> OwnerBinding {
    OwnerBinding::new(
        "law/demo".into(),
        OwnerDisposition::ExtractReusable,
        vec!["sim-demo-core".into()],
        vec![BoundSurface {
            key: SurfaceKey::new("api/demo").unwrap(),
            public_name: "demo::Api".into(),
            status: SurfaceStatus::Planned {
                producing_phase: "P2".into(),
            },
        }],
        vec!["consumer-a".into(), "consumer-b".into()],
        "consumer -> demo -> kernel".into(),
        SurfaceKey::new("route/demo").unwrap(),
        SurfaceKey::new("specimen/demo").unwrap(),
        command("validate"),
        command("docs"),
    )
    .unwrap()
}

fn checker_binding(owner: OwnerBindingId) -> CheckerBinding {
    let scope_x: CheckScopeId = sid("scope/x");
    let scope_y: CheckScopeId = sid("scope/y");
    let template = CheckTemplate::new(
        "check".into(),
        vec![
            CheckArgument::Literal("--binding".into()),
            CheckArgument::BindingSlot,
            CheckArgument::Literal("--subject".into()),
            CheckArgument::SubjectSlot,
            CheckArgument::Literal("--scope".into()),
            CheckArgument::ScopeSlot,
        ],
        sid("cwd/repo"),
        sid("env/sealed"),
        sid("shape/result"),
    )
    .unwrap();
    CheckerBinding::new(
        "C-DEMO".into(),
        owner,
        "demo::check".into(),
        vec![sid("pack/demo")],
        sid("shape/receipt"),
        sid("revocation/demo"),
        sid("command/validation"),
        sid("command/docs"),
        [scope_x, scope_y].into_iter().collect(),
        template,
    )
    .unwrap()
}

#[test]
fn semantic_vector_is_canonical_and_storage_is_a_distinct_role() {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct VectorKind;
    impl IdKind for VectorKind {
        const DOMAIN: &'static str = "test/vector-v1";
    }
    let id = SemanticId::<VectorKind>::from_fields(vec![(
        Symbol::qualified("vector", "answer"),
        Datum::Number(NumberLiteral {
            domain: Symbol::qualified("numbers", "u64"),
            canonical: "42".into(),
        }),
    )])
    .unwrap();
    let actual = id
        .content_id()
        .bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual,
        "c13d17c133a40252864f27f34aeadaeb273a83dadd2f9726754d31a2e02d6e68"
    );

    let bytes = b"same semantic transport";
    let location = StorageId::for_bytes(bytes);
    assert!(location.verify(bytes).is_ok());
    assert_eq!(
        location.verify(b"different"),
        Err(ConformanceError::StorageDigestMismatch)
    );
}

#[test]
fn one_binding_instantiates_four_distinct_calls_without_changing() {
    let owner = owner_binding();
    let binding = checker_binding(owner.id().clone());
    let original = binding.id().clone();
    let code: ProofCodeId = sid("code/v1");
    let pack: ConformancePackId = sid("pack/demo");
    let closure: CheckInputClosureId = sid("closure/one");
    let subjects: [CheckedSubjectId; 2] = [sid("subject/a"), sid("subject/b")];
    let scopes: [CheckScopeId; 2] = [sid("scope/x"), sid("scope/y")];
    let mut ids = BTreeSet::new();
    for subject in subjects {
        for scope in scopes.clone() {
            let invocation = binding
                .instantiate(
                    code.clone(),
                    pack.clone(),
                    subject.clone(),
                    scope,
                    closure.clone(),
                )
                .unwrap();
            ids.insert(invocation.id().clone());
        }
    }
    assert_eq!(ids.len(), 4);
    assert_eq!(binding.id(), &original);
}

#[test]
fn wrong_scope_substitution_and_unknown_revocation_fail_closed() {
    let owner = owner_binding();
    let binding = checker_binding(owner.id().clone());
    let build = |subject: &str, scope: &str| {
        binding
            .instantiate(
                sid("code/v1"),
                sid("pack/demo"),
                sid(subject),
                sid(scope),
                sid("closure/one"),
            )
            .unwrap()
    };
    assert_eq!(
        binding.instantiate(
            sid("code/v1"),
            sid("pack/demo"),
            sid("subject/a"),
            sid("scope/not-allowed"),
            sid("closure/one"),
        ),
        Err(ConformanceError::UnauthorizedScope)
    );
    let invocation_a = build("subject/a", "scope/x");
    let invocation_b = build("subject/b", "scope/x");
    let receipt = CheckerReceipt::passing(
        &invocation_a,
        sid("result/pass"),
        EvidenceGrade::Bootstrap,
        sid("provenance/local"),
        sid("policy/v1"),
        sid("support/empty"),
        RevocationStatus::Current,
    )
    .unwrap();
    assert_eq!(
        receipt.verify(&binding, &invocation_b, RevocationStatus::Current),
        Err(ConformanceError::InvocationMismatch(
            "binding or invocation"
        ))
    );
    assert_eq!(
        receipt.verify(&binding, &invocation_a, RevocationStatus::Unknown),
        Err(ConformanceError::RevocationUnknownOrActive)
    );
}

#[test]
fn templates_and_support_graphs_reject_cycles_before_dispatch() {
    assert_eq!(
        CheckTemplate::new(
            "check".into(),
            vec![CheckArgument::BindingSlot, CheckArgument::SubjectSlot],
            sid("cwd/repo"),
            sid("env/sealed"),
            sid("shape/result"),
        ),
        Err(ConformanceError::InvalidTemplate("scope slot"))
    );
    let mut graph = SupportGraph::default();
    graph
        .add_support(SupportNode("binding".into()), SupportNode("command".into()))
        .unwrap();
    graph
        .add_support(SupportNode("command".into()), SupportNode("receipt".into()))
        .unwrap();
    graph
        .add_support(SupportNode("receipt".into()), SupportNode("binding".into()))
        .unwrap();
    assert!(matches!(
        graph.validate(),
        Err(ConformanceError::SupportCycle(_))
    ));
}

#[test]
fn activation_scope_cannot_resolve_a_planned_surface() {
    let binding = owner_binding();
    assert_eq!(
        BindingQualificationReceipt::new(
            &binding,
            QualificationScope::Activation {
                map: sid("activation/map"),
            },
            vec![CheckedSurfaceRef {
                key: SurfaceKey::new("api/demo").unwrap(),
                source: sid("source/demo"),
                package: sid("package/demo"),
                receipt: sid("receipt/demo"),
            }],
            sid("subject/activation"),
            vec![sid("receipt/ownership")],
        ),
        Err(ConformanceError::ActivationIsNotProductionEvidence)
    );
}
