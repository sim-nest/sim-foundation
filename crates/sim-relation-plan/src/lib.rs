//! Complete provider-neutral relational plans with a sealed admission boundary.
//!
//! Providers receive [`CheckedQuery`] or [`CheckedMutation`], never SQL text or
//! unchecked syntax. The public algebra binds every field explicitly, including
//! correlated subqueries and the `excluded` row of conflict updates.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use sim_kernel::{Datum, Symbol};
use sim_relation_core::{
    BindingName, Cell, ColumnName, ConstraintName, DomainCatalog, DomainId, DomainTrait, FieldName,
    FieldType, ParameterName, RelationId, Row, RowType, SourceName, TableName, ToRelationDatum,
};
use sim_relation_schema::{Constraint, Schema, Table};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

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
    schema_id: RelationId,
    catalog_id: RelationId,
    parameters: RowType,
    output: RowType,
    plan_id: RelationId,
    raw: Rel,
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
    schema_id: RelationId,
    catalog_id: RelationId,
    parameters: RowType,
    output: RowType,
    plan_id: RelationId,
    raw: Mutation,
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

type Scope = BTreeMap<BindingName, RowType>;
struct Admit<'a> {
    schema: &'a Schema,
    domains: &'a DomainCatalog,
    params: &'a RowType,
    limits: AdmissionLimits,
}
impl<'a> Admit<'a> {
    fn table(&self, name: &TableName) -> Result<&'a Table, AdmissionError> {
        self.schema
            .tables()
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| AdmissionError::UnknownTable(name.clone()))
    }
    fn table_row(&self, table: &Table, nullable: bool) -> Result<RowType, AdmissionError> {
        RowType::new(table.columns().iter().map(|c| FieldType {
            name: FieldName::new(c.name().symbol().clone()).expect("validated"),
            domain: c.domain().clone(),
            nullable: nullable || c.nullable(),
        }))
        .map_err(|_| AdmissionError::DuplicateName)
    }
    fn bind(scope: &mut Scope, name: BindingName, row: RowType) -> Result<(), AdmissionError> {
        if scope.insert(name.clone(), row).is_some() {
            Err(AdmissionError::AmbiguousBinding(name))
        } else {
            Ok(())
        }
    }
    fn visible(&self, rel: &Rel, row: &RowType, scope: &mut Scope) -> Result<(), AdmissionError> {
        match rel {
            Rel::Scan { table, bind, .. } => Self::bind(
                scope,
                bind.clone(),
                self.table_row(self.table(table)?, false)?,
            ),
            Rel::Values { bind, row_type, .. } => Self::bind(scope, bind.clone(), row_type.clone()),
            Rel::Project { bind, .. } | Rel::Group { bind, .. } => {
                Self::bind(scope, bind.clone(), row.clone())
            }
            Rel::Join {
                left, right, kind, ..
            } => {
                let l = self.rel(left, scope)?;
                self.visible(left, &l, scope)?;
                let left_names: BTreeSet<_> = scope.keys().cloned().collect();
                let r = self.rel(right, scope)?;
                self.visible(right, &r, scope)?;
                if *kind == JoinKind::Left {
                    for (name, row) in scope.iter_mut() {
                        if !left_names.contains(name) {
                            *row = RowType::new(row.fields().iter().cloned().map(|mut field| {
                                field.nullable = true;
                                field
                            }))
                            .expect("changing nullability preserves field names");
                        }
                    }
                }
                Ok(())
            }
            Rel::Filter { input, .. }
            | Rel::Distinct(input)
            | Rel::Order { input, .. }
            | Rel::Limit { input, .. } => self.visible(input, row, scope),
            Rel::Set { .. } => Ok(()),
        }
    }
    fn scalar(&self, value: &Scalar, scope: &Scope) -> Result<FieldType, AdmissionError> {
        let bool_ty = || FieldType {
            name: fname("value"),
            domain: sim_relation_core::BaseDomain::Bool.id(),
            nullable: false,
        };
        match value {
            Scalar::Field(r) => scope
                .get(&r.binding)
                .ok_or_else(|| AdmissionError::UnresolvedBinding(r.binding.clone()))?
                .fields()
                .iter()
                .find(|f| f.name == r.field)
                .cloned()
                .ok_or_else(|| AdmissionError::UnresolvedField(r.clone())),
            Scalar::Literal(c) => Ok(FieldType {
                name: fname("value"),
                domain: c.domain().clone(),
                nullable: c.value().is_none(),
            }),
            Scalar::Param(p) => self
                .params
                .fields()
                .iter()
                .find(|f| f.name.symbol() == p.symbol())
                .cloned()
                .ok_or_else(|| AdmissionError::UnresolvedParameter(p.clone())),
            Scalar::Exists(q) => {
                self.rel(q, scope)?;
                Ok(bool_ty())
            }
            Scalar::InQuery { value, query } => {
                let l = self.scalar(value, scope)?;
                let q = self.rel(query, scope)?;
                if q.fields().len() != 1 {
                    return Err(AdmissionError::ScalarQueryArity);
                }
                compatible(&l, &q.fields()[0])?;
                Ok(bool_ty())
            }
            Scalar::ScalarQuery(q) => {
                let row = self.rel(q, scope)?;
                if row.fields().len() != 1 {
                    return Err(AdmissionError::ScalarQueryArity);
                }
                Ok(row.fields()[0].clone())
            }
            Scalar::Case {
                branches,
                otherwise,
            } => {
                let mut out = None;
                let mut nullable = otherwise.is_none();
                for (p, v) in branches {
                    require_bool(&self.scalar(p, scope)?)?;
                    let t = self.scalar(v, scope)?;
                    if let Some(old) = &out {
                        compatible(old, &t)?;
                    }
                    nullable |= t.nullable;
                    out = Some(t);
                }
                if let Some(v) = otherwise {
                    let t = self.scalar(v, scope)?;
                    if let Some(old) = &out {
                        compatible(old, &t)?;
                    }
                    nullable |= t.nullable;
                    out = Some(t);
                }
                let mut out = out.ok_or(AdmissionError::TypeError("empty case"))?;
                out.nullable = nullable;
                Ok(out)
            }
            Scalar::Call(op, args) => self.call(*op, args, scope),
        }
    }
    fn call(
        &self,
        op: ScalarOp,
        args: &[Scalar],
        scope: &Scope,
    ) -> Result<FieldType, AdmissionError> {
        let ts: Vec<_> = args
            .iter()
            .map(|v| self.scalar(v, scope))
            .collect::<Result<_, _>>()?;
        let arity = |n| {
            if ts.len() == n {
                Ok(())
            } else {
                Err(AdmissionError::TypeError("operator arity"))
            }
        };
        match op {
            ScalarOp::Not | ScalarOp::IsNull => {
                arity(1)?;
                if op == ScalarOp::Not {
                    require_bool(&ts[0])?;
                }
                Ok(FieldType {
                    name: fname("value"),
                    domain: sim_relation_core::BaseDomain::Bool.id(),
                    nullable: false,
                })
            }
            ScalarOp::And | ScalarOp::Or => {
                if ts.len() < 2 {
                    return Err(AdmissionError::TypeError("operator arity"));
                }
                for t in &ts {
                    require_bool(t)?;
                }
                Ok(FieldType {
                    name: fname("value"),
                    domain: sim_relation_core::BaseDomain::Bool.id(),
                    nullable: ts.iter().any(|t| t.nullable),
                })
            }
            ScalarOp::Eq
            | ScalarOp::Ne
            | ScalarOp::Lt
            | ScalarOp::Le
            | ScalarOp::Gt
            | ScalarOp::Ge => {
                arity(2)?;
                compatible(&ts[0], &ts[1])?;
                let needed = if matches!(op, ScalarOp::Eq | ScalarOp::Ne) {
                    DomainTrait::Equatable
                } else {
                    DomainTrait::Ordered
                };
                require_trait(self.domains, &ts[0].domain, needed)?;
                Ok(FieldType {
                    name: fname("value"),
                    domain: sim_relation_core::BaseDomain::Bool.id(),
                    nullable: ts.iter().any(|t| t.nullable),
                })
            }
            ScalarOp::Add | ScalarOp::Sub | ScalarOp::Mul | ScalarOp::Div => {
                arity(2)?;
                compatible(&ts[0], &ts[1])?;
                require_trait(self.domains, &ts[0].domain, DomainTrait::Ordered)?;
                let mut t = ts[0].clone();
                t.nullable = ts.iter().any(|t| t.nullable);
                Ok(t)
            }
            ScalarOp::Coalesce => {
                if ts.is_empty() {
                    return Err(AdmissionError::TypeError("empty coalesce"));
                }
                for t in &ts[1..] {
                    compatible(&ts[0], t)?;
                }
                let mut t = ts[0].clone();
                t.nullable = ts.iter().all(|t| t.nullable);
                Ok(t)
            }
        }
    }
    fn aggregate(&self, a: &Aggregate, scope: &Scope) -> Result<FieldType, AdmissionError> {
        match a {
            Aggregate::CountAll | Aggregate::Count(_) => {
                if let Aggregate::Count(v) = a {
                    self.scalar(v, scope)?;
                }
                Ok(FieldType {
                    name: fname("value"),
                    domain: sim_relation_core::BaseDomain::I64.id(),
                    nullable: false,
                })
            }
            Aggregate::Sum(v) | Aggregate::Min(v) | Aggregate::Max(v) => {
                let mut t = self.scalar(v, scope)?;
                require_trait(self.domains, &t.domain, DomainTrait::Ordered)?;
                t.nullable = true;
                Ok(t)
            }
        }
    }
    fn rel(&self, rel: &Rel, outer: &Scope) -> Result<RowType, AdmissionError> {
        match rel {
            Rel::Scan { table, bind, .. } => {
                let row = self.table_row(self.table(table)?, false)?;
                let mut s = outer.clone();
                Self::bind(&mut s, bind.clone(), row.clone())?;
                Ok(row)
            }
            Rel::Values {
                bind,
                row_type,
                rows,
            } => {
                if rows.len() > self.limits.max_literal_rows {
                    return Err(AdmissionError::LiteralRowLimit {
                        limit: self.limits.max_literal_rows,
                        actual: rows.len(),
                    });
                }
                if rows.iter().any(|r| r.row_type() != row_type) {
                    return Err(AdmissionError::TypeError("values row type"));
                }
                let mut s = outer.clone();
                Self::bind(&mut s, bind.clone(), row_type.clone())?;
                Ok(row_type.clone())
            }
            Rel::Project {
                input,
                bind,
                fields,
            } => {
                let input_ty = self.rel(input, outer)?;
                let mut s = outer.clone();
                self.visible(input, &input_ty, &mut s)?;
                let row = named_row(fields.iter().map(|n| (&n.name, &n.scalar)), |v| {
                    self.scalar(v, &s)
                })?;
                Self::bind(&mut s, bind.clone(), row.clone())?;
                Ok(row)
            }
            Rel::Filter { input, predicate } => {
                let row = self.rel(input, outer)?;
                let mut s = outer.clone();
                self.visible(input, &row, &mut s)?;
                require_bool(&self.scalar(predicate, &s)?)?;
                Ok(row)
            }
            Rel::Join {
                left,
                right,
                kind,
                on,
            } => {
                let l = self.rel(left, outer)?;
                let mut s = outer.clone();
                self.visible(left, &l, &mut s)?;
                let r = self.rel(right, &s)?;
                self.visible(right, &r, &mut s)?;
                if *kind != JoinKind::Cross {
                    require_bool(&self.scalar(on, &s)?)?;
                }
                let mut fields = Vec::new();
                for (binding, row) in &s {
                    if outer.contains_key(binding) {
                        continue;
                    }
                    for f in row.fields() {
                        let mut f = f.clone();
                        f.name = FieldName::new(Symbol::qualified(
                            binding.symbol().name.clone(),
                            f.name.symbol().name.clone(),
                        ))
                        .expect("qualified");
                        if *kind == JoinKind::Left && !scope_contains(left, binding) {
                            f.nullable = true;
                        }
                        fields.push(f);
                    }
                }
                RowType::new(fields).map_err(|_| AdmissionError::DuplicateName)
            }
            Rel::Group {
                input,
                bind,
                keys,
                aggregates,
                having,
            } => {
                let inrow = self.rel(input, outer)?;
                let mut s = outer.clone();
                self.visible(input, &inrow, &mut s)?;
                let mut fields = Vec::new();
                for n in keys {
                    let mut t = self.scalar(&n.scalar, &s)?;
                    t.name = n.name.clone();
                    fields.push(t);
                }
                for n in aggregates {
                    let mut t = self.aggregate(&n.aggregate, &s)?;
                    t.name = n.name.clone();
                    fields.push(t);
                }
                let row = RowType::new(fields).map_err(|_| AdmissionError::DuplicateName)?;
                let mut hs = outer.clone();
                Self::bind(&mut hs, bind.clone(), row.clone())?;
                if let Some(v) = having {
                    require_bool(&self.scalar(v, &hs)?)?;
                }
                Ok(row)
            }
            Rel::Set { inputs, .. } => {
                if inputs.len() < 2 {
                    return Err(AdmissionError::IncompatibleSet);
                }
                let first = self.rel(&inputs[0], outer)?;
                for v in &inputs[1..] {
                    let r = self.rel(v, outer)?;
                    if r.fields().len() != first.fields().len() {
                        return Err(AdmissionError::IncompatibleSet);
                    }
                    for (a, b) in first.fields().iter().zip(r.fields()) {
                        compatible(a, b).map_err(|_| AdmissionError::IncompatibleSet)?;
                    }
                }
                Ok(first)
            }
            Rel::Distinct(v) => self.rel(v, outer),
            Rel::Order { input, keys } => {
                let row = self.rel(input, outer)?;
                let mut s = outer.clone();
                self.visible(input, &row, &mut s)?;
                for k in keys {
                    let t = self.scalar(&k.scalar, &s)?;
                    require_trait(self.domains, &t.domain, DomainTrait::Ordered)?;
                }
                Ok(row)
            }
            Rel::Limit { input, .. } => self.rel(input, outer),
        }
    }
}

