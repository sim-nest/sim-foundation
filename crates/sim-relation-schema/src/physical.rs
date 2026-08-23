use sim_kernel::{Datum, Symbol};
use sim_relation_core::{
    ColumnName, DomainId, IndexName, ProviderName, RelationId, RevisionName, SchemaName,
    StorageRepr, TableName, ToRelationDatum,
};
use std::collections::BTreeSet;

/// A normalized provider-observed column.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalColumn {
    /// Observed column name.
    pub name: ColumnName,
    /// Normalized logical domain.
    pub domain: DomainId,
    /// Exact provider-boundary representation.
    pub storage: StorageRepr,
    /// Observed nullability.
    pub nullable: bool,
    /// Provider ordinal preserving semantic column order.
    pub ordinal: u32,
}
/// A normalized provider-observed index.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalIndex {
    /// Observed index name.
    pub name: IndexName,
    /// Observed key columns in order.
    pub columns: Vec<ColumnName>,
    /// Observed uniqueness.
    pub unique: bool,
}
/// A normalized provider-observed table.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalTable {
    /// Observed table name.
    pub name: TableName,
    /// Observed columns.
    pub columns: Vec<PhysicalColumn>,
    /// Observed indexes.
    pub indexes: Vec<PhysicalIndex>,
}
/// Immutable normalized evidence observed from a live provider catalog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhysicalSchema {
    provider: ProviderName,
    schema: SchemaName,
    revision: RevisionName,
    tables: Vec<PhysicalTable>,
}
impl PhysicalSchema {
    /// Normalizes an observed catalog. Tables/indexes are unordered; columns are sorted by provider ordinal.
    pub fn normalize(
        provider: ProviderName,
        schema: SchemaName,
        revision: RevisionName,
        mut tables: Vec<PhysicalTable>,
    ) -> Result<Self, &'static str> {
        let mut table_names = BTreeSet::new();
        for table in &mut tables {
            if !table_names.insert(table.name.clone()) {
                return Err("duplicate physical table");
            }
            table.columns.sort_by_key(|v| v.ordinal);
            if table
                .columns
                .windows(2)
                .any(|v| v[0].ordinal == v[1].ordinal)
            {
                return Err("duplicate physical ordinal");
            }
            let mut names = BTreeSet::new();
            if table.columns.iter().any(|v| !names.insert(v.name.clone())) {
                return Err("duplicate physical column");
            }
            table.indexes.sort_by(|a, b| a.name.cmp(&b.name));
        }
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(Self {
            provider,
            schema,
            revision,
            tables,
        })
    }
    /// Returns normalized tables.
    pub fn tables(&self) -> &[PhysicalTable] {
        &self.tables
    }
    /// Returns the distinct physical identity.
    pub fn id(&self) -> Result<RelationId, sim_kernel::Error> {
        RelationId::of(self)
    }
}
fn storage(v: StorageRepr) -> Symbol {
    Symbol::new(match v {
        StorageRepr::Bool => "bool",
        StorageRepr::I64 => "i64",
        StorageRepr::F64 => "f64",
        StorageRepr::Text => "text",
        StorageRepr::Bytes => "bytes",
    })
}
impl ToRelationDatum for PhysicalSchema {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation-schema", "physical-schema"),
            fields: vec![
                (
                    Symbol::new("provider"),
                    Datum::Symbol(self.provider.symbol().clone()),
                ),
                (
                    Symbol::new("schema"),
                    Datum::Symbol(self.schema.symbol().clone()),
                ),
                (
                    Symbol::new("revision"),
                    Datum::Symbol(self.revision.symbol().clone()),
                ),
                (
                    Symbol::new("tables"),
                    Datum::Vector(
                        self.tables
                            .iter()
                            .map(|t| Datum::Node {
                                tag: Symbol::qualified("relation-schema", "physical-table"),
                                fields: vec![
                                    (Symbol::new("name"), Datum::Symbol(t.name.symbol().clone())),
                                    (
                                        Symbol::new("columns"),
                                        Datum::Vector(
                                            t.columns
                                                .iter()
                                                .map(|c| Datum::Node {
                                                    tag: Symbol::qualified(
                                                        "relation-schema",
                                                        "physical-column",
                                                    ),
                                                    fields: vec![
                                                        (
                                                            Symbol::new("name"),
                                                            Datum::Symbol(c.name.symbol().clone()),
                                                        ),
                                                        (
                                                            Symbol::new("domain"),
                                                            Datum::Symbol(
                                                                c.domain.symbol().clone(),
                                                            ),
                                                        ),
                                                        (
                                                            Symbol::new("storage"),
                                                            Datum::Symbol(storage(c.storage)),
                                                        ),
                                                        (
                                                            Symbol::new("nullable"),
                                                            Datum::Bool(c.nullable),
                                                        ),
                                                        (
                                                            Symbol::new("ordinal"),
                                                            Datum::Number(
                                                                sim_kernel::NumberLiteral {
                                                                    domain: Symbol::qualified(
                                                                        "core", "u32",
                                                                    ),
                                                                    canonical: c
                                                                        .ordinal
                                                                        .to_string(),
                                                                },
                                                            ),
                                                        ),
                                                    ],
                                                })
                                                .collect(),
                                        ),
                                    ),
                                    (
                                        Symbol::new("indexes"),
                                        Datum::Vector(
                                            t.indexes
                                                .iter()
                                                .map(|i| Datum::Node {
                                                    tag: Symbol::qualified(
                                                        "relation-schema",
                                                        "physical-index",
                                                    ),
                                                    fields: vec![
                                                        (
                                                            Symbol::new("name"),
                                                            Datum::Symbol(i.name.symbol().clone()),
                                                        ),
                                                        (
                                                            Symbol::new("columns"),
                                                            Datum::Vector(
                                                                i.columns
                                                                    .iter()
                                                                    .map(|n| {
                                                                        Datum::Symbol(
                                                                            n.symbol().clone(),
                                                                        )
                                                                    })
                                                                    .collect(),
                                                            ),
                                                        ),
                                                        (
                                                            Symbol::new("unique"),
                                                            Datum::Bool(i.unique),
                                                        ),
                                                    ],
                                                })
                                                .collect(),
                                        ),
                                    ),
                                ],
                            })
                            .collect(),
                    ),
                ),
            ],
        }
    }
}
