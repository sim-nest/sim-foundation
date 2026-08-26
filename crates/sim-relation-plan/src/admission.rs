use crate::{
    datum::{MutationDatum, QueryDatum},
    model::*,
};
use sim_kernel::Symbol;
use sim_relation_core::{
    BindingName, ColumnName, DomainCatalog, DomainId, DomainTrait, FieldName, FieldType,
    RelationId, RowType, TableName,
};
use sim_relation_schema::{Constraint, Schema, Table};
use std::collections::{BTreeMap, BTreeSet};

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
