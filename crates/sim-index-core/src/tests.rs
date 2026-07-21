use sim_kernel::{Value, card::Card, testing::bare_cx};

use crate::{
    AnchorId, DiscoveredAnchor, DiscoveredSpecimen, DiscoveredSurface, FeatureDraft, FeatureId,
    FeatureRecord, GrammarContract, IndexDoc, IndexEdge, IndexError, RouteId, RouteRecord,
    RouteStep, SpecimenId, SubjectId, SubjectRecord, SurfaceId, Visibility, check_index_doc,
    draft::materialize_draft, feature_card, key::canonical_feature_key, route_card, specimen_card,
};

fn subject() -> SubjectRecord {
    SubjectRecord {
        id: SubjectId::new("crate/sim-run"),
        kind: "crate".to_owned(),
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
        runnable,
        checked,
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
        steps: vec![
            RouteStep::Feature(FeatureId::new("feature/sim-run/repl")),
            RouteStep::Specimen(SpecimenId::new("recipe/sim-run/repl")),
        ],
        doc_anchor: Some(AnchorId::new("doc/sim-run/repl")),
    }
}

fn valid_doc() -> IndexDoc {
    IndexDoc {
        schema: "sim.index".to_owned(),
        generated_by: "sim-index-core-tests".to_owned(),
        visibility: Visibility::Public,
        subjects: vec![subject()],
        anchors: vec![anchor("export/sim-run/repl"), anchor("doc/sim-run/repl")],
        surfaces: vec![surface()],
        specimens: vec![specimen("recipe/sim-run/repl", true, true)],
        drafts: Vec::new(),
        features: vec![feature()],
        routes: vec![route()],
        edges: vec![IndexEdge {
            from: FeatureId::new("feature/sim-run/repl"),
            predicate: "supports".to_owned(),
            to: FeatureId::new("feature/sim-run/repl"),
        }],
    }
}

fn check_error(doc: &IndexDoc) -> IndexError {
    check_index_doc(doc).expect_err("document should fail validation")
}

#[test]
fn valid_index_reports_counts() {
    let report = check_index_doc(&valid_doc()).expect("valid index");
    assert_eq!(report.subjects, 1);
    assert_eq!(report.features, 1);
    assert_eq!(report.specimens, 1);
    assert_eq!(report.routes, 1);
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
    doc.routes[0].steps.push(RouteStep::Feature(FeatureId::new(
        "feature/sim-run/missing",
    )));

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

fn card(value: &Value) -> &Card {
    value.object().downcast_ref::<Card>().expect("index card")
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

fn assert_card_entry(value: &Value, name: &str, expected: &str, cx: &mut sim_kernel::Cx) {
    let (_, value) = card(value)
        .entries()
        .iter()
        .find(|(symbol, _)| symbol.as_qualified_str() == name)
        .unwrap_or_else(|| panic!("missing card entry {name}"));
    assert_eq!(value.object().display(cx).expect("entry display"), expected);
}
