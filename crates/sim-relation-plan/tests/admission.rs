use sim_kernel::{Datum, Symbol};
use sim_relation_core::*;
use sim_relation_plan::*;
use sim_relation_schema::*;

fn sym(v: &str) -> Symbol {
    Symbol::new(v)
}
fn field(v: &str) -> FieldName {
    FieldName::new(sym(v)).unwrap()
}
fn column(v: &str) -> ColumnName {
    ColumnName::new(sym(v)).unwrap()
}
fn table(v: &str) -> TableName {
    TableName::new(sym(v)).unwrap()
}
fn bind(v: &str) -> BindingName {
    BindingName::new(sym(v)).unwrap()
}
fn source() -> SourceName {
    SourceName::new(sym("main")).unwrap()
}
fn empty() -> RowType {
    RowType::new([]).unwrap()
}
fn int(value: &str) -> Scalar {
    Scalar::Literal(Cell::new(
        BaseDomain::I64.id(),
        Some(Datum::Number(sim_kernel::NumberLiteral {
            domain: Symbol::qualified("core", "i64"),
            canonical: value.into(),
        })),
    ))
}
fn f(binding: &str, name: &str) -> Scalar {
    Scalar::Field(FieldRef {
        binding: bind(binding),
        field: field(name),
    })
}
fn named(name: &str, scalar: Scalar) -> NamedScalar {
    NamedScalar {
        name: field(name),
        scalar,
    }
}
fn scan(name: &str, binding: &str) -> Rel {
    Rel::Scan {
        source: source(),
        table: table(name),
        bind: bind(binding),
    }
}

fn fixture() -> (DomainCatalog, Schema) {
    let domains = DomainCatalog::new([
        BaseDomain::Bool.spec(),
        BaseDomain::I64.spec(),
        BaseDomain::Text.spec(),
    ])
    .unwrap();
    let ledger = TableBuilder::new(table("ledger"))
        .column(ColumnBuilder::required(column("id"), BaseDomain::I64.id()).build())
        .column(ColumnBuilder::required(column("account"), BaseDomain::Text.id()).build())
        .column(ColumnBuilder::required(column("amount"), BaseDomain::I64.id()).build())
        .column(ColumnBuilder::nullable(column("parent"), BaseDomain::I64.id()).build())
        .constraint(Constraint::Primary(PrimaryKey {
            name: ConstraintName::new(sym("ledger_pk")).unwrap(),
            columns: vec![column("id")],
        }))
        .constraint(Constraint::Unique(UniqueConstraint {
            name: ConstraintName::new(sym("ledger_account_key")).unwrap(),
            columns: vec![column("account")],
        }))
        .build();
    let product = TableBuilder::new(table("product"))
        .column(ColumnBuilder::required(column("id"), BaseDomain::I64.id()).build())
        .column(ColumnBuilder::required(column("name"), BaseDomain::Text.id()).build())
        .constraint(Constraint::Primary(PrimaryKey {
            name: ConstraintName::new(sym("product_pk")).unwrap(),
            columns: vec![column("id")],
        }))
        .build();
    let schema = SchemaBuilder::new(SchemaName::new(sym("app")).unwrap())
        .table(ledger)
        .table(product)
        .build(&domains, &AcceptAllValues)
        .unwrap();
    (domains, schema)
}

