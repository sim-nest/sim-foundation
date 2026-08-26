use sim_kernel::{Value, testing::bare_cx};

use crate::test_support::{assert_card_entry, card};
use crate::{
    AnchorId, DeclarationFact, DeclarationRole, DiscoveredAnchor, DiscoveredSpecimen,
    DiscoveredSurface, FeatureDraft, FeatureId, FeatureRecord, GrammarContract, HostSourceRole,
    IndexDoc, IndexEdge, IndexError, ProtocolRelation, ProtocolResolution, RouteId, RouteRecord,
    RouteStep, SourceCompleteness, SourceLocation, SourceReachability, SourceUnit, SpecimenId,
    SubjectId, SubjectRecord, SurfaceId, SyntaxBound, UnresolvedReason, Visibility,
    check_index_doc, check_index_fragment, declaration_card, draft::materialize_draft,
    feature_card, key::canonical_feature_key, protocol_relation_card, route_card, specimen_card,
};

#[test]
fn inventory_is_exhaustive_ordered_and_borrowed() {
    let mut doc = valid_doc();
    doc.source_units
        .push(source_unit(SourceCompleteness::Complete));
    doc.declarations.push(declaration("inventory"));
    doc.protocol_relations
        .push(protocol(ProtocolResolution::Resolved {
            protocol: "sim_index_core::Inventory".to_owned(),
        }));
    doc.drafts.push(FeatureDraft {
        id: FeatureId::new("feature/sim-run/inventory-draft"),
        subject: SubjectId::new("crate/sim-run"),
        title: "Inventory draft".to_owned(),
        summary: "Exercises the authored row family.".to_owned(),
        claims_anchors: Vec::new(),
        claims_surfaces: Vec::new(),
        claims_specimens: Vec::new(),
        literal_anchors: Vec::new(),
        literal_surfaces: Vec::new(),
        literal_specimens: Vec::new(),
        grammar_contracts: Vec::new(),
        doc_anchor: None,
    });

    let (metadata, rows) = doc.inventory();
    assert_eq!(metadata.schema, doc.schema);
    assert_eq!(metadata.generated_by, doc.generated_by);
    assert_eq!(metadata.visibility, doc.visibility);
    assert_eq!(rows.len(), 14);
    assert_eq!(
        rows.iter().map(|row| row.family()).collect::<Vec<_>>(),
        [
            crate::IndexRowFamily::Subject,
            crate::IndexRowFamily::Subject,
            crate::IndexRowFamily::Anchor,
            crate::IndexRowFamily::Anchor,
            crate::IndexRowFamily::SourceUnit,
            crate::IndexRowFamily::Declaration,
            crate::IndexRowFamily::ProtocolRelation,
            crate::IndexRowFamily::Surface,
            crate::IndexRowFamily::Specimen,
            crate::IndexRowFamily::Draft,
            crate::IndexRowFamily::Feature,
            crate::IndexRowFamily::Route,
            crate::IndexRowFamily::Edge,
            crate::IndexRowFamily::Edge,
        ]
    );
    assert!(
        matches!(rows[0], crate::IndexRowRef::Subject(row) if std::ptr::eq(row, &doc.subjects[0]))
    );
    assert!(
        matches!(rows[5], crate::IndexRowRef::Declaration(row) if std::ptr::eq(row, &doc.declarations[0]))
    );
    let owned: Vec<_> = rows.into_iter().map(crate::IndexRowRef::to_owned).collect();
    let mut normalized = owned.clone();
    normalized.sort();
    assert_eq!(doc.normalized_inventory(), normalized);
    assert_eq!(owned[5].diagnostic_key(), &owned[5]);
}

#[test]
fn inventory_has_one_top_level_ownership_destructure() {
    let source = include_str!("rows.rs");
    assert_eq!(source.matches("let Self {").count(), 1);
    assert!(!source.contains("let Self { .."));
}

