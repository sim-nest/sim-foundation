use sim_relation_core::{
    BindingName, Cell, ColumnName, ConstraintName, FieldName, ParameterName, RelationId, Row,
    RowType, SourceName, TableName,
};
use std::fmt;

/// An explicitly qualified field reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRef {
    /// Relation binding.
    pub binding: BindingName,
    /// Field within the binding.
    pub field: FieldName,
}
/// A named scalar projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedScalar {
    /// Output field name.
    pub name: FieldName,
    /// Expression.
    pub scalar: Scalar,
}
/// A named aggregate projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedAggregate {
    /// Output field name.
    pub name: FieldName,
    /// Aggregate expression.
    pub aggregate: Aggregate,
}
/// Join semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinKind {
    /// Matching rows only.
    Inner,
    /// All left rows, nullable right side.
    Left,
    /// Cartesian product.
    Cross,
}
/// Set operation semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    /// Deduplicating union.
    Union,
    /// Multiset union.
    UnionAll,
    /// Intersection.
    Intersect,
    /// Difference.
    Except,
}
/// Sort direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrderDirection {
    /// Ascending.
    Asc,
    /// Descending.
    Desc,
}
/// A typed ordering expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderKey {
    /// Expression.
    pub scalar: Scalar,
    /// Direction.
    pub direction: OrderDirection,
}
/// Portable scalar operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarOp {
    /// Boolean conjunction.
    And,
    /// Boolean disjunction.
    Or,
    /// Boolean negation.
    Not,
    /// Equality.
    Eq,
    /// Inequality.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Null test.
    IsNull,
    /// Null coalescing.
    Coalesce,
}
/// A scalar expression. Subqueries retain lexical access to their outer scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scalar {
    /// Bound field.
    Field(FieldRef),
    /// Typed literal.
    Literal(Cell),
    /// Declared parameter.
    Param(ParameterName),
    /// Portable operator call.
    Call(ScalarOp, Vec<Scalar>),
    /// Conditional expression.
    Case {
        /// Ordered predicate/value branches.
        branches: Vec<(Scalar, Scalar)>,
        /// Optional else expression.
        otherwise: Option<Box<Scalar>>,
    },
    /// Existence subquery.
    Exists(Box<Rel>),
    /// Membership subquery.
    InQuery {
        /// Compared expression.
        value: Box<Scalar>,
        /// Single-column query.
        query: Box<Rel>,
    },
    /// Single-value subquery.
    ScalarQuery(Box<Rel>),
}
/// Aggregate operations with domain-derived result types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Aggregate {
    /// Count all input rows.
    CountAll,
    /// Count non-null values.
    Count(Scalar),
    /// Sum ordered numeric values.
    Sum(Scalar),
    /// Minimum ordered value.
    Min(Scalar),
    /// Maximum ordered value.
    Max(Scalar),
}
/// Complete logical relation algebra.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Rel {
    /// Catalog table scan.
    Scan {
        /// Provider source.
        source: SourceName,
        /// Schema table.
        table: TableName,
        /// Introduced binding.
        bind: BindingName,
    },
    /// Bounded literal rows.
    Values {
        /// Introduced binding.
        bind: BindingName,
        /// Declared row type.
        row_type: RowType,
        /// Rows.
        rows: Vec<Row>,
    },
    /// Projection.
    Project {
        /// Input.
        input: Box<Rel>,
        /// Introduced output binding.
        bind: BindingName,
        /// Fields.
        fields: Vec<NamedScalar>,
    },
    /// Predicate filter.
    Filter {
        /// Input.
        input: Box<Rel>,
        /// Boolean predicate.
        predicate: Scalar,
    },
    /// Binary join.
    Join {
        /// Left relation.
        left: Box<Rel>,
        /// Right relation.
        right: Box<Rel>,
        /// Kind.
        kind: JoinKind,
        /// Boolean condition; ignored only for cross joins.
        on: Scalar,
    },
    /// Grouped projection.
    Group {
        /// Input.
        input: Box<Rel>,
        /// Introduced output binding.
        bind: BindingName,
        /// Group keys.
        keys: Vec<NamedScalar>,
        /// Aggregates.
        aggregates: Vec<NamedAggregate>,
        /// Post-aggregate predicate.
        having: Option<Scalar>,
    },
    /// Compatible set operation.
    Set {
        /// Operation.
        op: SetOp,
        /// Two or more inputs.
        inputs: Vec<Rel>,
    },
    /// Duplicate elimination.
    Distinct(Box<Rel>),
    /// Stable ordering.
    Order {
        /// Input.
        input: Box<Rel>,
        /// Ordering keys.
        keys: Vec<OrderKey>,
    },
    /// Bounded result window.
    Limit {
        /// Input.
        input: Box<Rel>,
        /// Optional maximum rows.
        count: Option<u64>,
        /// Rows skipped.
        offset: u64,
    },
}