/// Admits a complete query against immutable schema, catalog, and parameter contracts.
pub fn admit_query(
    raw: Rel,
    schema: &Schema,
    domains: &DomainCatalog,
    parameters: RowType,
    limits: AdmissionLimits,
) -> Result<CheckedQuery, AdmissionError> {
    let a = Admit {
        schema,
        domains,
        params: &parameters,
        limits,
    };
    let output = a.rel(&raw, &Scope::new())?;
    let schema_id = schema
        .id()
        .map_err(|_| AdmissionError::TypeError("schema identity"))?;
    let catalog_id =
        RelationId::of(domains).map_err(|_| AdmissionError::TypeError("catalog identity"))?;
    let plan_id = RelationId::of(&QueryDatum {
        raw: &raw,
        parameters: &parameters,
    })
    .map_err(|_| AdmissionError::TypeError("plan identity"))?;
    Ok(CheckedQuery {
        schema_id,
        catalog_id,
        parameters,
        output,
        plan_id,
        raw,
    })
}
/// Admits a complete insert, update, or delete plan.
pub fn admit_mutation(
    raw: Mutation,
    schema: &Schema,
    domains: &DomainCatalog,
    parameters: RowType,
    limits: AdmissionLimits,
) -> Result<CheckedMutation, AdmissionError> {
    let a = Admit {
        schema,
        domains,
        params: &parameters,
        limits,
    };
    let mut scope = Scope::new();
    let output = match &raw {
        Mutation::Insert {
            table,
            columns,
            input,
            conflict,
            returning,
        } => {
            let t = a.table(table)?;
            let input_ty = a.rel(input, &scope)?;
            if columns.len() != input_ty.fields().len() {
                return Err(AdmissionError::TypeError("insert arity"));
            }
            let mut seen = BTreeSet::new();
            for (c, f) in columns.iter().zip(input_ty.fields()) {
                if !seen.insert(c) {
                    return Err(AdmissionError::DuplicateName);
                }
                let target = t
                    .columns()
                    .iter()
                    .find(|x| x.name() == c)
                    .ok_or(AdmissionError::TypeError("unknown insert column"))?;
                compatible(
                    &FieldType {
                        name: fname("target"),
                        domain: target.domain().clone(),
                        nullable: target.nullable(),
                    },
                    f,
                )?;
            }
            for c in t.columns() {
                if !c.nullable()
                    && !c.has_default()
                    && !c.is_generated()
                    && !columns.contains(c.name())
                {
                    return Err(AdmissionError::MissingRequiredInsertField(c.name().clone()));
                }
            }
            validate_conflict(conflict, t, &a, &mut scope)?;
            if !scope.contains_key(&bname("target")) {
                let target_row = a.table_row(t, false)?;
                Admit::bind(&mut scope, bname("target"), target_row)?;
            }
            named_row(returning.iter().map(|n| (&n.name, &n.scalar)), |v| {
                a.scalar(v, &scope)
            })?
        }
        Mutation::Update {
            table,
            bind,
            assignments,
            predicate,
            returning,
        } => {
            let t = a.table(table)?;
            Admit::bind(&mut scope, bind.clone(), a.table_row(t, false)?)?;
            validate_assignments(assignments, t, &a, &scope)?;
            if let Some(v) = predicate {
                require_bool(&a.scalar(v, &scope)?)?;
            }
            named_row(returning.iter().map(|n| (&n.name, &n.scalar)), |v| {
                a.scalar(v, &scope)
            })?
        }
        Mutation::Delete {
            table,
            bind,
            predicate,
            returning,
        } => {
            let t = a.table(table)?;
            Admit::bind(&mut scope, bind.clone(), a.table_row(t, false)?)?;
            if let Some(v) = predicate {
                require_bool(&a.scalar(v, &scope)?)?;
            }
            named_row(returning.iter().map(|n| (&n.name, &n.scalar)), |v| {
                a.scalar(v, &scope)
            })?
        }
    };
    let schema_id = schema
        .id()
        .map_err(|_| AdmissionError::TypeError("schema identity"))?;
    let catalog_id =
        RelationId::of(domains).map_err(|_| AdmissionError::TypeError("catalog identity"))?;
    let plan_id = RelationId::of(&MutationDatum(&raw))
        .map_err(|_| AdmissionError::TypeError("plan identity"))?;
    Ok(CheckedMutation {
        schema_id,
        catalog_id,
        parameters,
        output,
        plan_id,
        raw,
    })
}

