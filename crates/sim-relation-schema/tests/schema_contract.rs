// conformance: validated logical schemas and normalized physical catalog records remain stable.

use sim_kernel::{Datum, Symbol};
use sim_relation_core::{BaseDomain, DomainCatalog, StorageRepr};
use sim_relation_schema::*;

fn n<T: TryFrom<Symbol>>(s: &str) -> T
where
    T::Error: std::fmt::Debug,
{
    T::try_from(Symbol::new(s)).unwrap()
}
fn domains() -> DomainCatalog {
    DomainCatalog::new([BaseDomain::I64.spec(), BaseDomain::Text.spec()]).unwrap()
}
fn column(name: &str, nullable: bool) -> Column {
    if nullable {
        ColumnBuilder::nullable(n(name), BaseDomain::I64.id())
    } else {
        ColumnBuilder::required(n(name), BaseDomain::I64.id())
    }
    .build()
}
fn table(name: &str) -> Table {
    TableBuilder::new(n(name))
        .column(column("id", false))
        .constraint(Constraint::Primary(PrimaryKey {
            name: n("pk"),
            columns: vec![n("id")],
        }))
        .build()
}

#[test]
fn exact_store_fixtures_are_distinct_and_canonical() {
    let v = AcceptAllValues;
    let values = [
        fixtures::document(&v).unwrap(),
        fixtures::gantt(&v).unwrap(),
        fixtures::ledger(&v).unwrap(),
        fixtures::relation_directory(&v).unwrap(),
    ];
    let ids: std::collections::BTreeSet<_> = values.iter().map(|v| v.id().unwrap()).collect();
    assert_eq!(ids.len(), 4);
    assert!(
        values
            .iter()
            .all(|v| v.tables()[0].name().symbol().name.as_ref() == "items")
    );
}
#[test]
fn refuses_duplicate_and_dangling_names() {
    let d = domains();
    let v = AcceptAllValues;
    assert!(matches!(
        Schema::new(n("s"), [table("t"), table("t")], [], &d, &v),
        Err(SchemaError::DuplicateTable(_))
    ));
    let bad = TableBuilder::new(n("t"))
        .column(column("id", false))
        .index(Index {
            name: n("i"),
            columns: vec![n("missing")],
            unique: false,
        })
        .build();
    assert!(matches!(
        Schema::new(n("s"), [bad], [], &d, &v),
        Err(SchemaError::DanglingColumn { .. })
    ));
}
#[test]
fn refuses_key_generation_default_and_shape_errors() {
    struct Reject;
    impl ValueShapeValidator for Reject {
        fn accepts(&self, _: &sim_relation_core::DomainId, _: &Datum) -> bool {
            false
        }
    }
    let d = domains();
    let nullable = TableBuilder::new(n("t"))
        .column(column("id", true))
        .constraint(Constraint::Primary(PrimaryKey {
            name: n("pk"),
            columns: vec![n("id")],
        }))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [nullable], [], &d, &AcceptAllValues),
        Err(SchemaError::NullablePrimaryKey(_))
    ));
    let both = TableBuilder::new(n("t"))
        .column(
            ColumnBuilder::required(n("id"), BaseDomain::I64.id())
                .default(Datum::Bool(true))
                .generated(GeneratedValue::new(Datum::Bool(true), []))
                .build(),
        )
        .build();
    assert!(matches!(
        Schema::new(n("s"), [both], [], &d, &AcceptAllValues),
        Err(SchemaError::DefaultAndGenerated(_))
    ));
    let default = TableBuilder::new(n("t"))
        .column(
            ColumnBuilder::required(n("id"), BaseDomain::I64.id())
                .default(Datum::Bool(true))
                .build(),
        )
        .build();
    assert!(matches!(
        Schema::new(n("s"), [default], [], &d, &Reject),
        Err(SchemaError::InvalidDefault(_))
    ));
}
#[test]
fn refuses_foreign_key_arity_domain_and_view_cycles() {
    let d = domains();
    let fk = TableBuilder::new(n("child"))
        .column(column("id", false))
        .constraint(Constraint::Foreign(ForeignKey {
            name: n("fk"),
            columns: vec![n("id")],
            target_table: n("parent"),
            target_columns: vec![],
        }))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [table("parent"), fk], [], &d, &AcceptAllValues),
        Err(SchemaError::ForeignKeyArity(_))
    ));
    let a = View {
        name: n("a"),
        query: Datum::Nil,
        table_dependencies: vec![],
        view_dependencies: vec![n("b")],
    };
    let b = View {
        name: n("b"),
        query: Datum::Nil,
        table_dependencies: vec![],
        view_dependencies: vec![n("a")],
    };
    assert!(matches!(
        Schema::new(n("s"), [], [a, b], &d, &AcceptAllValues),
        Err(SchemaError::ViewCycle(_))
    ));
}
#[test]
fn logical_identity_changes_for_every_meaningful_axis() {
    let d = domains();
    let v = AcceptAllValues;
    let base = Schema::new(n("s"), [table("t")], [], &d, &v).unwrap();
    let renamed = Schema::new(n("s2"), [table("t")], [], &d, &v).unwrap();
    let nullable = Schema::new(
        n("s"),
        [TableBuilder::new(n("t")).column(column("id", true)).build()],
        [],
        &d,
        &v,
    )
    .unwrap();
    assert_ne!(base.id().unwrap(), renamed.id().unwrap());
    assert_ne!(base.id().unwrap(), nullable.id().unwrap());
}
#[test]
fn physical_normalization_is_order_independent_but_evidence_sensitive() {
    let col = PhysicalColumn {
        name: n("id"),
        domain: BaseDomain::I64.id(),
        storage: StorageRepr::I64,
        nullable: false,
        ordinal: 0,
    };
    let t1 = PhysicalTable {
        name: n("z"),
        columns: vec![col.clone()],
        indexes: vec![],
    };
    let t2 = PhysicalTable {
        name: n("a"),
        columns: vec![col],
        indexes: vec![],
    };
    let p1 =
        PhysicalSchema::normalize(n("p"), n("s"), n("r1"), vec![t1.clone(), t2.clone()]).unwrap();
    let p2 = PhysicalSchema::normalize(n("p"), n("s"), n("r1"), vec![t2, t1]).unwrap();
    let p3 = PhysicalSchema::normalize(n("p"), n("s"), n("r2"), p2.tables().to_vec()).unwrap();
    assert_eq!(p1.id().unwrap(), p2.id().unwrap());
    assert_ne!(p1.id().unwrap(), p3.id().unwrap());
}