/// A uniqueness target for insert conflicts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictTarget {
    /// Primary key.
    PrimaryKey,
    /// Named unique constraint.
    UniqueConstraint(ConstraintName),
    /// Exact unique column sequence.
    Columns(Vec<ColumnName>),
}
/// Complete conflict behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConflictAction {
    /// Raise conflict.
    Fail,
    /// Ignore a matching row.
    DoNothing {
        /// Unique target.
        target: ConflictTarget,
    },
    /// Update a matching row.
    DoUpdate {
        /// Unique target.
        target: ConflictTarget,
        /// Assignments.
        assignments: Vec<(ColumnName, Scalar)>,
        /// Optional update predicate.
        predicate: Option<Scalar>,
    },
}
/// Provider-neutral data mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mutation {
    /// Insert rows produced by a relation.
    Insert {
        /// Target table.
        table: TableName,
        /// Populated columns in input order.
        columns: Vec<ColumnName>,
        /// Row-producing input.
        input: Box<Rel>,
        /// Conflict behavior.
        conflict: ConflictAction,
        /// Returned expressions.
        returning: Vec<NamedScalar>,
    },
    /// Update matching rows.
    Update {
        /// Target table.
        table: TableName,
        /// Target binding.
        bind: BindingName,
        /// Assignments.
        assignments: Vec<(ColumnName, Scalar)>,
        /// Optional filter.
        predicate: Option<Scalar>,
        /// Returned expressions.
        returning: Vec<NamedScalar>,
    },
    /// Delete matching rows.
    Delete {
        /// Target table.
        table: TableName,
        /// Target binding.
        bind: BindingName,
        /// Optional filter.
        predicate: Option<Scalar>,
        /// Returned expressions.
        returning: Vec<NamedScalar>,
    },
}

/// Admission resource limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionLimits {
    /// Maximum literal rows in one Values node.
    pub max_literal_rows: usize,
}
impl Default for AdmissionLimits {
    fn default() -> Self {
        Self {
            max_literal_rows: 1024,
        }
    }
}
/// Plan admission failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    /// Unknown table.
    UnknownTable(TableName),
    /// Unknown binding.
    UnresolvedBinding(BindingName),
    /// Binding shadows a live binding.
    AmbiguousBinding(BindingName),
    /// Unknown field.
    UnresolvedField(FieldRef),
    /// Parameter was not declared.
    UnresolvedParameter(ParameterName),
    /// Domain mismatch or unsupported operator.
    TypeError(&'static str),
    /// Aggregate appears outside aggregate position or grouping is invalid.
    IllegalAggregateScope,
    /// Set inputs disagree.
    IncompatibleSet,
    /// Scalar subquery does not return one field.
    ScalarQueryArity,
    /// Conflict target is not unique.
    UnsafeConflictTarget,
    /// Insert omits a required field.
    MissingRequiredInsertField(ColumnName),
    /// Literal rows exceed policy.
    LiteralRowLimit {
        /// Configured bound.
        limit: usize,
        /// Supplied rows.
        actual: usize,
    },
    /// Duplicate output or assignment name.
    DuplicateName,
}
impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for AdmissionError {}

/// Opaque admitted query. Its fields cannot be forged or changed by providers.
#[derive(Clone, Debug)]
pub struct CheckedQuery {
    pub(crate) schema_id: RelationId,
    pub(crate) catalog_id: RelationId,
    pub(crate) parameters: RowType,
    pub(crate) output: RowType,
    pub(crate) plan_id: RelationId,
    pub(crate) raw: Rel,
}
impl CheckedQuery {
    /// Schema identity used for admission.
    pub fn schema_id(&self) -> &RelationId {
        &self.schema_id
    }
    /// Domain catalog identity.
    pub fn catalog_id(&self) -> &RelationId {
        &self.catalog_id
    }
    /// Ordered parameter contract.
    pub fn parameters(&self) -> &RowType {
        &self.parameters
    }
    /// Output row contract.
    pub fn output(&self) -> &RowType {
        &self.output
    }
    /// Canonical plan identity.
    pub fn plan_id(&self) -> &RelationId {
        &self.plan_id
    }
    /// Read-only logical plan for codecs/providers.
    pub fn plan(&self) -> &Rel {
        &self.raw
    }
}
/// Opaque admitted mutation.
#[derive(Clone, Debug)]
pub struct CheckedMutation {
    pub(crate) schema_id: RelationId,
    pub(crate) catalog_id: RelationId,
    pub(crate) parameters: RowType,
    pub(crate) output: RowType,
    pub(crate) plan_id: RelationId,
    pub(crate) raw: Mutation,
}
impl CheckedMutation {
    /// Schema identity used for admission.
    pub fn schema_id(&self) -> &RelationId {
        &self.schema_id
    }
    /// Domain catalog identity.
    pub fn catalog_id(&self) -> &RelationId {
        &self.catalog_id
    }
    /// Parameter contract.
    pub fn parameters(&self) -> &RowType {
        &self.parameters
    }
    /// Returned row contract.
    pub fn output(&self) -> &RowType {
        &self.output
    }
    /// Canonical plan identity.
    pub fn plan_id(&self) -> &RelationId {
        &self.plan_id
    }
    /// Read-only mutation.
    pub fn plan(&self) -> &Mutation {
        &self.raw
    }
}