fn validate_assignments(
    v: &[(ColumnName, Scalar)],
    t: &Table,
    a: &Admit<'_>,
    s: &Scope,
) -> Result<(), AdmissionError> {
    let mut seen = BTreeSet::new();
    for (c, x) in v {
        if !seen.insert(c) {
            return Err(AdmissionError::DuplicateName);
        }
        let col = t
            .columns()
            .iter()
            .find(|z| z.name() == c)
            .ok_or(AdmissionError::TypeError("unknown assignment column"))?;
        compatible(
            &FieldType {
                name: fname("target"),
                domain: col.domain().clone(),
                nullable: col.nullable(),
            },
            &a.scalar(x, s)?,
        )?;
    }
    Ok(())
}
fn validate_conflict(
    c: &ConflictAction,
    t: &Table,
    a: &Admit<'_>,
    s: &mut Scope,
) -> Result<(), AdmissionError> {
    let (target, updates, pred) = match c {
        ConflictAction::Fail => return Ok(()),
        ConflictAction::DoNothing { target } => (target, None, None),
        ConflictAction::DoUpdate {
            target,
            assignments,
            predicate,
        } => (target, Some(assignments), predicate.as_ref()),
    };
    let valid = match target {
        ConflictTarget::PrimaryKey => t
            .constraints()
            .iter()
            .any(|c| matches!(c, Constraint::Primary(_))),
        ConflictTarget::UniqueConstraint(n) => t
            .constraints()
            .iter()
            .any(|c| matches!(c,Constraint::Unique(v)if &v.name==n)),
        ConflictTarget::Columns(cols) => t.constraints().iter().any(|c| match c {
            Constraint::Primary(v) => v.columns == *cols,
            Constraint::Unique(v) => v.columns == *cols,
            _ => false,
        }),
    };
    if !valid {
        return Err(AdmissionError::UnsafeConflictTarget);
    }
    if let Some(v) = updates {
        Admit::bind(s, bname("target"), a.table_row(t, false)?)?;
        Admit::bind(s, bname("excluded"), a.table_row(t, false)?)?;
        validate_assignments(v, t, a, s)?;
        if let Some(p) = pred {
            require_bool(&a.scalar(p, s)?)?;
        }
    }
    Ok(())
}
fn named_row<'a, T: 'a>(
    values: impl Iterator<Item = (&'a FieldName, &'a T)>,
    mut check: impl FnMut(&T) -> Result<FieldType, AdmissionError>,
) -> Result<RowType, AdmissionError> {
    let mut out = Vec::new();
    for (n, v) in values {
        let mut t = check(v)?;
        t.name = n.clone();
        out.push(t);
    }
    RowType::new(out).map_err(|_| AdmissionError::DuplicateName)
}
fn compatible(a: &FieldType, b: &FieldType) -> Result<(), AdmissionError> {
    if a.domain == b.domain {
        Ok(())
    } else {
        Err(AdmissionError::TypeError("domain mismatch"))
    }
}
fn require_bool(v: &FieldType) -> Result<(), AdmissionError> {
    if v.domain == sim_relation_core::BaseDomain::Bool.id() {
        Ok(())
    } else {
        Err(AdmissionError::TypeError("boolean required"))
    }
}
fn require_trait(c: &DomainCatalog, d: &DomainId, t: DomainTrait) -> Result<(), AdmissionError> {
    if c.get(d).is_some_and(|x| x.traits().contains(&t)) {
        Ok(())
    } else {
        Err(AdmissionError::TypeError("domain trait missing"))
    }
}
fn scope_contains(rel: &Rel, binding: &BindingName) -> bool {
    match rel {
        Rel::Scan { bind, .. }
        | Rel::Values { bind, .. }
        | Rel::Project { bind, .. }
        | Rel::Group { bind, .. } => bind == binding,
        Rel::Join { left, right, .. } => {
            scope_contains(left, binding) || scope_contains(right, binding)
        }
        Rel::Filter { input, .. }
        | Rel::Distinct(input)
        | Rel::Order { input, .. }
        | Rel::Limit { input, .. } => scope_contains(input, binding),
        Rel::Set { .. } => false,
    }
}
fn fname(v: &str) -> FieldName {
    FieldName::new(Symbol::new(v)).expect("literal")
}
fn bname(v: &str) -> BindingName {
    BindingName::new(Symbol::new(v)).expect("literal")
}

