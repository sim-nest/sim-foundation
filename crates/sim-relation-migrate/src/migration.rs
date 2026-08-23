//! Migration program model, admission, derivation, and attestations.
//!
//! Migration programs are admitted before a provider sees them. Admission
//! proves a single revision chain, exact schema transitions, typed backfills,
//! and the declared final target.
//!
//! ```
//! use sim_relation_migrate::{derive_lossless, OperationKind};
//! use sim_relation_schema::{fixtures, AcceptAllValues};
//! let schema = fixtures::document(&AcceptAllValues).unwrap();
//! assert!(derive_lossless(&schema, &schema).unwrap().is_empty());
//! let _: Option<OperationKind> = None;
//! ```

use sim_relation_core::{ColumnName, ConstraintName, IndexName, RelationId, TableName};
use sim_relation_plan::CheckedMutation;
use sim_relation_schema::{Column, Constraint, Index, Schema, Table};

/// A precisely described schema edit.
#[derive(Clone, Debug)]
pub enum OperationKind {
    /// Create a table.
    CreateTable(Table),
    /// Drop a table and its data.
    DropTable(TableName),
    /// Rename a table.
    RenameTable {
        /// Old name.
        from: TableName,
        /// New name.
        to: TableName,
    },
    /// Add a column.
    AddColumn {
        /// Owning table.
        table: TableName,
        /// New column.
        column: Column,
    },
    /// Drop a column and its data.
    DropColumn {
        /// Owning table.
        table: TableName,
        /// Removed column.
        column: ColumnName,
    },
    /// Rename a column.
    RenameColumn {
        /// Owning table.
        table: TableName,
        /// Old name.
        from: ColumnName,
        /// New name.
        to: ColumnName,
    },
    /// Alter a column domain, nullability, default, or generation rule.
    AlterColumn {
        /// Owning table.
        table: TableName,
        /// Changed column.
        column: ColumnName,
    },
    /// Add a constraint.
    AddConstraint {
        /// Owning table.
        table: TableName,
        /// Added constraint.
        constraint: Constraint,
    },
    /// Drop a constraint.
    DropConstraint {
        /// Owning table.
        table: TableName,
        /// Removed constraint.
        constraint: ConstraintName,
    },
    /// Add an index.
    AddIndex {
        /// Owning table.
        table: TableName,
        /// Added index.
        index: Index,
    },
    /// Drop an index.
    DropIndex {
        /// Owning table.
        table: TableName,
        /// Removed index.
        index: IndexName,
    },
    /// Run an already admitted data mutation during the transition.
    Backfill(Box<CheckedMutation>),
}

/// One exact state transition. The complete output snapshot makes omission
/// impossible: the declared operation cannot claim a target it does not produce.
#[derive(Clone, Debug)]
pub struct Operation {
    before: RelationId,
    after: Schema,
    kind: OperationKind,
}
impl Operation {
    /// Creates an operation from its exact input identity and output snapshot.
    pub fn new(before: RelationId, after: Schema, kind: OperationKind) -> Self {
        Self {
            before,
            after,
            kind,
        }
    }
    /// Required input schema identity.
    pub fn before(&self) -> &RelationId {
        &self.before
    }
    /// Produced schema.
    pub fn after(&self) -> &Schema {
        &self.after
    }
    /// Described edit.
    pub fn kind(&self) -> &OperationKind {
        &self.kind
    }
}

/// An authored revision in a strictly linear history.
#[derive(Clone, Debug)]
pub struct Revision {
    id: RelationId,
    parent: Option<RelationId>,
    target: RelationId,
    operations: Vec<Operation>,
}
impl Revision {
    /// Creates a revision with a stable caller-issued id, exact parent, target,
    /// and ordered operations.
    pub fn new(
        id: RelationId,
        parent: Option<RelationId>,
        target: RelationId,
        operations: Vec<Operation>,
    ) -> Self {
        Self {
            id,
            parent,
            target,
            operations,
        }
    }
    /// Revision identity.
    pub fn id(&self) -> &RelationId {
        &self.id
    }
    /// Exact predecessor revision.
    pub fn parent(&self) -> Option<&RelationId> {
        self.parent.as_ref()
    }
    /// Declared logical target.
    pub fn target(&self) -> &RelationId {
        &self.target
    }
    /// Ordered operations.
    pub fn operations(&self) -> &[Operation] {
        &self.operations
    }
}

/// An authored upgrade path.
#[derive(Clone, Debug)]
pub struct MigrationProgram {
    /// Identity of the revision already applied at the starting state.
    pub base_revision: RelationId,
    /// Exact starting logical schema.
    pub base_schema: Schema,
    /// Ordered, linear revisions.
    pub revisions: Vec<Revision>,
    /// Required final logical schema identity.
    pub target_schema: RelationId,
}

