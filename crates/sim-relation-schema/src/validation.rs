use crate::{Column, Constraint, Schema, Table, View};
use sim_kernel::Datum;
use sim_relation_core::{
    ColumnName, ConstraintName, DomainCatalog, DomainId, IndexName, SchemaName, TableName, ViewName,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

/// Injected runtime boundary for checking a Datum through a domain Shape.
pub trait ValueShapeValidator {
    /// Returns whether `value` satisfies the Shape registered for `domain`.
    fn accepts(&self, domain: &DomainId, value: &Datum) -> bool;
}
/// Validator useful when expression values have already been Shape-checked upstream.
pub struct AcceptAllValues;
impl ValueShapeValidator for AcceptAllValues {
    fn accepts(&self, _: &DomainId, _: &Datum) -> bool {
        true
    }
}

/// A schema refusal, reported before codec/provider dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaError {
    /// A table name repeats.
    DuplicateTable(TableName),
    /// A view name repeats.
    DuplicateView(ViewName),
    /// A column repeats in one table.
    DuplicateColumn {
        /// Containing table.
        table: TableName,
        /// Repeated column.
        column: ColumnName,
    },
    /// A constraint repeats in one table.
    DuplicateConstraint {
        /// Containing table.
        table: TableName,
        /// Repeated constraint.
        constraint: ConstraintName,
    },
    /// An index repeats in one table.
    DuplicateIndex {
        /// Containing table.
        table: TableName,
        /// Repeated index.
        index: IndexName,
    },
    /// A domain id is absent from the catalog.
    DanglingDomain(DomainId),
    /// A column reference is absent or crosses constraint scope.
    DanglingColumn {
        /// Referencing table.
        table: TableName,
        /// Missing column.
        column: ColumnName,
    },
    /// A table reference is absent.
    DanglingTable(TableName),
    /// A view reference is absent.
    DanglingView(ViewName),
    /// A key has no columns.
    EmptyKey(ConstraintName),
    /// Primary-key columns cannot be nullable.
    NullablePrimaryKey(ColumnName),
    /// A column declares both default and generation.
    DefaultAndGenerated(ColumnName),
    /// A generated column directly depends on itself.
    GeneratedCycle(ColumnName),
    /// A default is rejected by its domain Shape.
    InvalidDefault(ColumnName),
    /// A generated expression is rejected by its domain Shape.
    InvalidGenerated(ColumnName),
    /// Foreign key sides have different arity.
    ForeignKeyArity(ConstraintName),
    /// Foreign key domains differ at an offset.
    ForeignKeyDomain {
        /// Foreign-key constraint.
        constraint: ConstraintName,
        /// Mismatching mapping offset.
        index: usize,
    },
    /// View dependencies contain a cycle.
    ViewCycle(ViewName),
}
impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SchemaError {}

