use crate::{SchemaError, ValueShapeValidator};
use sim_kernel::{Datum, Symbol};
use sim_relation_core::{
    ColumnName, ConstraintName, DomainCatalog, DomainId, IndexName, RelationId, SchemaName,
    TableName, ToRelationDatum, ViewName,
};

/// A literal default checked through its logical domain Shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefaultValue(pub Datum);
/// A generated expression and the columns it reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedValue {
    pub(crate) expression: Datum,
    pub(crate) depends_on: Vec<ColumnName>,
}
impl GeneratedValue {
    /// Creates an expression with ordered dependency names.
    pub fn new(expression: Datum, depends_on: impl IntoIterator<Item = ColumnName>) -> Self {
        Self {
            expression,
            depends_on: depends_on.into_iter().collect(),
        }
    }
}
/// A logical column declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub(crate) name: ColumnName,
    pub(crate) domain: DomainId,
    pub(crate) nullable: bool,
    pub(crate) default: Option<DefaultValue>,
    pub(crate) generated: Option<GeneratedValue>,
}
impl Column {
    /// Returns the name.
    pub fn name(&self) -> &ColumnName {
        &self.name
    }
    /// Returns the domain.
    pub fn domain(&self) -> &DomainId {
        &self.domain
    }
    /// Returns whether NULL is permitted.
    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}
/// An ordered primary key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrimaryKey {
    /// Stable constraint name.
    pub name: ConstraintName,
    /// Key columns in comparison order.
    pub columns: Vec<ColumnName>,
}
/// An ordered unique key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniqueConstraint {
    /// Stable constraint name.
    pub name: ConstraintName,
    /// Key columns in comparison order.
    pub columns: Vec<ColumnName>,
}
/// A table-local check and the columns in its scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckConstraint {
    /// Stable constraint name.
    pub name: ConstraintName,
    /// Portable check expression.
    pub expression: Datum,
    /// Columns visible to the expression.
    pub columns: Vec<ColumnName>,
}
/// An ordered foreign-key mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignKey {
    /// Stable constraint name.
    pub name: ConstraintName,
    /// Local columns in mapping order.
    pub columns: Vec<ColumnName>,
    /// Referenced table.
    pub target_table: TableName,
    /// Referenced columns in mapping order.
    pub target_columns: Vec<ColumnName>,
}
/// A table constraint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constraint {
    /// One primary key.
    Primary(PrimaryKey),
    /// A uniqueness constraint.
    Unique(UniqueConstraint),
    /// A table-local predicate.
    Check(CheckConstraint),
    /// A referential constraint.
    Foreign(ForeignKey),
}
impl Constraint {
    pub(crate) fn name(&self) -> &ConstraintName {
        match self {
            Self::Primary(v) => &v.name,
            Self::Unique(v) => &v.name,
            Self::Check(v) => &v.name,
            Self::Foreign(v) => &v.name,
        }
    }
}
/// An index whose column order is semantic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    /// Stable index name.
    pub name: IndexName,
    /// Indexed columns in key order.
    pub columns: Vec<ColumnName>,
    /// Whether the provider must enforce uniqueness.
    pub unique: bool,
}
/// A validated table declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub(crate) name: TableName,
    pub(crate) columns: Vec<Column>,
    pub(crate) constraints: Vec<Constraint>,
    pub(crate) indexes: Vec<Index>,
}
impl Table {
    /// Returns the name.
    pub fn name(&self) -> &TableName {
        &self.name
    }
    /// Returns columns in semantic order.
    pub fn columns(&self) -> &[Column] {
        &self.columns
    }
    /// Returns constraints in canonical name order.
    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }
    /// Returns indexes in canonical name order.
    pub fn indexes(&self) -> &[Index] {
        &self.indexes
    }
}
/// A logical view and its table/view dependencies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// Stable view name.
    pub name: ViewName,
    /// Portable logical query.
    pub query: Datum,
    /// Tables read by the query.
    pub table_dependencies: Vec<TableName>,
    /// Views read by the query.
    pub view_dependencies: Vec<ViewName>,
}
/// Complete provider-neutral schema intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema {
    pub(crate) name: SchemaName,
    pub(crate) tables: Vec<Table>,
    pub(crate) views: Vec<View>,
}
impl Schema {
    /// Validates a complete schema graph.
    pub fn new(
        name: SchemaName,
        tables: impl IntoIterator<Item = Table>,
        views: impl IntoIterator<Item = View>,
        domains: &DomainCatalog,
        validator: &impl ValueShapeValidator,
    ) -> Result<Self, SchemaError> {
        crate::validation::validate(
            name,
            tables.into_iter().collect(),
            views.into_iter().collect(),
            domains,
            validator,
        )
    }
    /// Returns its name.
    pub fn name(&self) -> &SchemaName {
        &self.name
    }
    /// Returns tables in canonical name order.
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }
    /// Returns views in canonical name order.
    pub fn views(&self) -> &[View] {
        &self.views
    }
    /// Returns its canonical content identity.
    pub fn id(&self) -> Result<RelationId, sim_kernel::Error> {
        RelationId::of(self)
    }
}