#[test]
fn admits_union_group_having_conditional_and_self_join() {
    let (domains, schema) = fixture();
    let grouped = Rel::Group {
        input: Box::new(scan("ledger", "l")),
        bind: bind("g"),
        keys: vec![named("account", f("l", "account"))],
        aggregates: vec![NamedAggregate {
            name: field("total"),
            aggregate: Aggregate::Sum(f("l", "amount")),
        }],
        having: Some(Scalar::Call(ScalarOp::Gt, vec![f("g", "total"), int("0")])),
    };
    let conditional = Rel::Project {
        input: Box::new(grouped.clone()),
        bind: bind("trial"),
        fields: vec![named(
            "side",
            Scalar::Case {
                branches: vec![(
                    Scalar::Call(ScalarOp::Gt, vec![f("g", "total"), int("0")]),
                    Scalar::Literal(Cell::new(
                        BaseDomain::Text.id(),
                        Some(Datum::String("debit".into())),
                    )),
                )],
                otherwise: Some(Box::new(Scalar::Literal(Cell::new(
                    BaseDomain::Text.id(),
                    Some(Datum::String("credit".into())),
                )))),
            },
        )],
    };
    let union = Rel::Set {
        op: SetOp::UnionAll,
        inputs: vec![conditional.clone(), conditional],
    };
    let checked = admit_query(
        union,
        &schema,
        &domains,
        empty(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(checked.output().fields()[0].domain, BaseDomain::Text.id());

    let joined = Rel::Project {
        input: Box::new(Rel::Join {
            left: Box::new(scan("ledger", "child")),
            right: Box::new(scan("ledger", "parent")),
            kind: JoinKind::Left,
            on: Scalar::Call(ScalarOp::Eq, vec![f("child", "parent"), f("parent", "id")]),
        }),
        bind: bind("tree"),
        fields: vec![
            named("child", f("child", "id")),
            named("parent", f("parent", "id")),
        ],
    };
    let checked = admit_query(
        joined,
        &schema,
        &domains,
        empty(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert!(!checked.output().fields()[0].nullable);
    assert!(checked.output().fields()[1].nullable);
}

#[test]
fn admits_correlated_exists_in_and_scalar_subqueries() {
    let (domains, schema) = fixture();
    let one = |binding: &str| Rel::Project {
        input: Box::new(scan("ledger", binding)),
        bind: bind(&format!("{binding}_p")),
        fields: vec![named("id", f(binding, "id"))],
    };
    let query = Rel::Project {
        input: Box::new(scan("ledger", "outer")),
        bind: bind("result"),
        fields: vec![
            named(
                "exists",
                Scalar::Exists(Box::new(Rel::Filter {
                    input: Box::new(scan("ledger", "inner")),
                    predicate: Scalar::Call(
                        ScalarOp::Eq,
                        vec![f("inner", "parent"), f("outer", "id")],
                    ),
                })),
            ),
            named(
                "member",
                Scalar::InQuery {
                    value: Box::new(f("outer", "id")),
                    query: Box::new(one("membership")),
                },
            ),
            named(
                "single",
                Scalar::ScalarQuery(Box::new(Rel::Limit {
                    input: Box::new(one("scalar")),
                    count: Some(1),
                    offset: 0,
                })),
            ),
        ],
    };
    let checked = admit_query(
        query,
        &schema,
        &domains,
        empty(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(checked.output().fields().len(), 3);
}

#[test]
fn admits_complete_upsert_and_returned_values() {
    let (domains, schema) = fixture();
    let row_type = RowType::new([
        FieldType {
            name: field("id"),
            domain: BaseDomain::I64.id(),
            nullable: false,
        },
        FieldType {
            name: field("name"),
            domain: BaseDomain::Text.id(),
            nullable: false,
        },
    ])
    .unwrap();
    let row = Row::new(
        row_type.clone(),
        [
            Cell::new(
                BaseDomain::I64.id(),
                Some(Datum::Number(sim_kernel::NumberLiteral {
                    domain: Symbol::qualified("core", "i64"),
                    canonical: "1".into(),
                })),
            ),
            Cell::new(BaseDomain::Text.id(), Some(Datum::String("paper".into()))),
        ],
    )
    .unwrap();
    let mutation = Mutation::Insert {
        table: table("product"),
        columns: vec![column("id"), column("name")],
        input: Box::new(Rel::Values {
            bind: bind("new"),
            row_type,
            rows: vec![row],
        }),
        conflict: ConflictAction::DoUpdate {
            target: ConflictTarget::PrimaryKey,
            assignments: vec![(column("name"), f("excluded", "name"))],
            predicate: Some(Scalar::Call(
                ScalarOp::Ne,
                vec![f("target", "name"), f("excluded", "name")],
            )),
        },
        returning: vec![named("id", f("target", "id"))],
    };
    let checked = admit_mutation(
        mutation,
        &schema,
        &domains,
        empty(),
        AdmissionLimits::default(),
    )
    .unwrap();
    assert_eq!(checked.output().fields()[0].domain, BaseDomain::I64.id());
}

#[test]
fn rejects_unsafe_and_unbounded_plans() {
    let (domains, schema) = fixture();
    let ty = RowType::new([FieldType {
        name: field("x"),
        domain: BaseDomain::I64.id(),
        nullable: false,
    }])
    .unwrap();
    let row = Row::new(
        ty.clone(),
        [Cell::new(
            BaseDomain::I64.id(),
            Some(Datum::Number(sim_kernel::NumberLiteral {
                domain: Symbol::qualified("core", "i64"),
                canonical: "1".into(),
            })),
        )],
    )
    .unwrap();
    assert!(matches!(
        admit_query(
            Rel::Values {
                bind: bind("v"),
                row_type: ty,
                rows: vec![row]
            },
            &schema,
            &domains,
            empty(),
            AdmissionLimits {
                max_literal_rows: 0
            }
        ),
        Err(AdmissionError::LiteralRowLimit { .. })
    ));
    let unresolved = Rel::Project {
        input: Box::new(scan("ledger", "l")),
        bind: bind("p"),
        fields: vec![named("x", f("missing", "id"))],
    };
    assert!(matches!(
        admit_query(
            unresolved,
            &schema,
            &domains,
            empty(),
            AdmissionLimits::default()
        ),
        Err(AdmissionError::UnresolvedBinding(_))
    ));
}

#[test]
fn admits_update_delete_and_rejects_unsafe_conflict() {
    let (domains, schema) = fixture();
    let update = Mutation::Update {
        table: table("product"),
        bind: bind("p"),
        assignments: vec![(
            column("name"),
            Scalar::Literal(Cell::new(
                BaseDomain::Text.id(),
                Some(Datum::String("updated".into())),
            )),
        )],
        predicate: Some(Scalar::Call(ScalarOp::Eq, vec![f("p", "id"), int("1")])),
        returning: vec![named("name", f("p", "name"))],
    };
    assert_eq!(
        admit_mutation(
            update,
            &schema,
            &domains,
            empty(),
            AdmissionLimits::default()
        )
        .unwrap()
        .output()
        .fields()
        .len(),
        1
    );
    let delete = Mutation::Delete {
        table: table("product"),
        bind: bind("p"),
        predicate: None,
        returning: vec![named("id", f("p", "id"))],
    };
    assert!(
        admit_mutation(
            delete,
            &schema,
            &domains,
            empty(),
            AdmissionLimits::default()
        )
        .is_ok()
    );
    let ty = RowType::new([
        FieldType {
            name: field("id"),
            domain: BaseDomain::I64.id(),
            nullable: false,
        },
        FieldType {
            name: field("name"),
            domain: BaseDomain::Text.id(),
            nullable: false,
        },
    ])
    .unwrap();
    let unsafe_insert = Mutation::Insert {
        table: table("product"),
        columns: vec![column("id"), column("name")],
        input: Box::new(Rel::Values {
            bind: bind("new"),
            row_type: ty,
            rows: vec![],
        }),
        conflict: ConflictAction::DoNothing {
            target: ConflictTarget::Columns(vec![column("name")]),
        },
        returning: vec![],
    };
    assert!(matches!(
        admit_mutation(
            unsafe_insert,
            &schema,
            &domains,
            empty(),
            AdmissionLimits::default()
        ),
        Err(AdmissionError::UnsafeConflictTarget)
    ));
}
