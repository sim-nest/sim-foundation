use crate::{ClaimSite, DerivedClaim, Relation, VaultGranularity, VaultNoteId, VaultNoteKind};
use sim_index_core::{IndexRow, IndexRowRef, RouteStep};

pub(crate) fn primary_site(
    row: IndexRowRef<'_>,
    granularity: VaultGranularity,
) -> (VaultNoteId, VaultNoteKind, &'static str) {
    match row {
        IndexRowRef::Subject(r) => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Subject,
            "subject",
        ),
        IndexRowRef::Anchor(r) if granularity == VaultGranularity::Full => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Anchor,
            "anchor",
        ),
        IndexRowRef::Anchor(r) => (
            VaultNoteId::new(r.subject.to_string()),
            VaultNoteKind::Subject,
            "anchors",
        ),
        IndexRowRef::SourceUnit(r) => (
            VaultNoteId::new(r.subject.to_string()),
            VaultNoteKind::Subject,
            "source-units",
        ),
        IndexRowRef::Declaration(r) => (
            VaultNoteId::new(r.anchor.to_string()),
            VaultNoteKind::Anchor,
            "declarations",
        ),
        IndexRowRef::ProtocolRelation(r) => (
            VaultNoteId::new(r.anchor.to_string()),
            VaultNoteKind::Anchor,
            "protocols",
        ),
        IndexRowRef::Surface(r) => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Surface,
            "surface",
        ),
        IndexRowRef::Specimen(r) => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Specimen,
            "specimen",
        ),
        IndexRowRef::Draft(r) => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Draft,
            "draft",
        ),
        IndexRowRef::Feature(r) => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Feature,
            "feature",
        ),
        IndexRowRef::Route(r) => (
            VaultNoteId::new(r.id.to_string()),
            VaultNoteKind::Route,
            "route",
        ),
        IndexRowRef::Edge(_) => (
            VaultNoteId::new("index/relations"),
            VaultNoteKind::Index,
            "edges",
        ),
    }
}

pub(crate) fn derive(
    row: IndexRowRef<'_>,
    owned: &IndexRow,
    primary_note: &VaultNoteId,
    claims: &mut Vec<DerivedClaim>,
    relations: &mut Vec<Relation>,
) {
    let mut add = |note: String, section: &str, origin: &str| {
        if note != primary_note.as_str() {
            claims.push(DerivedClaim {
                row: owned.clone(),
                site: ClaimSite {
                    note: VaultNoteId::new(note),
                    section: section.to_owned(),
                },
                origin: origin.to_owned(),
            });
        }
    };
    match row {
        IndexRowRef::Anchor(r) => add(r.id.to_string(), "owner", "embedded-owner-summary"),
        IndexRowRef::Declaration(r) => add(r.anchor.to_string(), "backlinks", "anchor-backlink"),
        IndexRowRef::ProtocolRelation(r) => {
            add(r.anchor.to_string(), "backlinks", "anchor-backlink")
        }
        IndexRowRef::Specimen(r) => {
            if let Some(a) = &r.doc_anchor {
                add(a.to_string(), "navigation", "readme-navigation");
            }
        }
        IndexRowRef::Draft(r) => {
            for a in &r.claims_anchors {
                add(a.to_string(), "claimed-by", "feature-claim");
            }
        }
        IndexRowRef::Feature(r) => {
            for a in &r.anchors {
                add(a.to_string(), "claimed-by", "feature-claim");
            }
        }
        IndexRowRef::Route(r) => {
            for step in &r.steps {
                let target = match step {
                    RouteStep::Feature { id, .. } => id.to_string(),
                    RouteStep::Specimen { id, .. } => id.to_string(),
                };
                add(target, "routes", "route-step");
            }
        }
        IndexRowRef::Edge(r) => relations.push(Relation::canonical(&r.from, &r.rel, &r.to)),
        _ => {}
    }
}