#[test]
fn exact_non_id_duplicates_are_distinct_from_duplicate_ids() {
    let mut doc = valid_doc();
    doc.edges.push(doc.edges[0].clone());
    assert!(matches!(
        check_index_doc(&doc),
        Err(IndexError::DuplicateExactRow {
            family: crate::IndexRowFamily::Edge,
            ..
        })
    ));
}

#[test]
fn host_source_roles_are_permanent_and_closed() {
    assert_eq!(
        [
            HostSourceRole::Pure,
            HostSourceRole::Capsule,
            HostSourceRole::Bootstrap,
            HostSourceRole::Tool,
            HostSourceRole::Test,
        ]
        .map(HostSourceRole::as_str),
        ["pure", "capsule", "bootstrap", "tool", "test"]
    );
}

fn subject() -> SubjectRecord {
    SubjectRecord {
        id: SubjectId::new("crate/sim-run"),
        kind: "crate".to_owned(),
        title: "sim-run".to_owned(),
    }
}

fn repo_subject() -> SubjectRecord {
    SubjectRecord {
        id: SubjectId::new("repo/sim-run"),
        kind: "repo".to_owned(),
        title: "sim-run".to_owned(),
    }
}

fn anchor(id: &str) -> DiscoveredAnchor {
    DiscoveredAnchor {
        id: AnchorId::new(id),
        subject: SubjectId::new("crate/sim-run"),
        kind: "export".to_owned(),
    }
}

fn surface() -> DiscoveredSurface {
    DiscoveredSurface {
        id: SurfaceId::new("cli/repl"),
        subject: SubjectId::new("crate/sim-run"),
        kind: "cli".to_owned(),
    }
}

fn specimen(id: &str, runnable: bool, checked: bool) -> DiscoveredSpecimen {
    DiscoveredSpecimen {
        id: SpecimenId::new(id),
        subject: SubjectId::new("crate/sim-run"),
        kind: "recipe".to_owned(),
        path: "recipes/01-basics/repl/recipe.toml".to_owned(),
        language: Some("cli-transcript".to_owned()),
        runnable,
        checked,
        checked_by: Some("xtask check-recipes".to_owned()),
        doc_anchor: Some(AnchorId::new("doc/sim-run/repl")),
    }
}

fn feature() -> FeatureRecord {
    let subject = SubjectId::new("crate/sim-run");
    let id = FeatureId::new("feature/sim-run/repl");
    FeatureRecord {
        key: canonical_feature_key(&subject, id.as_str()),
        id,
        subject,
        title: "REPL".to_owned(),
        summary: "Interactive command loop for SIM sessions.".to_owned(),
        anchors: vec![AnchorId::new("export/sim-run/repl")],
        surfaces: vec![SurfaceId::new("cli/repl")],
        specimens: vec![SpecimenId::new("recipe/sim-run/repl")],
        grammar_contracts: vec![GrammarContract {
            id: "grammar/repl".to_owned(),
            decoder: Some(AnchorId::new("export/sim-run/repl")),
            encoder: None,
            surface: Some(SurfaceId::new("cli/repl")),
            round_trip: true,
        }],
        doc_anchor: Some(AnchorId::new("doc/sim-run/repl")),
    }
}

fn route() -> RouteRecord {
    RouteRecord {
        id: RouteId::new("route/use-repl"),
        title: "Use the REPL".to_owned(),
        audiences: vec!["user".to_owned(), "code".to_owned()],
        steps: vec![
            RouteStep::Feature {
                id: FeatureId::new("feature/sim-run/repl"),
                why: "The REPL is the interactive entry point.".to_owned(),
            },
            RouteStep::Specimen {
                id: SpecimenId::new("recipe/sim-run/repl"),
                why: "Run the REPL recipe.".to_owned(),
            },
        ],
        doc_anchor: Some(AnchorId::new("doc/sim-run/repl")),
    }
}

