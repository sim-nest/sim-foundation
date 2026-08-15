//! Kernel `Card` projections for index rows.

use std::sync::Arc;

use sim_kernel::{Cx, Ref, Result, Symbol, Value, card::Card};

use crate::{
    DeclarationFact, DiscoveredSpecimen, FeatureRecord, ProtocolRelation, ProtocolResolution,
    RouteRecord, RouteStep,
};

/// Projects compact declaration-role evidence without copying source signatures.
pub fn declaration_card(cx: &mut Cx, declaration: &DeclarationFact) -> Result<Value> {
    let entries = vec![
        ("anchor", text(cx, declaration.anchor.as_str())?),
        ("kind", symbol(cx, "declaration")?),
        ("source-role", symbol(cx, declaration.role.as_str())?),
        ("module-path", text(cx, &declaration.module_path)?),
    ];
    card_value(cx, "declaration", declaration.anchor.as_str(), entries)
}

/// Projects compact protocol-role and resolution evidence.
pub fn protocol_relation_card(cx: &mut Cx, relation: &ProtocolRelation) -> Result<Value> {
    let (state, protocol) = match &relation.resolution {
        ProtocolResolution::Resolved { protocol } => ("resolved", Some(protocol.as_str())),
        ProtocolResolution::Unresolved { .. } => ("unresolved", None),
    };
    let mut entries = vec![
        ("anchor", text(cx, relation.anchor.as_str())?),
        ("kind", symbol(cx, "protocol-relation")?),
        ("source-role", symbol(cx, "implementor")?),
        ("resolution", symbol(cx, state)?),
    ];
    if let Some(protocol) = protocol {
        entries.push(("protocol", text(cx, protocol)?));
    }
    card_value(cx, "protocol-relation", relation.anchor.as_str(), entries)
}

/// Projects a feature row into an ordinary kernel `Card`.
pub fn feature_card(cx: &mut Cx, feature: &FeatureRecord) -> Result<Value> {
    let entries = vec![
        ("id", text(cx, feature.id.as_str())?),
        ("kind", symbol(cx, "feature")?),
        ("canonical-key", text(cx, feature.key.as_str())?),
        ("subject", text(cx, feature.subject.as_str())?),
        ("title", text(cx, &feature.title)?),
        ("summary", text(cx, &feature.summary)?),
        (
            "anchors",
            list_text(cx, feature.anchors.iter().map(|id| id.as_str()))?,
        ),
        (
            "surfaces",
            list_text(cx, feature.surfaces.iter().map(|id| id.as_str()))?,
        ),
        (
            "specimens",
            list_text(cx, feature.specimens.iter().map(|id| id.as_str()))?,
        ),
    ];
    card_value(cx, "feature", feature.id.as_str(), entries)
}

/// Projects a specimen row into an ordinary kernel `Card`.
pub fn specimen_card(cx: &mut Cx, specimen: &DiscoveredSpecimen) -> Result<Value> {
    let mut entries = vec![
        ("id", text(cx, specimen.id.as_str())?),
        ("kind", symbol(cx, "specimen")?),
        ("subject", text(cx, specimen.subject.as_str())?),
        ("specimen-kind", text(cx, &specimen.kind)?),
        ("path", text(cx, &specimen.path)?),
        ("runnable", cx.factory().bool(specimen.runnable)?),
        ("checked", cx.factory().bool(specimen.checked)?),
    ];
    if let Some(language) = &specimen.language {
        entries.push(("language", text(cx, language)?));
    }
    if let Some(checked_by) = &specimen.checked_by {
        entries.push(("checked-by", text(cx, checked_by)?));
    }
    card_value(cx, "specimen", specimen.id.as_str(), entries)
}

/// Projects a route row into an ordinary kernel `Card`.
pub fn route_card(cx: &mut Cx, route: &RouteRecord) -> Result<Value> {
    let entries = vec![
        ("id", text(cx, route.id.as_str())?),
        ("kind", symbol(cx, "route")?),
        ("title", text(cx, &route.title)?),
        ("steps", list_text(cx, route.steps.iter().map(step_label))?),
    ];
    card_value(cx, "route", route.id.as_str(), entries)
}

fn card_value(cx: &mut Cx, kind: &str, id: &str, entries: Vec<(&str, Value)>) -> Result<Value> {
    let subject = Ref::Symbol(Symbol::new(format!("index/{kind}/{id}")));
    let entries = entries
        .into_iter()
        .map(|(name, value)| (Symbol::new(name), value))
        .collect();
    cx.factory().opaque(Arc::new(Card::new(subject, entries)))
}

fn text(cx: &mut Cx, value: &str) -> Result<Value> {
    cx.factory().string(value.to_owned())
}

fn symbol(cx: &mut Cx, value: &str) -> Result<Value> {
    cx.factory().symbol(Symbol::new(value))
}

fn list_text<'a>(cx: &mut Cx, values: impl Iterator<Item = &'a str>) -> Result<Value> {
    let values = values
        .map(|value| text(cx, value))
        .collect::<Result<Vec<_>>>()?;
    cx.factory().list(values)
}

fn step_label(step: &RouteStep) -> &str {
    step.id()
}
