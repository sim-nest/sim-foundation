use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};
use sim_study_core::*;
use std::collections::BTreeSet;

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes(Symbol::qualified("core", "sha256-datum-v1"), [byte; 32])
}
fn coordinate() -> StudyCoordinate {
    StudyCoordinate::new(SubjectRevision::new(id(1)), id(2), id(3), id(4), id(5), 6)
}
fn number(domain: &str, value: &str) -> NumberLiteral {
    NumberLiteral {
        domain: Symbol::qualified("numbers", domain),
        canonical: value.into(),
    }
}

fn datum_id(value: Datum) -> ContentId {
    value.content_id().unwrap()
}

#[test]
fn coordinate_is_opaque_canonical_and_every_field_bears_identity() {
    let base = coordinate();
    let bytes = base.to_datum().canonical_bytes().unwrap();
    let decoded = StudyCoordinate::from_datum(&base.to_datum()).unwrap();
    assert_eq!(decoded, base);
    assert_eq!(decoded.to_datum().canonical_bytes().unwrap(), bytes);
    let variants = [
        StudyCoordinate::new(SubjectRevision::new(id(9)), id(2), id(3), id(4), id(5), 6),
        StudyCoordinate::new(SubjectRevision::new(id(1)), id(9), id(3), id(4), id(5), 6),
        StudyCoordinate::new(SubjectRevision::new(id(1)), id(2), id(9), id(4), id(5), 6),
        StudyCoordinate::new(SubjectRevision::new(id(1)), id(2), id(3), id(9), id(5), 6),
        StudyCoordinate::new(SubjectRevision::new(id(1)), id(2), id(3), id(4), id(9), 6),
        StudyCoordinate::new(SubjectRevision::new(id(1)), id(2), id(3), id(4), id(5), 9),
    ];
    for changed in variants {
        assert_ne!(changed.content_id().unwrap(), base.content_id().unwrap());
    }
}

#[test]
fn operational_fields_are_identity_neutral_and_unsafe_paths_refuse() {
    let key = coordinate().content_id().unwrap();
    for context in [
        OperationalContext {
            safe_path: Some("work/item".into()),
            ..Default::default()
        },
        OperationalContext {
            timestamp: Some("2030-01-01T00:00:00Z".into()),
            ..Default::default()
        },
        OperationalContext {
            retry_policy: Some("bounded-three".into()),
            ..Default::default()
        },
        OperationalContext {
            placement: Some("site-a".into()),
            ..Default::default()
        },
    ] {
        context.validate().unwrap();
        assert_eq!(coordinate().content_id().unwrap(), key);
    }
    assert_eq!(
        OperationalContext {
            safe_path: Some("../secret".into()),
            ..Default::default()
        }
        .validate(),
        Err(StudyError::UnsafePath)
    );
}

