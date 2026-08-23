use crate::{
    Column, Constraint, DefaultValue, GeneratedValue, Index, Schema, SchemaError, Table,
    ValueShapeValidator, View,
};
use sim_kernel::Datum;
use sim_relation_core::{ColumnName, DomainCatalog, DomainId, SchemaName, TableName};

/// Compact column builder.
pub struct ColumnBuilder(Column);
impl ColumnBuilder {
    /// Starts a required column.
    pub fn required(name: ColumnName, domain: DomainId) -> Self {
        Self(Column {
            name,
            domain,
            nullable: false,
            default: None,
            generated: None,
        })
    }
    /// Starts a nullable column.
    pub fn nullable(name: ColumnName, domain: DomainId) -> Self {
        Self(Column {
            name,
            domain,
            nullable: true,
            default: None,
            generated: None,
        })
    }
    /// Adds a literal default.
    pub fn default(mut self, value: Datum) -> Self {
        self.0.default = Some(DefaultValue(value));
        self
    }
    /// Adds a generated expression.
    pub fn generated(mut self, value: GeneratedValue) -> Self {
        self.0.generated = Some(value);
        self
    }
    /// Finishes the declaration.
    pub fn build(self) -> Column {
        self.0
    }
}
/// Compact table builder.
pub struct TableBuilder(Table);
impl TableBuilder {
    /// Starts a table.
    pub fn new(name: TableName) -> Self {
        Self(Table {
            name,
            columns: Vec::new(),
            constraints: Vec::new(),
            indexes: Vec::new(),
        })
    }
    /// Appends a semantic column.
    pub fn column(mut self, column: Column) -> Self {
        self.0.columns.push(column);
        self
    }
    /// Adds an unordered constraint declaration.
    pub fn constraint(mut self, value: Constraint) -> Self {
        self.0.constraints.push(value);
        self
    }
    /// Adds an unordered index declaration.
    pub fn index(mut self, value: Index) -> Self {
        self.0.indexes.push(value);
        self
    }
    /// Finishes the declaration; graph validation occurs in [`SchemaBuilder::build`].
    pub fn build(self) -> Table {
        self.0
    }
}
/// Compact complete-schema builder.
pub struct SchemaBuilder {
    name: SchemaName,
    tables: Vec<Table>,
    views: Vec<View>,
}
impl SchemaBuilder {
    /// Starts a logical schema.
    pub fn new(name: SchemaName) -> Self {
        Self {
            name,
            tables: Vec::new(),
            views: Vec::new(),
        }
    }
    /// Adds a table.
    pub fn table(mut self, value: Table) -> Self {
        self.tables.push(value);
        self
    }
    /// Adds a view.
    pub fn view(mut self, value: View) -> Self {
        self.views.push(value);
        self
    }
    /// Validates and constructs the schema.
    pub fn build(
        self,
        domains: &DomainCatalog,
        validator: &impl ValueShapeValidator,
    ) -> Result<Schema, SchemaError> {
        Schema::new(self.name, self.tables, self.views, domains, validator)
    }
}