fn valid_doc() -> IndexDoc {
    IndexDoc {
        schema: "sim.index".to_owned(),
        generated_by: "sim-index-core-tests".to_owned(),
        visibility: Visibility::Public,
        subjects: vec![repo_subject(), subject()],
        anchors: vec![anchor("export/sim-run/repl"), anchor("doc/sim-run/repl")],
        source_units: Vec::new(),
        declarations: Vec::new(),
        protocol_relations: Vec::new(),
        surfaces: vec![surface()],
        specimens: vec![specimen("recipe/sim-run/repl", true, true)],
        drafts: Vec::new(),
        features: vec![feature()],
        routes: vec![route()],
        edges: vec![
            IndexEdge::relates(
                FeatureId::new("feature/sim-run/repl"),
                "supports",
                FeatureId::new("feature/sim-run/repl"),
            ),
            IndexEdge::contains(
                SubjectId::new("repo/sim-run"),
                SubjectId::new("crate/sim-run"),
            ),
        ],
    }
}

fn source_unit(completeness: SourceCompleteness) -> SourceUnit {
    SourceUnit {
        subject: SubjectId::new("crate/sim-run"),
        path: "src/lib.rs".to_owned(),
        reachability: SourceReachability::Reachable,
        completeness,
        reason: if completeness == SourceCompleteness::Complete {
            String::new()
        } else {
            "bounded scanner evidence".to_owned()
        },
        retained_bound: SyntaxBound {
            max_bytes: 4096,
            truncated: completeness == SourceCompleteness::Truncated,
        },
        declaration_count: 7,
    }
}

#[test]
fn fragments_retain_incomplete_units_but_strict_graphs_reject_them() {
    for state in [
        SourceCompleteness::Malformed,
        SourceCompleteness::Unreadable,
        SourceCompleteness::Truncated,
        SourceCompleteness::Unsupported,
        SourceCompleteness::Unresolved,
    ] {
        let mut doc = valid_doc();
        doc.source_units.push(source_unit(state));
        check_index_fragment(&doc).expect("fragment retains incomplete evidence");
        assert!(matches!(
            check_index_doc(&doc),
            Err(IndexError::IncompleteReachableSource { state: actual, .. }) if actual == state.as_str()
        ));
    }
}

#[test]
fn source_unit_reasons_and_bounds_are_closed_and_bounded() {
    let mut doc = valid_doc();
    let mut unit = source_unit(SourceCompleteness::Malformed);
    unit.reason = "x".repeat(513);
    doc.source_units.push(unit);
    assert!(matches!(
        check_index_fragment(&doc),
        Err(IndexError::InvalidSourceUnit { .. })
    ));

    doc.source_units[0] = source_unit(SourceCompleteness::Truncated);
    doc.source_units[0].retained_bound.truncated = false;
    assert!(matches!(
        check_index_fragment(&doc),
        Err(IndexError::InvalidSourceUnit { .. })
    ));
}

fn declaration(path: &str) -> DeclarationFact {
    DeclarationFact {
        anchor: AnchorId::new("export/sim-run/repl"),
        role: DeclarationRole::Struct,
        module_path: path.to_owned(),
        generics: "<T>".to_owned(),
        members: vec!["value:T".to_owned()],
        location: SourceLocation {
            file: "src/lib.rs".to_owned(),
            declaration: 0,
        },
        syntax_bound: SyntaxBound {
            max_bytes: 64,
            truncated: false,
        },
    }
}

fn protocol(resolution: ProtocolResolution) -> ProtocolRelation {
    ProtocolRelation {
        anchor: AnchorId::new("export/sim-run/repl"),
        implementor: "Repl".to_owned(),
        source_spelling: "Function".to_owned(),
        body_fingerprint: "call(&self)".to_owned(),
        body_bound: SyntaxBound {
            max_bytes: 64,
            truncated: false,
        },
        resolution,
    }
}

fn check_error(doc: &IndexDoc) -> IndexError {
    check_index_doc(doc).expect_err("document should fail validation")
}

#[test]
fn valid_index_reports_counts() {
    let report = check_index_doc(&valid_doc()).expect("valid index");
    assert_eq!(report.subjects, 2);
    assert_eq!(report.features, 1);
    assert_eq!(report.specimens, 1);
    assert_eq!(report.routes, 1);
}