#[test]
fn every_non_coordinate_record_field_bears_identity() {
    fn distinct(base: Datum, variants: impl IntoIterator<Item = Datum>) {
        let base = datum_id(base);
        for variant in variants {
            assert_ne!(datum_id(variant), base);
        }
    }

    let attempt = Attempt::new(coordinate(), 1, EvidenceClass::Publishable);
    let mut terminal_attempt = attempt.clone();
    terminal_attempt
        .transition(AttemptOutcome::Observed)
        .unwrap();
    distinct(
        attempt.to_datum(),
        [
            Attempt::new(
                StudyCoordinate::new(SubjectRevision::new(id(9)), id(2), id(3), id(4), id(5), 6),
                1,
                EvidenceClass::Publishable,
            )
            .to_datum(),
            Attempt::new(coordinate(), 2, EvidenceClass::Publishable).to_datum(),
            Attempt::new(coordinate(), 1, EvidenceClass::ReportOnly).to_datum(),
            terminal_attempt.to_datum(),
        ],
    );

    let facet = FacetObservation {
        coordinate: id(1),
        facet: Symbol::qualified("study", "answer"),
        value: Datum::Bool(true),
        evidence: EvidenceClass::Publishable,
    };
    distinct(
        facet.to_datum(),
        [
            FacetObservation {
                coordinate: id(2),
                ..facet.clone()
            }
            .to_datum(),
            FacetObservation {
                facet: Symbol::qualified("study", "score"),
                ..facet.clone()
            }
            .to_datum(),
            FacetObservation {
                value: Datum::Bool(false),
                ..facet.clone()
            }
            .to_datum(),
            FacetObservation {
                evidence: EvidenceClass::ReportOnly,
                ..facet
            }
            .to_datum(),
        ],
    );

    let resource = ResourceEvent {
        coordinate: id(1),
        resource: Symbol::qualified("study", "energy"),
        amount: number("decimal", "1"),
        evidence: EvidenceClass::Publishable,
    };
    distinct(
        resource.to_datum(),
        [
            ResourceEvent {
                coordinate: id(2),
                ..resource.clone()
            }
            .to_datum(),
            ResourceEvent {
                resource: Symbol::qualified("study", "time"),
                ..resource.clone()
            }
            .to_datum(),
            ResourceEvent {
                amount: number("decimal", "2"),
                ..resource.clone()
            }
            .to_datum(),
            ResourceEvent {
                evidence: EvidenceClass::ReportOnly,
                ..resource
            }
            .to_datum(),
        ],
    );

    let treatment = TreatmentRecord {
        treatment: id(1),
        contract: id(2),
        evidence: EvidenceClass::Publishable,
    };
    distinct(
        treatment.to_datum(),
        [
            TreatmentRecord {
                treatment: id(3),
                ..treatment.clone()
            }
            .to_datum(),
            TreatmentRecord {
                contract: id(3),
                ..treatment.clone()
            }
            .to_datum(),
            TreatmentRecord {
                evidence: EvidenceClass::ReportOnly,
                ..treatment
            }
            .to_datum(),
        ],
    );

    let estimate = EstimateRecord::new(
        number("decimal", "1"),
        number("decimal", "0"),
        number("decimal", "2"),
        number("decimal", ".95"),
        id(1),
        EvidenceClass::Publishable,
    )
    .unwrap();
    distinct(
        estimate.to_datum(),
        [
            EstimateRecord::new(
                number("decimal", "3"),
                estimate.lower.clone(),
                estimate.upper.clone(),
                estimate.confidence.clone(),
                estimate.inference.clone(),
                estimate.evidence,
            )
            .unwrap()
            .to_datum(),
            EstimateRecord::new(
                estimate.point.clone(),
                number("decimal", "-1"),
                estimate.upper.clone(),
                estimate.confidence.clone(),
                estimate.inference.clone(),
                estimate.evidence,
            )
            .unwrap()
            .to_datum(),
            EstimateRecord::new(
                estimate.point.clone(),
                estimate.lower.clone(),
                number("decimal", "4"),
                estimate.confidence.clone(),
                estimate.inference.clone(),
                estimate.evidence,
            )
            .unwrap()
            .to_datum(),
            EstimateRecord::new(
                estimate.point.clone(),
                estimate.lower.clone(),
                estimate.upper.clone(),
                number("decimal", ".99"),
                estimate.inference.clone(),
                estimate.evidence,
            )
            .unwrap()
            .to_datum(),
            EstimateRecord::new(
                estimate.point.clone(),
                estimate.lower.clone(),
                estimate.upper.clone(),
                estimate.confidence.clone(),
                id(2),
                estimate.evidence,
            )
            .unwrap()
            .to_datum(),
            EstimateRecord::new(
                estimate.point.clone(),
                estimate.lower.clone(),
                estimate.upper.clone(),
                estimate.confidence.clone(),
                estimate.inference.clone(),
                EvidenceClass::ReportOnly,
            )
            .unwrap()
            .to_datum(),
        ],
    );

    let closure = BTreeSet::from([id(1), id(2), id(3)]);
    let decision = DecisionRecord::new(
        id(1),
        vec![id(2)],
        vec![id(3)],
        Symbol::qualified("study", "keep"),
        vec![Symbol::qualified("study", "quality")],
        Some(id(1)),
        EvidenceClass::Publishable,
        &closure,
    )
    .unwrap();
    distinct(
        decision.to_datum(),
        [
            DecisionRecord::new(
                id(2),
                decision.evidence_ids.clone(),
                decision.frontier.clone(),
                decision.verdict.clone(),
                decision.decisive_dimensions.clone(),
                decision.expiry.clone(),
                decision.evidence,
                &closure,
            )
            .unwrap()
            .to_datum(),
            DecisionRecord::new(
                decision.policy.clone(),
                vec![id(3)],
                decision.frontier.clone(),
                decision.verdict.clone(),
                decision.decisive_dimensions.clone(),
                decision.expiry.clone(),
                decision.evidence,
                &closure,
            )
            .unwrap()
            .to_datum(),
            DecisionRecord::new(
                decision.policy.clone(),
                decision.evidence_ids.clone(),
                vec![id(2)],
                decision.verdict.clone(),
                decision.decisive_dimensions.clone(),
                decision.expiry.clone(),
                decision.evidence,
                &closure,
            )
            .unwrap()
            .to_datum(),
            DecisionRecord::new(
                decision.policy.clone(),
                decision.evidence_ids.clone(),
                decision.frontier.clone(),
                Symbol::qualified("study", "reject"),
                decision.decisive_dimensions.clone(),
                decision.expiry.clone(),
                decision.evidence,
                &closure,
            )
            .unwrap()
            .to_datum(),
            DecisionRecord::new(
                decision.policy.clone(),
                decision.evidence_ids.clone(),
                decision.frontier.clone(),
                decision.verdict.clone(),
                vec![Symbol::qualified("study", "cost")],
                decision.expiry.clone(),
                decision.evidence,
                &closure,
            )
            .unwrap()
            .to_datum(),
            DecisionRecord::new(
                decision.policy.clone(),
                decision.evidence_ids.clone(),
                decision.frontier.clone(),
                decision.verdict.clone(),
                decision.decisive_dimensions.clone(),
                None,
                decision.evidence,
                &closure,
            )
            .unwrap()
            .to_datum(),
            DecisionRecord::new(
                decision.policy.clone(),
                decision.evidence_ids.clone(),
                decision.frontier.clone(),
                decision.verdict.clone(),
                decision.decisive_dimensions.clone(),
                decision.expiry.clone(),
                EvidenceClass::ReportOnly,
                &closure,
            )
            .unwrap()
            .to_datum(),
        ],
    );

    let selection = SelectionRecord {
        decision: id(1),
        subject: SubjectRevision::new(id(2)),
        role: Symbol::qualified("study", "primary"),
        evidence: EvidenceClass::Publishable,
    };
    distinct(
        selection.to_datum(),
        [
            SelectionRecord {
                decision: id(3),
                ..selection.clone()
            }
            .to_datum(),
            SelectionRecord {
                subject: SubjectRevision::new(id(3)),
                ..selection.clone()
            }
            .to_datum(),
            SelectionRecord {
                role: Symbol::qualified("study", "reference"),
                ..selection.clone()
            }
            .to_datum(),
            SelectionRecord {
                evidence: EvidenceClass::ReportOnly,
                ..selection
            }
            .to_datum(),
        ],
    );
}