pub(crate) fn validate(
    name: SchemaName,
    mut tables: Vec<Table>,
    mut views: Vec<View>,
    domains: &DomainCatalog,
    validator: &impl ValueShapeValidator,
) -> Result<Schema, SchemaError> {
    unique(&tables, |v| v.name.clone(), SchemaError::DuplicateTable)?;
    unique(&views, |v| v.name.clone(), SchemaError::DuplicateView)?;
    let table_map: BTreeMap<_, _> = tables.iter().map(|v| (v.name.clone(), v)).collect();
    for table in &tables {
        validate_table(table, &table_map, domains, validator)?;
    }
    let view_names: BTreeSet<_> = views.iter().map(|v| v.name.clone()).collect();
    for view in &views {
        for table in &view.table_dependencies {
            if !table_map.contains_key(table) {
                return Err(SchemaError::DanglingTable(table.clone()));
            }
        }
        for dep in &view.view_dependencies {
            if !view_names.contains(dep) {
                return Err(SchemaError::DanglingView(dep.clone()));
            }
        }
    }
    for view in &views {
        visit_view(
            &view.name,
            &views,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
        )?;
    }
    tables.sort_by(|a, b| a.name.cmp(&b.name));
    views.sort_by(|a, b| a.name.cmp(&b.name));
    for table in &mut tables {
        table.constraints.sort_by(|a, b| a.name().cmp(b.name()));
        table.indexes.sort_by(|a, b| a.name.cmp(&b.name));
    }
    Ok(Schema {
        name,
        tables,
        views,
    })
}
fn unique<T, K: Ord + Clone>(
    values: &[T],
    key: impl Fn(&T) -> K,
    error: impl Fn(K) -> SchemaError,
) -> Result<(), SchemaError> {
    let mut seen = BTreeSet::new();
    for v in values {
        let k = key(v);
        if !seen.insert(k.clone()) {
            return Err(error(k));
        }
    }
    Ok(())
}
fn columns(table: &Table) -> BTreeMap<ColumnName, &Column> {
    table.columns.iter().map(|v| (v.name.clone(), v)).collect()
}
fn validate_table(
    table: &Table,
    tables: &BTreeMap<TableName, &Table>,
    domains: &DomainCatalog,
    validator: &impl ValueShapeValidator,
) -> Result<(), SchemaError> {
    unique(
        &table.columns,
        |v| v.name.clone(),
        |column| SchemaError::DuplicateColumn {
            table: table.name.clone(),
            column,
        },
    )?;
    unique(
        &table.constraints,
        |v| v.name().clone(),
        |constraint| SchemaError::DuplicateConstraint {
            table: table.name.clone(),
            constraint,
        },
    )?;
    unique(
        &table.indexes,
        |v| v.name.clone(),
        |index| SchemaError::DuplicateIndex {
            table: table.name.clone(),
            index,
        },
    )?;
    let cols = columns(table);
    for c in &table.columns {
        if domains.get(&c.domain).is_none() {
            return Err(SchemaError::DanglingDomain(c.domain.clone()));
        }
        if c.default.is_some() && c.generated.is_some() {
            return Err(SchemaError::DefaultAndGenerated(c.name.clone()));
        }
        if let Some(v) = &c.default
            && !validator.accepts(&c.domain, &v.0)
        {
            return Err(SchemaError::InvalidDefault(c.name.clone()));
        }
        if let Some(v) = &c.generated {
            for dep in &v.depends_on {
                require_column(table, &cols, dep)?;
                if dep == &c.name {
                    return Err(SchemaError::GeneratedCycle(c.name.clone()));
                }
            }
            if !validator.accepts(&c.domain, &v.expression) {
                return Err(SchemaError::InvalidGenerated(c.name.clone()));
            }
        }
    }
    for constraint in &table.constraints {
        match constraint {
            Constraint::Primary(v) => {
                key(table, &cols, &v.name, &v.columns)?;
                for n in &v.columns {
                    if cols[n].nullable {
                        return Err(SchemaError::NullablePrimaryKey(n.clone()));
                    }
                }
            }
            Constraint::Unique(v) => key(table, &cols, &v.name, &v.columns)?,
            Constraint::Check(v) => {
                for n in &v.columns {
                    require_column(table, &cols, n)?
                }
            }
            Constraint::Foreign(v) => {
                key(table, &cols, &v.name, &v.columns)?;
                if v.columns.len() != v.target_columns.len() {
                    return Err(SchemaError::ForeignKeyArity(v.name.clone()));
                }
                let target = tables
                    .get(&v.target_table)
                    .ok_or_else(|| SchemaError::DanglingTable(v.target_table.clone()))?;
                let target_cols = columns(target);
                for (i, (left, right)) in v.columns.iter().zip(&v.target_columns).enumerate() {
                    require_column(target, &target_cols, right)?;
                    if cols[left].domain != target_cols[right].domain {
                        return Err(SchemaError::ForeignKeyDomain {
                            constraint: v.name.clone(),
                            index: i,
                        });
                    }
                }
            }
        }
    }
    for index in &table.indexes {
        if index.columns.is_empty() {
            return Err(SchemaError::DanglingColumn {
                table: table.name.clone(),
                column: ColumnName::new(sim_kernel::Symbol::new("<empty-index>")).expect("valid"),
            });
        }
        for n in &index.columns {
            require_column(table, &cols, n)?
        }
    }
    Ok(())
}
fn require_column(
    table: &Table,
    cols: &BTreeMap<ColumnName, &Column>,
    name: &ColumnName,
) -> Result<(), SchemaError> {
    if cols.contains_key(name) {
        Ok(())
    } else {
        Err(SchemaError::DanglingColumn {
            table: table.name.clone(),
            column: name.clone(),
        })
    }
}
fn key(
    table: &Table,
    cols: &BTreeMap<ColumnName, &Column>,
    name: &ConstraintName,
    names: &[ColumnName],
) -> Result<(), SchemaError> {
    if names.is_empty() {
        return Err(SchemaError::EmptyKey(name.clone()));
    }
    for n in names {
        require_column(table, cols, n)?
    }
    Ok(())
}
fn visit_view(
    name: &ViewName,
    views: &[View],
    visiting: &mut BTreeSet<ViewName>,
    done: &mut BTreeSet<ViewName>,
) -> Result<(), SchemaError> {
    if done.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name.clone()) {
        return Err(SchemaError::ViewCycle(name.clone()));
    }
    let view = views
        .iter()
        .find(|v| &v.name == name)
        .expect("existence checked");
    for dep in &view.view_dependencies {
        visit_view(dep, views, visiting, done)?;
    }
    visiting.remove(name);
    done.insert(name.clone());
    Ok(())
}