/// Opaque proof that a migration program passed simulation.
#[derive(Clone, Debug)]
pub struct CheckedProgram {
    program: MigrationProgram,
}
impl CheckedProgram {
    /// Returns the admitted program for provider execution.
    pub fn program(&self) -> &MigrationProgram {
        &self.program
    }
}

/// Failure to prove a migration program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationError {
    /// A revision does not name the preceding revision.
    WrongParent,
    /// An operation does not consume the current simulated schema.
    StaleBefore,
    /// A backfill was admitted against a different schema.
    InvalidBackfill,
    /// A revision does not produce its declared target.
    RevisionTargetMismatch,
    /// The program does not produce its final target.
    ProgramTargetMismatch,
    /// An operation's claimed edit does not match the before/after snapshots.
    IncompleteOperationCoverage,
    /// The requested automatic diff is destructive, narrowing, or ambiguous.
    AuthoredOperationRequired,
    /// Schema identity could not be calculated.
    Identity,
}

/// Simulates and admits the whole migration program.
pub fn admit(program: MigrationProgram) -> Result<CheckedProgram, MigrationError> {
    let mut schema = program.base_schema.clone();
    let mut parent = program.base_revision.clone();
    for revision in &program.revisions {
        if revision.parent.as_ref() != Some(&parent) {
            return Err(MigrationError::WrongParent);
        }
        for operation in &revision.operations {
            let current = schema.id().map_err(|_| MigrationError::Identity)?;
            if operation.before != current {
                return Err(MigrationError::StaleBefore);
            }
            validate_operation(&schema, operation)?;
            if let OperationKind::Backfill(mutation) = &operation.kind
                && mutation.schema_id() != &current
            {
                return Err(MigrationError::InvalidBackfill);
            }
            schema = operation.after.clone();
        }
        if schema.id().map_err(|_| MigrationError::Identity)? != revision.target {
            return Err(MigrationError::RevisionTargetMismatch);
        }
        parent = revision.id.clone();
    }
    if schema.id().map_err(|_| MigrationError::Identity)? != program.target_schema {
        return Err(MigrationError::ProgramTargetMismatch);
    }
    Ok(CheckedProgram { program })
}

fn validate_operation(before: &Schema, operation: &Operation) -> Result<(), MigrationError> {
    let after = &operation.after;
    let bt = before.tables();
    let at = after.tables();
    let ok = match &operation.kind {
        OperationKind::CreateTable(table) => {
            !has_table(bt, table.name())
                && has_table(at, table.name())
                && at.len() == bt.len() + 1
                && bt.iter().all(|old| at.contains(old))
        }
        OperationKind::DropTable(name) => {
            has_table(bt, name)
                && !has_table(at, name)
                && bt.len() == at.len() + 1
                && at.iter().all(|new| bt.contains(new))
        }
        OperationKind::AddColumn { table, column } => {
            table_pair(bt, at, table).is_some_and(|(b, a)| {
                !has_column(b, column.name())
                    && has_column(a, column.name())
                    && a.columns().len() == b.columns().len() + 1
                    && b.columns().iter().all(|old| a.columns().contains(old))
                    && b.constraints() == a.constraints()
                    && b.indexes() == a.indexes()
                    && same_other_tables(bt, at, table)
            })
        }
        OperationKind::DropColumn { table, column } => {
            table_pair(bt, at, table).is_some_and(|(b, a)| {
                has_column(b, column)
                    && !has_column(a, column)
                    && b.columns().len() == a.columns().len() + 1
                    && a.columns().iter().all(|new| b.columns().contains(new))
                    && b.constraints() == a.constraints()
                    && b.indexes() == a.indexes()
                    && same_other_tables(bt, at, table)
            })
        }
        OperationKind::AddConstraint { table, constraint } => table_pair(bt, at, table)
            .is_some_and(|(b, a)| {
                a.constraints().len() == b.constraints().len() + 1
                    && a.constraints().contains(constraint)
            }),
        OperationKind::DropConstraint { table, .. } => table_pair(bt, at, table)
            .is_some_and(|(b, a)| b.constraints().len() == a.constraints().len() + 1),
        OperationKind::AddIndex { table, index } => {
            table_pair(bt, at, table).is_some_and(|(b, a)| {
                a.indexes().len() == b.indexes().len() + 1 && a.indexes().contains(index)
            })
        }
        OperationKind::DropIndex { table, .. } => table_pair(bt, at, table)
            .is_some_and(|(b, a)| b.indexes().len() == a.indexes().len() + 1),
        OperationKind::RenameTable { from, to } => {
            has_table(bt, from) && !has_table(bt, to) && !has_table(at, from) && has_table(at, to)
        }
        OperationKind::RenameColumn { table, from, to } => {
            table_pair(bt, at, table).is_some_and(|(b, a)| {
                has_column(b, from)
                    && !has_column(b, to)
                    && !has_column(a, from)
                    && has_column(a, to)
            })
        }
        OperationKind::AlterColumn { table, column } => {
            table_pair(bt, at, table).is_some_and(|(b, a)| {
                has_column(b, column) && has_column(a, column) && b.columns() != a.columns()
            })
        }
        OperationKind::Backfill(_) => before.id().ok() == after.id().ok(),
    };
    if ok {
        Ok(())
    } else {
        Err(MigrationError::IncompleteOperationCoverage)
    }
}
fn has_table(tables: &[Table], name: &TableName) -> bool {
    tables.iter().any(|t| t.name() == name)
}
fn has_column(table: &Table, name: &ColumnName) -> bool {
    table.columns().iter().any(|c| c.name() == name)
}
fn table_pair<'a>(
    before: &'a [Table],
    after: &'a [Table],
    name: &TableName,
) -> Option<(&'a Table, &'a Table)> {
    Some((
        before.iter().find(|t| t.name() == name)?,
        after.iter().find(|t| t.name() == name)?,
    ))
}
fn same_other_tables(before: &[Table], after: &[Table], changed: &TableName) -> bool {
    before.len() == after.len()
        && before
            .iter()
            .filter(|table| table.name() != changed)
            .all(|table| after.contains(table))
}

