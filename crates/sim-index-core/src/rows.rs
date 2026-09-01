//! Exhaustive traversal of the canonical rows in an [`IndexDoc`].

use crate::{
    DeclarationFact, DiscoveredAnchor, DiscoveredSpecimen, DiscoveredSurface, FeatureDraft,
    FeatureRecord, IndexDoc, IndexEdge, ProtocolRelation, RouteRecord, SourceUnit, SubjectRecord,
    Visibility,
};

/// Borrowed top-level document metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexMetadataRef<'a> {
    /// Schema marker.
    pub schema: &'a str,
    /// Generator identity.
    pub generated_by: &'a str,
    /// Document visibility.
    pub visibility: Visibility,
}

/// Canonical row families, in inventory emission order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum IndexRowFamily {
    /// Subject records.
    Subject,
    /// Anchor records.
    Anchor,
    /// Source-unit records.
    SourceUnit,
    /// Declaration records.
    Declaration,
    /// Protocol-relation records.
    ProtocolRelation,
    /// Surface records.
    Surface,
    /// Specimen records.
    Specimen,
    /// Authored feature drafts.
    Draft,
    /// Materialized feature records.
    Feature,
    /// Route records.
    Route,
    /// Graph edges.
    Edge,
}

/// One borrowed canonical index row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexRowRef<'a> {
    /// Subject row.
    Subject(&'a SubjectRecord),
    /// Anchor row.
    Anchor(&'a DiscoveredAnchor),
    /// Source-unit row.
    SourceUnit(&'a SourceUnit),
    /// Declaration row.
    Declaration(&'a DeclarationFact),
    /// Protocol-relation row.
    ProtocolRelation(&'a ProtocolRelation),
    /// Surface row.
    Surface(&'a DiscoveredSurface),
    /// Specimen row.
    Specimen(&'a DiscoveredSpecimen),
    /// Draft row.
    Draft(&'a FeatureDraft),
    /// Feature row.
    Feature(&'a FeatureRecord),
    /// Route row.
    Route(&'a RouteRecord),
    /// Edge row.
    Edge(&'a IndexEdge),
}

/// One owned canonical index row.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum IndexRow {
    /// Subject row.
    Subject(SubjectRecord),
    /// Anchor row.
    Anchor(DiscoveredAnchor),
    /// Source-unit row.
    SourceUnit(SourceUnit),
    /// Declaration row.
    Declaration(DeclarationFact),
    /// Protocol-relation row.
    ProtocolRelation(ProtocolRelation),
    /// Surface row.
    Surface(DiscoveredSurface),
    /// Specimen row.
    Specimen(DiscoveredSpecimen),
    /// Draft row.
    Draft(FeatureDraft),
    /// Feature row.
    Feature(FeatureRecord),
    /// Route row.
    Route(RouteRecord),
    /// Edge row.
    Edge(IndexEdge),
}

impl IndexRowRef<'_> {
    /// Returns this row's exact family.
    pub const fn family(self) -> IndexRowFamily {
        match self {
            Self::Subject(_) => IndexRowFamily::Subject,
            Self::Anchor(_) => IndexRowFamily::Anchor,
            Self::SourceUnit(_) => IndexRowFamily::SourceUnit,
            Self::Declaration(_) => IndexRowFamily::Declaration,
            Self::ProtocolRelation(_) => IndexRowFamily::ProtocolRelation,
            Self::Surface(_) => IndexRowFamily::Surface,
            Self::Specimen(_) => IndexRowFamily::Specimen,
            Self::Draft(_) => IndexRowFamily::Draft,
            Self::Feature(_) => IndexRowFamily::Feature,
            Self::Route(_) => IndexRowFamily::Route,
            Self::Edge(_) => IndexRowFamily::Edge,
        }
    }

    /// Clones the canonical record into its owned row form.
    pub fn to_owned(self) -> IndexRow {
        match self {
            Self::Subject(row) => IndexRow::Subject(row.clone()),
            Self::Anchor(row) => IndexRow::Anchor(row.clone()),
            Self::SourceUnit(row) => IndexRow::SourceUnit(row.clone()),
            Self::Declaration(row) => IndexRow::Declaration(row.clone()),
            Self::ProtocolRelation(row) => IndexRow::ProtocolRelation(row.clone()),
            Self::Surface(row) => IndexRow::Surface(row.clone()),
            Self::Specimen(row) => IndexRow::Specimen(row.clone()),
            Self::Draft(row) => IndexRow::Draft(row.clone()),
            Self::Feature(row) => IndexRow::Feature(row.clone()),
            Self::Route(row) => IndexRow::Route(row.clone()),
            Self::Edge(row) => IndexRow::Edge(row.clone()),
        }
    }

    /// Returns a stable, complete diagnostic representation of the canonical record.
    pub fn diagnostic_key(self) -> IndexRow {
        self.to_owned()
    }
}

impl IndexRow {
    /// Returns this row's exact family.
    pub const fn family(&self) -> IndexRowFamily {
        match self {
            Self::Subject(_) => IndexRowFamily::Subject,
            Self::Anchor(_) => IndexRowFamily::Anchor,
            Self::SourceUnit(_) => IndexRowFamily::SourceUnit,
            Self::Declaration(_) => IndexRowFamily::Declaration,
            Self::ProtocolRelation(_) => IndexRowFamily::ProtocolRelation,
            Self::Surface(_) => IndexRowFamily::Surface,
            Self::Specimen(_) => IndexRowFamily::Specimen,
            Self::Draft(_) => IndexRowFamily::Draft,
            Self::Feature(_) => IndexRowFamily::Feature,
            Self::Route(_) => IndexRowFamily::Route,
            Self::Edge(_) => IndexRowFamily::Edge,
        }
    }

    /// Returns a stable diagnostic key containing the complete canonical record.
    pub const fn diagnostic_key(&self) -> &Self {
        self
    }
}

impl IndexDoc {
    /// Borrows metadata and every row in canonical family and source order.
    pub fn inventory(&self) -> (IndexMetadataRef<'_>, Vec<IndexRowRef<'_>>) {
        let Self {
            schema,
            generated_by,
            visibility,
            subjects,
            anchors,
            source_units,
            declarations,
            protocol_relations,
            surfaces,
            specimens,
            drafts,
            features,
            routes,
            edges,
        } = self;
        let metadata = IndexMetadataRef {
            schema,
            generated_by,
            visibility: *visibility,
        };
        let mut rows = Vec::with_capacity(
            subjects.len()
                + anchors.len()
                + source_units.len()
                + declarations.len()
                + protocol_relations.len()
                + surfaces.len()
                + specimens.len()
                + drafts.len()
                + features.len()
                + routes.len()
                + edges.len(),
        );
        rows.extend(subjects.iter().map(IndexRowRef::Subject));
        rows.extend(anchors.iter().map(IndexRowRef::Anchor));
        rows.extend(source_units.iter().map(IndexRowRef::SourceUnit));
        rows.extend(declarations.iter().map(IndexRowRef::Declaration));
        rows.extend(protocol_relations.iter().map(IndexRowRef::ProtocolRelation));
        rows.extend(surfaces.iter().map(IndexRowRef::Surface));
        rows.extend(specimens.iter().map(IndexRowRef::Specimen));
        rows.extend(drafts.iter().map(IndexRowRef::Draft));
        rows.extend(features.iter().map(IndexRowRef::Feature));
        rows.extend(routes.iter().map(IndexRowRef::Route));
        rows.extend(edges.iter().map(IndexRowRef::Edge));
        (metadata, rows)
    }

    /// Clones and structurally sorts all rows into deterministic identity order.
    pub fn normalized_inventory(&self) -> Vec<IndexRow> {
        let mut rows: Vec<_> = self
            .inventory()
            .1
            .into_iter()
            .map(IndexRowRef::to_owned)
            .collect();
        rows.sort();
        rows
    }
}
