use sim_index_core::{
    AnchorId, DeclarationFact, DeclarationRole, DiscoveredAnchor, DiscoveredSpecimen,
    DiscoveredSurface, FeatureDraft, FeatureId, FeatureRecord, GrammarContract, IndexDoc,
    IndexEdge, IndexRow, ProtocolRelation, ProtocolResolution, RouteId, RouteRecord, RouteStep,
    SourceCompleteness, SourceLocation, SourceReachability, SourceUnit, SpecimenId, SubjectId,
    SubjectRecord, SurfaceId, SyntaxBound, Visibility, canonical_feature_key,
};
use sim_index_vault_core::{
    ClaimCertificate, ClaimSite, DerivedClaim, ProjectionError, VaultGranularity, VaultNoteId,
    VaultProjection,
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
