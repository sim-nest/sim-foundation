use crate::{
    ColumnBuilder, Constraint, ForeignKey, PrimaryKey, Schema, SchemaBuilder, SchemaError,
    TableBuilder, UniqueConstraint, ValueShapeValidator,
};
use sim_kernel::Symbol;
use sim_relation_core::{BaseDomain, ColumnName, DomainCatalog, TableName};

fn n<T: TryFrom<Symbol>>(value: &str) -> T
where
    T::Error: std::fmt::Debug,
{
    T::try_from(Symbol::new(value)).expect("fixture name")
}
fn domains() -> DomainCatalog {
    DomainCatalog::new([
        BaseDomain::Bool.spec(),
        BaseDomain::I64.spec(),
        BaseDomain::F64.spec(),
        BaseDomain::Text.spec(),
        BaseDomain::Bytes.spec(),
    ])
    .expect("fixture domains")
}
fn base(
    name: &str,
    validator: &impl ValueShapeValidator,
    extra: bool,
) -> Result<Schema, SchemaError> {
    let id: ColumnName = n("id");
    let owner: ColumnName = n("owner_id");
    let items: TableName = n("items");
    let owners: TableName = n("owners");
    let owners_t = TableBuilder::new(owners.clone())
        .column(ColumnBuilder::required(id.clone(), BaseDomain::I64.id()).build())
        .constraint(Constraint::Primary(PrimaryKey {
            name: n("owners_pk"),
            columns: vec![id.clone()],
        }))
        .build();
    let mut item = TableBuilder::new(items.clone())
        .column(ColumnBuilder::required(id.clone(), BaseDomain::I64.id()).build())
        .column(ColumnBuilder::required(owner.clone(), BaseDomain::I64.id()).build())
        .column(ColumnBuilder::required(n("title"), BaseDomain::Text.id()).build())
        .constraint(Constraint::Primary(PrimaryKey {
            name: n("items_pk"),
            columns: vec![id.clone()],
        }))
        .constraint(Constraint::Foreign(ForeignKey {
            name: n("items_owner_fk"),
            columns: vec![owner],
            target_table: owners.clone(),
            target_columns: vec![id],
        }));
    if extra {
        item = item.constraint(Constraint::Unique(UniqueConstraint {
            name: n("items_title_uq"),
            columns: vec![n("title")],
        }));
    }
    SchemaBuilder::new(n(name))
        .table(item.build())
        .table(owners_t)
        .build(&domains(), validator)
}
/// Exact logical fixture for document stores.
pub fn document(v: &impl ValueShapeValidator) -> Result<Schema, SchemaError> {
    base("document", v, false)
}
/// Exact logical fixture for Gantt stores.
pub fn gantt(v: &impl ValueShapeValidator) -> Result<Schema, SchemaError> {
    base("gantt", v, true)
}
/// Exact logical fixture for ledger stores.
pub fn ledger(v: &impl ValueShapeValidator) -> Result<Schema, SchemaError> {
    base("ledger", v, true)
}
/// Exact logical fixture for relation-directory stores.
pub fn relation_directory(v: &impl ValueShapeValidator) -> Result<Schema, SchemaError> {
    base("relation-directory", v, false)
}