fn sym(name: &str, value: Symbol) -> (Symbol, Datum) {
    (Symbol::new(name), Datum::Symbol(value))
}
fn node(tag: &str, fields: Vec<(Symbol, Datum)>) -> Datum {
    Datum::Node {
        tag: Symbol::qualified("relation-schema", tag),
        fields,
    }
}
fn names<T>(values: &[T], f: impl Fn(&T) -> Symbol) -> Datum {
    Datum::Vector(values.iter().map(|v| Datum::Symbol(f(v))).collect())
}
impl ToRelationDatum for Column {
    fn to_datum(&self) -> Datum {
        node(
            "column",
            vec![
                sym("name", self.name.symbol().clone()),
                sym("domain", self.domain.symbol().clone()),
                (Symbol::new("nullable"), Datum::Bool(self.nullable)),
                (
                    Symbol::new("default"),
                    self.default.as_ref().map_or(Datum::Nil, |v| v.0.clone()),
                ),
                (
                    Symbol::new("generated"),
                    self.generated.as_ref().map_or(Datum::Nil, |v| {
                        node(
                            "generated",
                            vec![
                                (Symbol::new("expression"), v.expression.clone()),
                                (
                                    Symbol::new("depends-on"),
                                    names(&v.depends_on, |n| n.symbol().clone()),
                                ),
                            ],
                        )
                    }),
                ),
            ],
        )
    }
}
impl ToRelationDatum for Constraint {
    fn to_datum(&self) -> Datum {
        match self {
            Self::Primary(v) => node(
                "primary",
                vec![
                    sym("name", v.name.symbol().clone()),
                    (
                        Symbol::new("columns"),
                        names(&v.columns, |n| n.symbol().clone()),
                    ),
                ],
            ),
            Self::Unique(v) => node(
                "unique",
                vec![
                    sym("name", v.name.symbol().clone()),
                    (
                        Symbol::new("columns"),
                        names(&v.columns, |n| n.symbol().clone()),
                    ),
                ],
            ),
            Self::Check(v) => node(
                "check",
                vec![
                    sym("name", v.name.symbol().clone()),
                    (Symbol::new("expression"), v.expression.clone()),
                    (
                        Symbol::new("columns"),
                        names(&v.columns, |n| n.symbol().clone()),
                    ),
                ],
            ),
            Self::Foreign(v) => node(
                "foreign",
                vec![
                    sym("name", v.name.symbol().clone()),
                    (
                        Symbol::new("columns"),
                        names(&v.columns, |n| n.symbol().clone()),
                    ),
                    sym("target-table", v.target_table.symbol().clone()),
                    (
                        Symbol::new("target-columns"),
                        names(&v.target_columns, |n| n.symbol().clone()),
                    ),
                ],
            ),
        }
    }
}
impl ToRelationDatum for Index {
    fn to_datum(&self) -> Datum {
        node(
            "index",
            vec![
                sym("name", self.name.symbol().clone()),
                (
                    Symbol::new("columns"),
                    names(&self.columns, |n| n.symbol().clone()),
                ),
                (Symbol::new("unique"), Datum::Bool(self.unique)),
            ],
        )
    }
}
impl ToRelationDatum for Table {
    fn to_datum(&self) -> Datum {
        node(
            "table",
            vec![
                sym("name", self.name.symbol().clone()),
                (
                    Symbol::new("columns"),
                    Datum::Vector(self.columns.iter().map(ToRelationDatum::to_datum).collect()),
                ),
                (
                    Symbol::new("constraints"),
                    Datum::Vector(
                        self.constraints
                            .iter()
                            .map(ToRelationDatum::to_datum)
                            .collect(),
                    ),
                ),
                (
                    Symbol::new("indexes"),
                    Datum::Vector(self.indexes.iter().map(ToRelationDatum::to_datum).collect()),
                ),
            ],
        )
    }
}
impl ToRelationDatum for View {
    fn to_datum(&self) -> Datum {
        node(
            "view",
            vec![
                sym("name", self.name.symbol().clone()),
                (Symbol::new("query"), self.query.clone()),
                (
                    Symbol::new("tables"),
                    names(&self.table_dependencies, |n| n.symbol().clone()),
                ),
                (
                    Symbol::new("views"),
                    names(&self.view_dependencies, |n| n.symbol().clone()),
                ),
            ],
        )
    }
}
impl ToRelationDatum for Schema {
    fn to_datum(&self) -> Datum {
        node(
            "logical-schema",
            vec![
                sym("name", self.name.symbol().clone()),
                (
                    Symbol::new("tables"),
                    Datum::Vector(self.tables.iter().map(ToRelationDatum::to_datum).collect()),
                ),
                (
                    Symbol::new("views"),
                    Datum::Vector(self.views.iter().map(ToRelationDatum::to_datum).collect()),
                ),
            ],
        )
    }
}