/// Derives only lossless table creation and nullable column addition operations.
/// Every other difference fails closed and requires authored intent.
pub fn derive_lossless(before: &Schema, after: &Schema) -> Result<Vec<Operation>, MigrationError> {
    let mut operations = Vec::new();
    let mut current = before.clone();
    for table in after.tables() {
        match current.tables().iter().find(|t| t.name() == table.name()) {
            None => {
                if after.tables().len() != before.tables().len() + 1 {
                    return Err(MigrationError::AuthoredOperationRequired);
                }
                operations.push(Operation::new(
                    current.id().map_err(|_| MigrationError::Identity)?,
                    after.clone(),
                    OperationKind::CreateTable(table.clone()),
                ));
                current = after.clone();
            }
            Some(old) if old != table => {
                let additions: Vec<_> = table
                    .columns()
                    .iter()
                    .filter(|c| !has_column(old, c.name()))
                    .collect();
                if additions.len() != 1
                    || !additions[0].nullable()
                    || table.columns().len() != old.columns().len() + 1
                    || after.tables().len() != before.tables().len()
                {
                    return Err(MigrationError::AuthoredOperationRequired);
                }
                operations.push(Operation::new(
                    current.id().map_err(|_| MigrationError::Identity)?,
                    after.clone(),
                    OperationKind::AddColumn {
                        table: table.name().clone(),
                        column: additions[0].clone(),
                    },
                ));
                current = after.clone();
            }
            _ => {}
        }
    }
    if current.id().map_err(|_| MigrationError::Identity)?
        != after.id().map_err(|_| MigrationError::Identity)?
    {
        return Err(MigrationError::AuthoredOperationRequired);
    }
    Ok(operations)
}

/// Provider features needed for safely applying an admitted program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MigrationCapabilities {
    /// Provider applies all DDL and backfills atomically.
    pub transactional_ddl: bool,
    /// Provider can normalize and report live objects after application.
    pub post_apply_introspection: bool,
}
impl MigrationCapabilities {
    /// Requires both safety capabilities.
    pub fn require(self) -> Result<(), CapabilityError> {
        if self.transactional_ddl && self.post_apply_introspection {
            Ok(())
        } else {
            Err(CapabilityError)
        }
    }
}
/// Missing provider migration capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityError;

/// Signed or otherwise provider-authenticated evidence of observed state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaAttestation {
    /// Admitted logical schema.
    pub logical_schema: RelationId,
    /// Normalized live physical-object identity.
    pub physical_schema: RelationId,
    /// Revision actually applied.
    pub revision: RelationId,
}

/// Exact adoption declaration for an existing store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdoptionManifest {
    /// Logical schema the old file is claimed to implement.
    pub logical_schema: RelationId,
    /// Exact normalized identity observed from that file.
    pub physical_schema: RelationId,
}
impl AdoptionManifest {
    /// Accepts adoption only when live introspection exactly matches the
    /// authored file identity; a metadata row cannot override drift.
    pub fn verify(&self, live_physical_schema: &RelationId) -> Result<(), AdoptionError> {
        if &self.physical_schema == live_physical_schema {
            Ok(())
        } else {
            Err(AdoptionError::ExternalDrift)
        }
    }
}
/// Adoption verification failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdoptionError {
    /// Live managed objects differ from the manifest.
    ExternalDrift,
}
