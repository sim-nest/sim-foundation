use sim_conformance_core::*;

// conformance: pure bounded implementation packet construction and admission.
use sim_kernel::Symbol;
use sim_work_core::*;

fn sid<K: IdKind>(value: &str) -> SemanticId<K> {
    SemanticId::from_text(value).unwrap()
}

fn command(value: &str) -> OwnerCommand {
    OwnerCommand::new("repo".into(), vec![value.into()], "sealed".into()).unwrap()
}

fn binding() -> OwnerBinding {
    OwnerBinding::new(
        "law/projection".into(),
        OwnerDisposition::ExtractReusable,
        vec!["sim-projection-core".into()],
        vec![
            BoundSurface {
                key: SurfaceKey::new("api/local-adapter").unwrap(),
                public_name: "adapter::Local".into(),
                status: SurfaceStatus::Existing,
            },
            BoundSurface {
                key: SurfaceKey::new("api/projection").unwrap(),
                public_name: "projection::Projection".into(),
                status: SurfaceStatus::Planned {
                    producing_phase: "NV12.06".into(),
                },
            },
        ],
        vec!["sim-world".into(), "sim-prove".into()],
        "products -> projection -> kernel".into(),
        SurfaceKey::new("route/projection").unwrap(),
        SurfaceKey::new("specimen/projection").unwrap(),
        command("validate"),
        command("docs"),
    )
    .unwrap()
}

fn uses(owner: OwnerBindingId) -> DependencyUseSet {
    DependencyUseSet::new(
        "NV12.06".into(),
        owner,
        vec![
            SurfaceUse {
                surface: SurfaceKey::new("api/local-adapter").unwrap(),
                role: SurfaceUseRole::ReleasedDependency,
            },
            SurfaceUse {
                surface: SurfaceKey::new("api/projection").unwrap(),
                role: SurfaceUseRole::FundedTarget,
            },
        ],
    )
    .unwrap()
}

fn draft(owner: OwnerBindingId, budget: InputBudget) -> PacketDraft {
    PacketDraft {
        phase: "NV12.06".into(),
        owner,
        behavior: sid("behavior/projection"),
        falsifier: sid("falsifier/map-order"),
        allowed_api: vec![sid("type/datum")],
        input_budget: budget,
        output_contract: sid("shape/proposal"),
        forbidden_edges: vec![ForbiddenEdge {
            from: "projection".into(),
            to: "host-process".into(),
        }],
        tests_first: vec![sid("proof/map-order")],
        validation: sid("command/validate"),
        docs: sid("command/docs"),
        index_impact: IndexImpact::SourceFacts,
        descent: DescentCertificate {
            measure: Symbol::qualified("work", "unknowns"),
            before: 2,
            after: 1,
        },
        stop: StopCondition {
            condition: Symbol::qualified("work", "gate-green"),
            max_attempts: 2,
        },
        commit_subject: Some("Implement projection law".into()),
    }
}

fn evidence(owner: &OwnerBindingId, uses: &DependencyUseSet) -> Vec<SurfaceEvidence> {
    vec![
        SurfaceEvidence {
            key: SurfaceKey::new("api/local-adapter").unwrap(),
            owner: sid("owner/adapter"),
            state: SurfaceEvidenceState::Released {
                dependency_uses: uses.id().clone(),
            },
        },
        SurfaceEvidence {
            key: SurfaceKey::new("api/projection").unwrap(),
            owner: owner.clone(),
            state: SurfaceEvidenceState::Planned {
                producing_phase: "NV12.06".into(),
            },
        },
    ]
}

#[test]
fn deterministic_packet_accepts_qualified_dependency_and_planned_target() {
    let binding = binding();
    let uses = uses(binding.id().clone());
    let mut left = FakeInputPort::default();
    let location = left.insert(b"exact source".to_vec());
    let input = PacketInputSpec {
        surface: SurfaceKey::new("api/local-adapter").unwrap(),
        location,
        tokens: 2,
    };
    let budget = InputBudget {
        bytes: 32,
        files: 1,
        tokens: 4,
        output_bytes: 64,
    };
    let first = PacketBuilder::build(
        draft(binding.id().clone(), budget),
        &binding,
        &uses,
        &evidence(binding.id(), &uses),
        vec![input.clone()],
        &mut left,
    )
    .unwrap();
    let mut right = FakeInputPort::default();
    let right_location = right.insert(b"exact source".to_vec());
    let second = PacketBuilder::build(
        draft(binding.id().clone(), budget),
        &binding,
        &uses,
        &evidence(binding.id(), &uses),
        vec![PacketInputSpec {
            location: right_location,
            ..input
        }],
        &mut right,
    )
    .unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(
        first.funded_targets(),
        [SurfaceKey::new("api/projection").unwrap()]
    );
}