#[test]
fn fragment_check_defers_cross_repository_relationships_until_merge() {
    let mut doc = valid_doc();
    doc.edges.push(IndexEdge::relates(
        FeatureId::new("feature/sim-run/repl"),
        "presents",
        FeatureId::new("feature/sim-runtime/read-eval"),
    ));
    doc.routes[0].steps.push(RouteStep::Specimen {
        id: SpecimenId::new("spec-test/sim-sdk/tests/read_eval"),
        why: "The SDK proves the public facade.".to_owned(),
    });

    check_index_fragment(&doc).expect("fragment defers external endpoints");
    assert!(matches!(
        check_index_doc(&doc),
        Err(IndexError::UnresolvedClaim { .. })
    ));
}

#[test]
fn old_graphs_and_cards_are_unchanged_without_source_facts() {
    let doc = valid_doc();
    assert!(doc.declarations.is_empty() && doc.protocol_relations.is_empty());
    check_index_doc(&doc).expect("old graph remains valid");
    let mut cx = bare_cx();
    let value = feature_card(&mut cx, &doc.features[0]).expect("feature card");
    assert!(
        !card(&value)
            .entries()
            .iter()
            .any(|(name, _)| name.as_qualified_str() == "source-role")
    );
}

#[test]
fn source_fact_validation_fails_closed() {
    let mut doc = valid_doc();
    doc.declarations = vec![declaration("repl"), declaration("repl")];
    assert!(matches!(
        check_error(&doc),
        IndexError::DuplicateSourceFact { .. }
    ));

    let mut doc = valid_doc();
    let mut fact = declaration("repl");
    fact.anchor = AnchorId::new("anchor/rustdoc/missing");
    doc.declarations.push(fact);
    assert!(
        matches!(check_error(&doc), IndexError::UnresolvedClaim { owner, .. } if owner.starts_with("declaration:"))
    );

    let mut doc = valid_doc();
    let mut fact = declaration("repl");
    fact.syntax_bound.max_bytes = 1;
    doc.declarations.push(fact);
    assert!(matches!(
        check_error(&doc),
        IndexError::InvalidSourceBound { .. }
    ));

    let mut doc = valid_doc();
    doc.declarations = vec![declaration("z"), declaration("a")];
    assert!(matches!(
        check_error(&doc),
        IndexError::UnstableOrdering {
            kind: "declaration"
        }
    ));
}

#[test]
fn protocol_resolution_validation_fails_closed() {
    let resolved = ProtocolResolution::Resolved {
        protocol: "sim_kernel::Function".to_owned(),
    };
    let mut doc = valid_doc();
    doc.protocol_relations = vec![protocol(resolved.clone()), protocol(resolved)];
    assert!(matches!(
        check_error(&doc),
        IndexError::DuplicateProtocolRelation { .. }
    ));

    let mut doc = valid_doc();
    doc.protocol_relations = vec![
        protocol(ProtocolResolution::Resolved {
            protocol: "sim_kernel::Function".to_owned(),
        }),
        protocol(ProtocolResolution::Unresolved {
            reason: UnresolvedReason::ExternalMetadataAbsent,
            candidates: vec![],
        }),
    ];
    assert!(matches!(
        check_error(&doc),
        IndexError::ConflictingProtocolResolution { .. }
    ));

    let mut doc = valid_doc();
    doc.protocol_relations
        .push(protocol(ProtocolResolution::Unresolved {
            reason: UnresolvedReason::AmbiguousName,
            candidates: vec!["z::Function".to_owned(), "a::Function".to_owned()],
        }));
    assert!(matches!(
        check_error(&doc),
        IndexError::InvalidProtocolResolution { .. }
    ));

    let mut doc = valid_doc();
    let mut relation = protocol(ProtocolResolution::Resolved {
        protocol: "sim_kernel::Function".to_owned(),
    });
    relation.body_bound.max_bytes = 1;
    doc.protocol_relations.push(relation);
    assert!(matches!(
        check_error(&doc),
        IndexError::InvalidSourceBound { .. }
    ));
}

