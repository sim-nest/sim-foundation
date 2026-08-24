use sim_index_core::{
    AnchorId, DeclarationFact, DeclarationRole, DiscoveredAnchor, DiscoveredSpecimen,
    DiscoveredSurface, FeatureDraft, FeatureId, FeatureRecord, GrammarContract, IndexDoc,
    IndexEdge, IndexRow, ProtocolRelation, ProtocolResolution, RouteId, RouteRecord, RouteStep,
    SourceCompleteness, SourceLocation, SourceReachability, SourceUnit, SpecimenId, SubjectId,
    SubjectRecord, SurfaceId, SyntaxBound, UnresolvedReason, Visibility, canonical_feature_key,
};
use sim_index_vault_core::{
    ClaimCertificate, ClaimSite, DerivedClaim, ProjectionError, Relation, RelationOrigin,
    VaultGranularity, VaultNoteId, VaultNoteKind, VaultProjection, validate_incoming_relations,
};

fn fixture() -> IndexDoc {
    let subject = SubjectId::new("crate/example");
    let anchor = AnchorId::new("export/example/value");
    let doc_anchor = AnchorId::new("doc/example/value");
    let surface = SurfaceId::new("syntax/example");
    let specimen = SpecimenId::new("recipe/example/value");
    let feature = FeatureId::new("feature/example/value");
    IndexDoc {
        schema: "sim.index".into(),
        generated_by: "vault-test".into(),
        visibility: Visibility::Public,
        subjects: vec![SubjectRecord {
            id: subject.clone(),
            kind: "crate".into(),
            title: "Example".into(),
        }],
        anchors: vec![
            DiscoveredAnchor {
                id: anchor.clone(),
                subject: subject.clone(),
                kind: "export".into(),
            },
            DiscoveredAnchor {
                id: doc_anchor.clone(),
                subject: subject.clone(),
                kind: "doc".into(),
            },
        ],
        source_units: vec![SourceUnit {
            subject: subject.clone(),
            path: "src/lib.rs".into(),
            reachability: SourceReachability::Reachable,
            completeness: SourceCompleteness::Complete,
            reason: String::new(),
            retained_bound: SyntaxBound {
                max_bytes: 4096,
                truncated: false,
            },
            declaration_count: 2,
        }],
        declarations: vec![DeclarationFact {
            anchor: anchor.clone(),
            role: DeclarationRole::Struct,
            module_path: "example::Value".into(),
            generics: String::new(),
            members: vec!["value: String".into()],
            location: SourceLocation {
                file: "src/lib.rs".into(),
                declaration: 0,
            },
            syntax_bound: SyntaxBound {
                max_bytes: 4096,
                truncated: false,
            },
        }],
        protocol_relations: vec![ProtocolRelation {
            anchor: anchor.clone(),
            implementor: "Value".into(),
            source_spelling: "Display".into(),
            body_fingerprint: "fmt".into(),
            body_bound: SyntaxBound {
                max_bytes: 4096,
                truncated: false,
            },
            resolution: ProtocolResolution::Resolved {
                protocol: "core::fmt::Display".into(),
            },
        }],
        surfaces: vec![DiscoveredSurface {
            id: surface.clone(),
            subject: subject.clone(),
            kind: "syntax".into(),
        }],
        specimens: vec![DiscoveredSpecimen {
            id: specimen.clone(),
            subject: subject.clone(),
            kind: "recipe".into(),
            path: "recipes/value".into(),
            language: Some("sim".into()),
            runnable: true,
            checked: true,
            checked_by: Some("test".into()),
            doc_anchor: Some(doc_anchor.clone()),
        }],
        drafts: vec![FeatureDraft {
            id: FeatureId::new("feature/example/draft"),
            subject: subject.clone(),
            title: "Draft".into(),
            summary: "Draft feature".into(),
            claims_anchors: vec![],
            claims_surfaces: vec![],
            claims_specimens: vec![],
            literal_anchors: vec![],
            literal_surfaces: vec![],
            literal_specimens: vec![],
            grammar_contracts: vec![],
            doc_anchor: None,
        }],
        features: vec![FeatureRecord {
            id: feature.clone(),
            key: canonical_feature_key(&subject, feature.as_str()),
            subject: subject.clone(),
            title: "Value".into(),
            summary: "Example value".into(),
            anchors: vec![anchor.clone()],
            surfaces: vec![surface.clone()],
            specimens: vec![specimen.clone()],
            grammar_contracts: vec![GrammarContract {
                id: "grammar/example".into(),
                decoder: Some(anchor),
                encoder: None,
                surface: Some(surface),
                round_trip: true,
            }],
            doc_anchor: Some(doc_anchor),
        }],
        routes: vec![RouteRecord {
            id: RouteId::new("route/example/value"),
            title: "Use value".into(),
            audiences: vec!["user".into()],
            steps: vec![
                RouteStep::Feature {
                    id: feature.clone(),
                    why: "Learn it".into(),
                },
                RouteStep::Specimen {
                    id: specimen,
                    why: "Run it".into(),
                },
            ],
            doc_anchor: None,
        }],
        edges: vec![IndexEdge::relates(feature.clone(), "supports", feature)],
    }
}