#[test]
fn unimplemented_dependency_wrong_target_phase_and_wrong_owner_refuse() {
    let binding = binding();
    let uses = uses(binding.id().clone());
    let budget = InputBudget {
        bytes: 32,
        files: 1,
        tokens: 4,
        output_bytes: 64,
    };
    let input = |port: &mut FakeInputPort| PacketInputSpec {
        surface: SurfaceKey::new("api/local-adapter").unwrap(),
        location: port.insert(b"source".to_vec()),
        tokens: 1,
    };
    let mut port = FakeInputPort::default();
    let mut unimplemented = evidence(binding.id(), &uses);
    unimplemented[0].state = SurfaceEvidenceState::Planned {
        producing_phase: "NV12.05".into(),
    };
    assert!(matches!(
        PacketBuilder::build(
            draft(binding.id().clone(), budget),
            &binding,
            &uses,
            &unimplemented,
            vec![input(&mut port)],
            &mut port,
        ),
        Err(WorkError::Qualification(message)) if message.contains("UnqualifiedDependency")
    ));

    let mut wrong_phase = evidence(binding.id(), &uses);
    wrong_phase[1].state = SurfaceEvidenceState::Planned {
        producing_phase: "NV12.07".into(),
    };
    let mut port = FakeInputPort::default();
    let spec = input(&mut port);
    assert!(matches!(
        PacketBuilder::build(
            draft(binding.id().clone(), budget),
            &binding,
            &uses,
            &wrong_phase,
            vec![spec],
            &mut port,
        ),
        Err(WorkError::Qualification(message)) if message.contains("WrongProducingPhase")
    ));

    let mut wrong_owner = evidence(binding.id(), &uses);
    wrong_owner[1].owner = sid("owner/other");
    let mut port = FakeInputPort::default();
    let spec = input(&mut port);
    assert!(matches!(
        PacketBuilder::build(
            draft(binding.id().clone(), budget),
            &binding,
            &uses,
            &wrong_owner,
            vec![spec],
            &mut port,
        ),
        Err(WorkError::Qualification(message)) if message.contains("WrongOwner")
    ));
}

#[test]
fn undeclared_and_oversize_sources_fail_without_truncation() {
    let binding = binding();
    let uses = uses(binding.id().clone());
    let mut port = FakeInputPort::default();
    let location = port.insert(b"source larger than budget".to_vec());
    let budget = InputBudget {
        bytes: 3,
        files: 1,
        tokens: 1,
        output_bytes: 64,
    };
    let declared = PacketInputSpec {
        surface: SurfaceKey::new("api/local-adapter").unwrap(),
        location: location.clone(),
        tokens: 1,
    };
    assert_eq!(
        PacketBuilder::build(
            draft(binding.id().clone(), budget),
            &binding,
            &uses,
            &evidence(binding.id(), &uses),
            vec![declared],
            &mut port,
        ),
        Err(WorkError::ByteBudget)
    );
    let mut port = FakeInputPort::default();
    let location = port.insert(b"x".to_vec());
    assert!(matches!(
        PacketBuilder::build(
            draft(
                binding.id().clone(),
                InputBudget {
                    bytes: 3,
                    files: 1,
                    tokens: 1,
                    output_bytes: 64
                }
            ),
            &binding,
            &uses,
            &evidence(binding.id(), &uses),
            vec![PacketInputSpec {
                surface: SurfaceKey::new("api/not-declared").unwrap(),
                location,
                tokens: 1,
            }],
            &mut port,
        ),
        Err(WorkError::UndeclaredInput(_))
    ));
}