#[test]
fn source_cards_publish_compact_roles_without_signatures() {
    let fact = declaration("repl");
    let relation = protocol(ProtocolResolution::Resolved {
        protocol: "sim_kernel::Function".to_owned(),
    });
    let mut cx = bare_cx();
    let fact_value = declaration_card(&mut cx, &fact).expect("declaration card");
    let relation_value = protocol_relation_card(&mut cx, &relation).expect("protocol card");
    assert_card_entry(&fact_value, "source-role", "struct", &mut cx);
    assert_card_entry(&relation_value, "resolution", "resolved", &mut cx);
    assert!(!card(&fact_value).entries().iter().any(|(name, _)| {
        let name = name.as_qualified_str();
        name == "generics" || name == "members"
    }));
}

#[test]
fn duplicate_ids_fail() {
    let mut doc = valid_doc();
    doc.features.push(doc.features[0].clone());

    assert!(matches!(
        check_error(&doc),
        IndexError::DuplicateId {
            kind: "feature",
            id
        } if id == "feature/sim-run/repl"
    ));
}

#[test]
fn malformed_ids_fail() {
    let mut doc = valid_doc();
    doc.features[0].id = FeatureId::new("Feature/Bad");

    assert!(matches!(
        check_error(&doc),
        IndexError::InvalidId {
            kind: "feature",
            id
        } if id == "Feature/Bad"
    ));
}

#[test]
fn authored_literals_fail() {
    let mut doc = valid_doc();
    doc.drafts.push(FeatureDraft {
        id: FeatureId::new("feature/sim-run/literal"),
        subject: SubjectId::new("crate/sim-run"),
        title: "Literal".to_owned(),
        summary: "Invalid authored literal.".to_owned(),
        claims_anchors: Vec::new(),
        claims_surfaces: Vec::new(),
        claims_specimens: Vec::new(),
        literal_anchors: vec!["export/sim-run/literal".to_owned()],
        literal_surfaces: Vec::new(),
        literal_specimens: Vec::new(),
        grammar_contracts: Vec::new(),
        doc_anchor: None,
    });

    assert!(matches!(
        check_error(&doc),
        IndexError::LiteralClaim {
            owner,
            kind: "anchor"
        } if owner == "feature/sim-run/literal"
    ));
}

#[test]
fn unresolved_claims_fail() {
    let mut doc = valid_doc();
    doc.features[0]
        .anchors
        .push(AnchorId::new("export/sim-run/missing"));

    assert!(matches!(
        check_error(&doc),
        IndexError::UnresolvedClaim {
            owner,
            kind: "anchor",
            id
        } if owner == "feature:feature/sim-run/repl" && id == "export/sim-run/missing"
    ));
}

#[test]
fn duplicate_claims_fail() {
    let mut doc = valid_doc();
    doc.features[0]
        .specimens
        .push(SpecimenId::new("recipe/sim-run/repl"));

    assert!(matches!(
        check_error(&doc),
        IndexError::DuplicateClaim {
            owner,
            kind: "specimen",
            id
        } if owner == "feature/sim-run/repl" && id == "recipe/sim-run/repl"
    ));
}

#[test]
fn duplicate_canonical_keys_fail() {
    let mut doc = valid_doc();
    let mut duplicate = doc.features[0].clone();
    duplicate.id = FeatureId::new("feature/sim-run/repl-copy");
    doc.features.push(duplicate);

    assert!(matches!(
        check_error(&doc),
        IndexError::DuplicateCanonicalKey { key } if key == "crate/sim-run/feature-sim-run-repl"
    ));
}

#[test]
fn invalid_grammar_contracts_fail() {
    let mut doc = valid_doc();
    doc.features[0].grammar_contracts[0].round_trip = false;

    assert!(matches!(
        check_error(&doc),
        IndexError::InvalidGrammarContract {
            owner,
            id
        } if owner == "feature/sim-run/repl" && id == "grammar/repl"
    ));
}