fn site() -> ClaimSite {
    ClaimSite {
        note: VaultNoteId::new("note/test"),
        section: "rows".into(),
    }
}

#[test]
fn every_inventory_family_closes_and_permutations_are_identical() {
    let doc = fixture();
    let projection = VaultProjection::from_complete(&doc, VaultGranularity::Full).unwrap();
    assert!(projection.certificate().is_closed());
    assert_eq!(
        projection.certificate().primary().len(),
        doc.inventory().1.len()
    );
    let mut reordered = doc.clone();
    reordered.anchors.reverse();
    assert_eq!(
        projection,
        VaultProjection::from_complete(&reordered, VaultGranularity::Full).unwrap()
    );
    assert!(matches!(
        doc.protocol_relations[0].resolution,
        ProtocolResolution::Resolved { .. }
    ));
}

#[test]
fn exact_certificate_distinguishes_all_claim_failures_and_substitution() {
    let a = IndexRow::Subject(fixture().subjects[0].clone());
    let mut other = fixture().subjects[0].clone();
    other.id = SubjectId::new("crate/other");
    let b = IndexRow::Subject(other);
    assert!(matches!(
        ClaimCertificate::close([a.clone(), a.clone()], [], vec![]),
        Err(ProjectionError::DuplicateCanonicalRow(_))
    ));
    assert!(matches!(
        ClaimCertificate::close([a.clone()], [(b.clone(), site())], vec![]),
        Err(ProjectionError::UnknownClaimedRow(_))
    ));
    assert!(matches!(
        ClaimCertificate::close([a.clone()], [], vec![]),
        Err(ProjectionError::UnclaimedRow(_))
    ));
    assert!(matches!(
        ClaimCertificate::close(
            [a.clone()],
            [(a.clone(), site()), (a.clone(), site())],
            vec![]
        ),
        Err(ProjectionError::MultiplyClaimedRow(_))
    ));
    let derived = DerivedClaim {
        row: b.clone(),
        site: site(),
        origin: "test".into(),
    };
    assert!(matches!(
        ClaimCertificate::close([a.clone()], [(a, site())], vec![derived]),
        Err(ProjectionError::DerivedWithoutPrimary(_))
    ));
    // A same-sized substitution is rejected by row identity, never hidden by totals.
    assert!(matches!(
        ClaimCertificate::close([b.clone()], [(b.clone(), site()), (b, site())], vec![]),
        Err(ProjectionError::MultiplyClaimedRow(_))
    ));
}

#[test]
fn private_documents_are_rejected_before_planning() {
    let mut doc = fixture();
    doc.visibility = Visibility::PrivateLocal;
    assert_eq!(
        VaultProjection::from_complete(&doc, VaultGranularity::Full),
        Err(ProjectionError::NonPublicDocument)
    );
}

#[test]
fn compact_and_full_have_identical_rows_and_only_anchor_placement_differs() {
    let doc = fixture();
    let compact = VaultProjection::from_complete(&doc, VaultGranularity::Compact).unwrap();
    let full = VaultProjection::from_complete(&doc, VaultGranularity::Full).unwrap();
    assert_eq!(
        compact.certificate().primary_rows(),
        full.certificate().primary_rows()
    );
    for row in compact.certificate().primary_rows() {
        let compact_site = &compact.certificate().primary()[row];
        let full_site = &full.certificate().primary()[row];
        match row {
            IndexRow::Anchor(_) | IndexRow::Declaration(_) | IndexRow::ProtocolRelation(_) => {
                assert_ne!(compact_site.note, full_site.note)
            }
            _ => assert_eq!(compact_site, full_site),
        }
    }
    assert!(
        !compact
            .notes()
            .iter()
            .any(|note| note.kind == VaultNoteKind::Anchor)
    );
    assert!(
        full.notes()
            .iter()
            .any(|note| note.kind == VaultNoteKind::Anchor)
    );
}