struct QueryDatum<'a> {
    raw: &'a Rel,
    parameters: &'a RowType,
}
struct MutationDatum<'a>(&'a Mutation);
impl ToRelationDatum for QueryDatum<'_> {
    fn to_datum(&self) -> Datum {
        node(
            "query",
            vec![
                ("parameters", self.parameters.to_datum()),
                ("plan", rel_datum(self.raw)),
            ],
        )
    }
}
impl ToRelationDatum for MutationDatum<'_> {
    fn to_datum(&self) -> Datum {
        mutation_datum(self.0)
    }
}
fn rel_datum(v: &Rel) -> Datum {
    match v {
        Rel::Scan {
            source,
            table,
            bind,
        } => node(
            "scan",
            vec![
                ("source", symbol(source.symbol())),
                ("table", symbol(table.symbol())),
                ("bind", symbol(bind.symbol())),
            ],
        ),
        Rel::Values {
            bind,
            row_type,
            rows,
        } => node(
            "values",
            vec![
                ("bind", symbol(bind.symbol())),
                ("row-type", row_type.to_datum()),
                ("rows", list(rows.iter().map(ToRelationDatum::to_datum))),
            ],
        ),
        Rel::Project {
            input,
            bind,
            fields,
        } => node(
            "project",
            vec![
                ("input", rel_datum(input)),
                ("bind", symbol(bind.symbol())),
                ("fields", list(fields.iter().map(named_datum))),
            ],
        ),
        Rel::Filter { input, predicate } => node(
            "filter",
            vec![
                ("input", rel_datum(input)),
                ("predicate", scalar_datum(predicate)),
            ],
        ),
        Rel::Join {
            left,
            right,
            kind,
            on,
        } => node(
            "join",
            vec![
                ("left", rel_datum(left)),
                ("right", rel_datum(right)),
                (
                    "kind",
                    word(match kind {
                        JoinKind::Inner => "inner",
                        JoinKind::Left => "left",
                        JoinKind::Cross => "cross",
                    }),
                ),
                ("on", scalar_datum(on)),
            ],
        ),
        Rel::Group {
            input,
            bind,
            keys,
            aggregates,
            having,
        } => node(
            "group",
            vec![
                ("input", rel_datum(input)),
                ("bind", symbol(bind.symbol())),
                ("keys", list(keys.iter().map(named_datum))),
                (
                    "aggregates",
                    list(aggregates.iter().map(|v| {
                        node(
                            "named-aggregate",
                            vec![
                                ("name", symbol(v.name.symbol())),
                                ("aggregate", aggregate_datum(&v.aggregate)),
                            ],
                        )
                    })),
                ),
                ("having", option(having.as_ref().map(scalar_datum))),
            ],
        ),
        Rel::Set { op, inputs } => node(
            "set",
            vec![
                (
                    "op",
                    word(match op {
                        SetOp::Union => "union",
                        SetOp::UnionAll => "union-all",
                        SetOp::Intersect => "intersect",
                        SetOp::Except => "except",
                    }),
                ),
                ("inputs", list(inputs.iter().map(rel_datum))),
            ],
        ),
        Rel::Distinct(input) => node("distinct", vec![("input", rel_datum(input))]),
        Rel::Order { input, keys } => node(
            "order",
            vec![
                ("input", rel_datum(input)),
                (
                    "keys",
                    list(keys.iter().map(|v| {
                        node(
                            "order-key",
                            vec![
                                ("scalar", scalar_datum(&v.scalar)),
                                (
                                    "direction",
                                    word(match v.direction {
                                        OrderDirection::Asc => "asc",
                                        OrderDirection::Desc => "desc",
                                    }),
                                ),
                            ],
                        )
                    })),
                ),
            ],
        ),
        Rel::Limit {
            input,
            count,
            offset,
        } => node(
            "limit",
            vec![
                ("input", rel_datum(input)),
                ("count", option(count.map(number))),
                ("offset", number(*offset)),
            ],
        ),
    }
}
fn scalar_datum(v: &Scalar) -> Datum {
    match v {
        Scalar::Field(v) => node(
            "field",
            vec![
                ("binding", symbol(v.binding.symbol())),
                ("field", symbol(v.field.symbol())),
            ],
        ),
        Scalar::Literal(v) => node("literal", vec![("cell", v.to_datum())]),
        Scalar::Param(v) => node("parameter", vec![("name", symbol(v.symbol()))]),
        Scalar::Call(op, args) => node(
            "call",
            vec![
                (
                    "operator",
                    word(match op {
                        ScalarOp::And => "and",
                        ScalarOp::Or => "or",
                        ScalarOp::Not => "not",
                        ScalarOp::Eq => "eq",
                        ScalarOp::Ne => "ne",
                        ScalarOp::Lt => "lt",
                        ScalarOp::Le => "le",
                        ScalarOp::Gt => "gt",
                        ScalarOp::Ge => "ge",
                        ScalarOp::Add => "add",
                        ScalarOp::Sub => "sub",
                        ScalarOp::Mul => "mul",
                        ScalarOp::Div => "div",
                        ScalarOp::IsNull => "is-null",
                        ScalarOp::Coalesce => "coalesce",
                    }),
                ),
                ("arguments", list(args.iter().map(scalar_datum))),
            ],
        ),
        Scalar::Case {
            branches,
            otherwise,
        } => node(
            "case",
            vec![
                (
                    "branches",
                    list(branches.iter().map(|(p, v)| {
                        node(
                            "branch",
                            vec![("when", scalar_datum(p)), ("then", scalar_datum(v))],
                        )
                    })),
                ),
                ("otherwise", option(otherwise.as_deref().map(scalar_datum))),
            ],
        ),
        Scalar::Exists(v) => node("exists", vec![("query", rel_datum(v))]),
        Scalar::InQuery { value, query } => node(
            "in-query",
            vec![("value", scalar_datum(value)), ("query", rel_datum(query))],
        ),
        Scalar::ScalarQuery(v) => node("scalar-query", vec![("query", rel_datum(v))]),
    }
}
fn aggregate_datum(v: &Aggregate) -> Datum {
    match v {
        Aggregate::CountAll => node("count-all", vec![]),
        Aggregate::Count(v) => node("count", vec![("value", scalar_datum(v))]),
        Aggregate::Sum(v) => node("sum", vec![("value", scalar_datum(v))]),
        Aggregate::Min(v) => node("min", vec![("value", scalar_datum(v))]),
        Aggregate::Max(v) => node("max", vec![("value", scalar_datum(v))]),
    }
}
fn named_datum(v: &NamedScalar) -> Datum {
    node(
        "named-scalar",
        vec![
            ("name", symbol(v.name.symbol())),
            ("scalar", scalar_datum(&v.scalar)),
        ],
    )
}
fn target_datum(v: &ConflictTarget) -> Datum {
    match v {
        ConflictTarget::PrimaryKey => node("primary-key", vec![]),
        ConflictTarget::UniqueConstraint(n) => {
            node("unique-constraint", vec![("name", symbol(n.symbol()))])
        }
        ConflictTarget::Columns(v) => node(
            "unique-columns",
            vec![("columns", list(v.iter().map(|n| symbol(n.symbol()))))],
        ),
    }
}
fn conflict_datum(v: &ConflictAction) -> Datum {
    match v {
        ConflictAction::Fail => node("fail", vec![]),
        ConflictAction::DoNothing { target } => {
            node("do-nothing", vec![("target", target_datum(target))])
        }
        ConflictAction::DoUpdate {
            target,
            assignments,
            predicate,
        } => node(
            "do-update",
            vec![
                ("target", target_datum(target)),
                ("assignments", assignments_datum(assignments)),
                ("predicate", option(predicate.as_ref().map(scalar_datum))),
            ],
        ),
    }
}
fn assignments_datum(v: &[(ColumnName, Scalar)]) -> Datum {
    list(v.iter().map(|(n, v)| {
        node(
            "assignment",
            vec![("column", symbol(n.symbol())), ("value", scalar_datum(v))],
        )
    }))
}
fn mutation_datum(v: &Mutation) -> Datum {
    match v {
        Mutation::Insert {
            table,
            columns,
            input,
            conflict,
            returning,
        } => node(
            "insert",
            vec![
                ("table", symbol(table.symbol())),
                ("columns", list(columns.iter().map(|n| symbol(n.symbol())))),
                ("input", rel_datum(input)),
                ("conflict", conflict_datum(conflict)),
                ("returning", list(returning.iter().map(named_datum))),
            ],
        ),
        Mutation::Update {
            table,
            bind,
            assignments,
            predicate,
            returning,
        } => node(
            "update",
            vec![
                ("table", symbol(table.symbol())),
                ("bind", symbol(bind.symbol())),
                ("assignments", assignments_datum(assignments)),
                ("predicate", option(predicate.as_ref().map(scalar_datum))),
                ("returning", list(returning.iter().map(named_datum))),
            ],
        ),
        Mutation::Delete {
            table,
            bind,
            predicate,
            returning,
        } => node(
            "delete",
            vec![
                ("table", symbol(table.symbol())),
                ("bind", symbol(bind.symbol())),
                ("predicate", option(predicate.as_ref().map(scalar_datum))),
                ("returning", list(returning.iter().map(named_datum))),
            ],
        ),
    }
}
fn symbol(v: &Symbol) -> Datum {
    Datum::Symbol(v.clone())
}
fn word(v: &str) -> Datum {
    Datum::Symbol(Symbol::new(v))
}
fn list(v: impl IntoIterator<Item = Datum>) -> Datum {
    Datum::Vector(v.into_iter().collect())
}
fn option(v: Option<Datum>) -> Datum {
    v.unwrap_or(Datum::Nil)
}
fn number(v: u64) -> Datum {
    Datum::Number(sim_kernel::NumberLiteral {
        domain: Symbol::qualified("core", "u64"),
        canonical: v.to_string(),
    })
}
fn node(tag: &str, fields: Vec<(&str, Datum)>) -> Datum {
    Datum::Node {
        tag: Symbol::qualified("relation-plan", tag),
        fields: fields
            .into_iter()
            .map(|(k, v)| (Symbol::new(k), v))
            .collect(),
    }
}