#[test]
fn non_runnable_specimen_claims_fail() {
    let mut doc = valid_doc();
    doc.specimens[0].runnable = false;

    assert!(matches!(
        check_error(&doc),
        IndexError::NonRunnableSpecimen {
            owner,
            id
        } if owner == "feature/sim-run/repl" && id == "recipe/sim-run/repl"
    ));
}

#[test]
fn dead_route_steps_fail() {
    let mut doc = valid_doc();
    doc.routes[0].steps.push(RouteStep::Feature {
        id: FeatureId::new("feature/sim-run/missing"),
        why: "Missing feature.".to_owned(),
    });

    assert!(matches!(
        check_error(&doc),
        IndexError::DeadRouteStep {
            route,
            step
        } if route == "route/use-repl" && step == "feature/sim-run/missing"
    ));
}

#[test]
fn dangling_doc_anchors_fail() {
    let mut doc = valid_doc();
    doc.routes[0].doc_anchor = Some(AnchorId::new("doc/sim-run/missing"));

    assert!(matches!(
        check_error(&doc),
        IndexError::DanglingDocAnchor {
            owner,
            id
        } if owner == "route/use-repl" && id == "doc/sim-run/missing"
    ));
}

#[test]
fn feature_drafts_materialize_claim_ids_without_literals() {
    let draft = FeatureDraft {
        id: FeatureId::new("feature/sim-run/repl"),
        subject: SubjectId::new("crate/sim-run"),
        title: "REPL".to_owned(),
        summary: "Interactive command loop for SIM sessions.".to_owned(),
        claims_anchors: vec![AnchorId::new("export/sim-run/repl")],
        claims_surfaces: vec![SurfaceId::new("cli/repl")],
        claims_specimens: vec![SpecimenId::new("recipe/sim-run/repl")],
        literal_anchors: Vec::new(),
        literal_surfaces: Vec::new(),
        literal_specimens: Vec::new(),
        grammar_contracts: Vec::new(),
        doc_anchor: Some(AnchorId::new("doc/sim-run/repl")),
    };

    let feature = materialize_draft(draft);
    assert_eq!(feature.anchors, vec![AnchorId::new("export/sim-run/repl")]);
    assert_eq!(feature.surfaces, vec![SurfaceId::new("cli/repl")]);
    assert_eq!(
        feature.specimens,
        vec![SpecimenId::new("recipe/sim-run/repl")]
    );
    assert_eq!(feature.key.as_str(), "crate/sim-run/feature-sim-run-repl");
}

#[test]
fn feature_specimen_and_route_cards_publish_open_entries() {
    let feature = feature();
    let specimen = specimen("recipe/sim-run/repl", true, true);
    let route = route();
    let mut cx = bare_cx();

    let feature_value = feature_card(&mut cx, &feature).expect("feature card");
    let specimen_value = specimen_card(&mut cx, &specimen).expect("specimen card");
    let route_value = route_card(&mut cx, &route).expect("route card");

    assert_card_entry(&feature_value, "kind", "feature", &mut cx);
    assert_card_entry(
        &feature_value,
        "canonical-key",
        "crate/sim-run/feature-sim-run-repl",
        &mut cx,
    );
    assert_card_entry(&specimen_value, "kind", "specimen", &mut cx);
    assert_card_entry(&specimen_value, "runnable", "true", &mut cx);
    assert_card_entry(&route_value, "kind", "route", &mut cx);
    assert_entry_name(&route_value, "steps");

    let _expr = feature_value
        .object()
        .as_expr(&mut cx)
        .expect("card projects to expr");
}

fn assert_entry_name(value: &Value, name: &str) {
    assert!(
        card(value)
            .entries()
            .iter()
            .any(|(symbol, _)| symbol.as_qualified_str() == name),
        "missing card entry {name}"
    );
}