#[test]
fn route_order_and_exact_source_and_grammar_fields_survive_normalization() {
    let doc = fixture();
    let projection = VaultProjection::from_complete(&doc, VaultGranularity::Full).unwrap();
    let route = projection
        .certificate()
        .primary_rows()
        .iter()
        .find_map(|row| match row {
            IndexRow::Route(route) => Some(route),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        (&route.steps[0], &route.steps[1]),
        (RouteStep::Feature { .. }, RouteStep::Specimen { .. })
    ));
    let source = projection
        .certificate()
        .primary_rows()
        .iter()
        .find_map(|row| match row {
            IndexRow::SourceUnit(unit) => Some(unit),
            _ => None,
        })
        .unwrap();
    assert_eq!(source.retained_bound, doc.source_units[0].retained_bound);
    let feature = projection
        .certificate()
        .primary_rows()
        .iter()
        .find_map(|row| match row {
            IndexRow::Feature(feature) => Some(feature),
            _ => None,
        })
        .unwrap();
    assert_eq!(feature.grammar_contracts, doc.features[0].grammar_contracts);
}

#[test]
fn incoming_relations_are_derived_and_cannot_be_forged_or_omitted() {
    let projection = VaultProjection::from_complete(&fixture(), VaultGranularity::Full).unwrap();
    assert!(
        projection
            .reverse_relations()
            .iter()
            .all(|relation| relation.origin == RelationOrigin::Derived)
    );
    validate_incoming_relations(projection.relations(), projection.reverse_relations()).unwrap();
    let forged = Relation {
        from: "feature/other".into(),
        rel: "supports".into(),
        to: "feature/missing".into(),
        origin: RelationOrigin::Derived,
    };
    assert!(matches!(
        validate_incoming_relations(projection.relations(), &[forged]),
        Err(ProjectionError::ReverseWithoutForward(_))
    ));
    assert!(matches!(
        validate_incoming_relations(projection.relations(), &[]),
        Err(ProjectionError::MissingDerivedRelation(_))
    ));
    let mut dangling = fixture();
    dangling.edges[0].to = "feature/external".into();
    assert!(matches!(
        VaultProjection::from_complete(&dangling, VaultGranularity::Full),
        Err(ProjectionError::IncompleteDocument(_))
    ));
}

#[test]
fn fragments_close_local_rows_and_expose_external_boundaries() {
    let mut doc = fixture();
    doc.edges[0].to = "feature/external/deep/case-sensitive".into();
    let fragment = VaultProjection::project_fragment(&doc).unwrap();
    assert!(fragment.certificate().local_claims().is_closed());
    assert!(!fragment.certificate().is_whole_graph_complete());
    assert_eq!(fragment.certificate().metadata().generated_by, "vault-test");
    assert_eq!(
        fragment.certificate().deferred_external_endpoints()[0].external_to,
        "feature/external/deep/case-sensitive"
    );
    let mut missing_local = doc;
    missing_local.edges[0].from = "feature/missing-local".into();
    assert!(matches!(
        VaultProjection::project_fragment(&missing_local),
        Err(ProjectionError::InvalidFragment(_))
    ));
}

#[test]
fn unsafe_source_paths_are_rejected_before_placement() {
    for path in [
        "../secret",
        "src\\lib.rs",
        "C:/absolute.rs",
        "/absolute.rs",
        "src//lib.rs",
        "src/./lib.rs",
        "src/\u{7f}lib.rs",
    ] {
        let mut doc = fixture();
        doc.source_units[0].path = path.into();
        assert_eq!(
            VaultProjection::from_complete(&doc, VaultGranularity::Full),
            Err(ProjectionError::InvalidSourcePath(path.into()))
        );
    }
}

#[test]
fn optional_and_protocol_and_truncation_states_remain_exact() {
    let mut doc = fixture();
    doc.specimens[0].language = None;
    doc.specimens[0].checked_by = None;
    doc.declarations[0].syntax_bound.truncated = true;
    doc.declarations[0].syntax_bound.max_bytes = 8;
    doc.declarations[0].generics.clear();
    doc.declarations[0].members.clear();
    doc.protocol_relations[0].resolution = ProtocolResolution::Unresolved {
        candidates: vec![],
        reason: UnresolvedReason::ExternalMetadataAbsent,
    };
    let projection = VaultProjection::from_complete(&doc, VaultGranularity::Full).unwrap();
    assert_eq!(
        projection.certificate().primary_rows(),
        &doc.normalized_inventory().into_iter().collect()
    );
}