#[test]
fn every_named_graph_refusal_is_exercised() {
    let d = domains();
    let v = AcceptAllValues;
    let duplicate_column = TableBuilder::new(n("t"))
        .column(column("id", false))
        .column(column("id", false))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [duplicate_column], [], &d, &v),
        Err(SchemaError::DuplicateColumn { .. })
    ));
    let duplicate_constraint = TableBuilder::new(n("t"))
        .column(column("id", false))
        .constraint(Constraint::Unique(UniqueConstraint {
            name: n("u"),
            columns: vec![n("id")],
        }))
        .constraint(Constraint::Unique(UniqueConstraint {
            name: n("u"),
            columns: vec![n("id")],
        }))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [duplicate_constraint], [], &d, &v),
        Err(SchemaError::DuplicateConstraint { .. })
    ));
    let duplicate_index = TableBuilder::new(n("t"))
        .column(column("id", false))
        .index(Index {
            name: n("i"),
            columns: vec![n("id")],
            unique: false,
        })
        .index(Index {
            name: n("i"),
            columns: vec![n("id")],
            unique: true,
        })
        .build();
    assert!(matches!(
        Schema::new(n("s"), [duplicate_index], [], &d, &v),
        Err(SchemaError::DuplicateIndex { .. })
    ));
    let views = [
        View {
            name: n("v"),
            query: Datum::Nil,
            table_dependencies: vec![],
            view_dependencies: vec![],
        },
        View {
            name: n("v"),
            query: Datum::Nil,
            table_dependencies: vec![],
            view_dependencies: vec![],
        },
    ];
    assert!(matches!(
        Schema::new(n("s"), [], views, &d, &v),
        Err(SchemaError::DuplicateView(_))
    ));
    let dangling_domain = TableBuilder::new(n("t"))
        .column(ColumnBuilder::required(n("id"), n("unknown")).build())
        .build();
    assert!(matches!(
        Schema::new(n("s"), [dangling_domain], [], &d, &v),
        Err(SchemaError::DanglingDomain(_))
    ));
    let empty_key = TableBuilder::new(n("t"))
        .column(column("id", false))
        .constraint(Constraint::Unique(UniqueConstraint {
            name: n("u"),
            columns: vec![],
        }))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [empty_key], [], &d, &v),
        Err(SchemaError::EmptyKey(_))
    ));
    let generated_cycle = TableBuilder::new(n("t"))
        .column(
            ColumnBuilder::required(n("id"), BaseDomain::I64.id())
                .generated(GeneratedValue::new(Datum::Nil, [n("id")]))
                .build(),
        )
        .build();
    assert!(matches!(
        Schema::new(n("s"), [generated_cycle], [], &d, &v),
        Err(SchemaError::GeneratedCycle(_))
    ));
    struct Reject;
    impl ValueShapeValidator for Reject {
        fn accepts(&self, _: &sim_relation_core::DomainId, _: &Datum) -> bool {
            false
        }
    }
    let invalid_generated = TableBuilder::new(n("t"))
        .column(
            ColumnBuilder::required(n("id"), BaseDomain::I64.id())
                .generated(GeneratedValue::new(Datum::Nil, []))
                .build(),
        )
        .build();
    assert!(matches!(
        Schema::new(n("s"), [invalid_generated], [], &d, &Reject),
        Err(SchemaError::InvalidGenerated(_))
    ));
    let bad_check = TableBuilder::new(n("t"))
        .column(column("id", false))
        .constraint(Constraint::Check(CheckConstraint {
            name: n("c"),
            expression: Datum::Nil,
            columns: vec![n("outside")],
        }))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [bad_check], [], &d, &v),
        Err(SchemaError::DanglingColumn { .. })
    ));
    let dangling_table_view = View {
        name: n("v"),
        query: Datum::Nil,
        table_dependencies: vec![n("missing")],
        view_dependencies: vec![],
    };
    assert!(matches!(
        Schema::new(n("s"), [], [dangling_table_view], &d, &v),
        Err(SchemaError::DanglingTable(_))
    ));
    let dangling_view = View {
        name: n("v"),
        query: Datum::Nil,
        table_dependencies: vec![],
        view_dependencies: vec![n("missing")],
    };
    assert!(matches!(
        Schema::new(n("s"), [], [dangling_view], &d, &v),
        Err(SchemaError::DanglingView(_))
    ));
    let parent = TableBuilder::new(n("parent"))
        .column(ColumnBuilder::required(n("id"), BaseDomain::Text.id()).build())
        .build();
    let child = TableBuilder::new(n("child"))
        .column(column("id", false))
        .constraint(Constraint::Foreign(ForeignKey {
            name: n("fk"),
            columns: vec![n("id")],
            target_table: n("parent"),
            target_columns: vec![n("id")],
        }))
        .build();
    assert!(matches!(
        Schema::new(n("s"), [parent, child], [], &d, &v),
        Err(SchemaError::ForeignKeyDomain { .. })
    ));
}
