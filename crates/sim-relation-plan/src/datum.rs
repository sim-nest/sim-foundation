use crate::model::*;
use sim_kernel::{Datum, Symbol};
use sim_relation_core::{ColumnName, RowType, ToRelationDatum};

pub(crate) struct QueryDatum<'a> {
    pub(crate) raw: &'a Rel,
    pub(crate) parameters: &'a RowType,
}
pub(crate) struct MutationDatum<'a>(pub(crate) &'a Mutation);
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
