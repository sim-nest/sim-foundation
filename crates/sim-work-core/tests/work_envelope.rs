use sim_conformance_core::{OutputShapeId, SemanticId};
use sim_kernel::{Datum, Symbol};
use sim_work_core::{
    AttemptPolicy, CapabilityGrant, DescentCertificate, InputBudget, ProgressContract,
    ResourceNeed, SemanticInputId, WorkEnvelope, WorkError,
};

fn semantic<K: sim_conformance_core::IdKind>(value: &str) -> SemanticId<K> {
    SemanticId::from_text(value).unwrap()
}

fn envelope(capabilities: Vec<Symbol>) -> Result<WorkEnvelope, WorkError> {
    WorkEnvelope::new(
        vec![semantic::<sim_work_core::SemanticInputKind>("input/source")],
        InputBudget {
            bytes: 1_024,
            files: 2,
            tokens: 256,
            output_bytes: 512,
        },
        semantic::<sim_conformance_core::OutputShapeKind>("shape/work-return"),
        vec![ResourceNeed {
            kind: Symbol::qualified("resource", "cpu"),
            requirement: Datum::String("bounded".into()),
        }],
        CapabilityGrant { capabilities },
        ProgressContract {
            beat_work_units: 10,
            max_beats: 8,
        },
        AttemptPolicy {
            max_attempts: 2,
            retry_malformed_once: true,
        },
        DescentCertificate {
            measure: Symbol::qualified("work", "remaining-files"),
            before: 2,
            after: 1,
        },
    )
}

#[test]
fn envelope_identity_covers_the_complete_immutable_contract() {
    let capabilities = vec![
        Symbol::qualified("capability", "read"),
        Symbol::qualified("capability", "write"),
    ];
    let first = envelope(capabilities.clone()).unwrap();
    let second = envelope(capabilities).unwrap();
    assert_eq!(first.id(), second.id());
    assert_eq!(first.semantic_inputs().len(), 1);
    assert_eq!(first.resources().len(), 1);
    assert_eq!(
        first.output_shape(),
        &semantic::<sim_conformance_core::OutputShapeKind>("shape/work-return")
    );

    let changed = envelope(vec![Symbol::qualified("capability", "read")]).unwrap();
    assert_ne!(first.id(), changed.id());
}

#[test]
fn envelope_rejects_ambiguous_grants_and_non_decreasing_work() {
    let repeated = Symbol::qualified("capability", "read");
    assert_eq!(
        envelope(vec![repeated.clone(), repeated]),
        Err(WorkError::InvalidPacket("capability order"))
    );

    let input: SemanticInputId = semantic("input/source");
    let output: OutputShapeId = semantic("shape/work-return");
    assert_eq!(
        WorkEnvelope::new(
            vec![input],
            InputBudget {
                bytes: 1,
                files: 1,
                tokens: 1,
                output_bytes: 1,
            },
            output,
            vec![],
            CapabilityGrant::default(),
            ProgressContract {
                beat_work_units: 1,
                max_beats: 1,
            },
            AttemptPolicy {
                max_attempts: 1,
                retry_malformed_once: false,
            },
            DescentCertificate {
                measure: Symbol::qualified("work", "remaining"),
                before: 1,
                after: 1,
            },
        ),
        Err(WorkError::NoDescent)
    );
}