#[test]
fn terminal_transition_is_total_and_single_assignment() {
    for outcome in [
        AttemptOutcome::Observed,
        AttemptOutcome::Unsupported,
        AttemptOutcome::Unresolved,
        AttemptOutcome::Quarantined,
    ] {
        let mut attempt = Attempt::new(coordinate(), 1, EvidenceClass::Publishable);
        attempt.transition(outcome).unwrap();
        assert_eq!(attempt.outcome(), Some(outcome));
        assert_eq!(
            attempt.transition(outcome),
            Err(StudyError::AlreadyTerminal)
        );
        let other = if outcome == AttemptOutcome::Observed {
            AttemptOutcome::Unsupported
        } else {
            AttemptOutcome::Observed
        };
        assert_eq!(
            attempt.transition(other),
            Err(StudyError::ConflictingTerminalOutcome)
        );
    }
}

#[test]
fn estimate_domains_and_provenance_fail_closed() {
    assert!(
        EstimateRecord::new(
            number("decimal", "1"),
            number("decimal", "0"),
            number("decimal", "2"),
            number("decimal", ".95"),
            id(7),
            EvidenceClass::Publishable
        )
        .is_ok()
    );
    assert_eq!(
        EstimateRecord::new(
            number("decimal", "1"),
            number("binary", "0"),
            number("decimal", "2"),
            number("decimal", ".95"),
            id(7),
            EvidenceClass::Publishable
        ),
        Err(StudyError::IncompatibleNumberDomain)
    );
    let closure = BTreeSet::from([id(1)]);
    assert!(matches!(
        DecisionRecord::new(
            id(9),
            vec![id(2)],
            vec![],
            Symbol::qualified("study", "keep"),
            vec![],
            None,
            EvidenceClass::ReportOnly,
            &closure
        ),
        Err(StudyError::MissingProvenance(_))
    ));
}

#[test]
fn evidence_only_weakens_and_exports_exclude_private_or_secret_values() {
    assert_eq!(
        EvidenceClass::derive([EvidenceClass::Publishable, EvidenceClass::PrivateLocal]),
        EvidenceClass::PrivateLocal
    );
    assert_eq!(
        EvidenceClass::PrivateLocal.permits_derivation(EvidenceClass::Publishable),
        Err(StudyError::EvidenceStrengthened)
    );
    let private = FacetObservation {
        coordinate: id(1),
        facet: Symbol::qualified("study", "answer"),
        value: Datum::Bool(true),
        evidence: EvidenceClass::PrivateLocal,
    };
    assert_eq!(private.export_public(), Err(StudyError::PrivateExport));
    let secret = FacetObservation {
        coordinate: id(1),
        facet: Symbol::qualified("study", "answer"),
        value: Datum::Node {
            tag: Symbol::qualified("study", "secret"),
            fields: vec![],
        },
        evidence: EvidenceClass::Publishable,
    };
    assert_eq!(secret.export_public(), Err(StudyError::SecretForbidden));
}

#[test]
fn shapes_reject_versions_duplicates_order_and_overflow() {
    let mut datum = coordinate().to_datum();
    COORDINATE_SHAPE.check(&datum).unwrap();
    let Datum::Node { fields, .. } = &mut datum else {
        unreachable!()
    };
    fields.swap(1, 2);
    assert!(COORDINATE_SHAPE.check(&datum).is_err());
    let mut wrong_version = coordinate().to_datum();
    let Datum::Node { fields, .. } = &mut wrong_version else {
        unreachable!()
    };
    fields[0].1 = Datum::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "u32"),
        canonical: "2".into(),
    });
    assert_eq!(
        StudyCoordinate::from_datum(&wrong_version),
        Err(StudyError::UnknownSchemaVersion(2))
    );
    let huge = "x".repeat(4097);
    assert!(
        EstimateRecord::new(
            number("decimal", &huge),
            number("decimal", "0"),
            number("decimal", "2"),
            number("decimal", ".95"),
            id(7),
            EvidenceClass::Publishable
        )
        .is_err()
    );
}
