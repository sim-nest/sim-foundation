use sim_kernel::Symbol;
use sim_relation_core::*;
use sim_relation_migrate::*;
use sim_relation_plan::*;
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
fn schema(nullable_extra: bool, second_table: bool) -> Schema {
    let mut item = TableBuilder::new(n("items"))
        .column(ColumnBuilder::required(n("id"), BaseDomain::I64.id()).build());
    if nullable_extra {
        item = item.column(ColumnBuilder::nullable(n("note"), BaseDomain::Text.id()).build());
    }
    let mut s = SchemaBuilder::new(n("app")).table(item.build());
    if second_table {
        s = s.table(
            TableBuilder::new(n("audit"))
                .column(ColumnBuilder::required(n("id"), BaseDomain::I64.id()).build())
                .build(),
        );
    }
    s.build(&domains(), &AcceptAllValues).unwrap()
}
fn rid(label: &str) -> RelationId {
    SchemaBuilder::new(n(label))
        .build(&domains(), &AcceptAllValues)
        .unwrap()
        .id()
        .unwrap()
}
fn program(base: Schema, target: Schema, revision: Revision) -> MigrationProgram {
    MigrationProgram {
        base_revision: rid("base"),
        base_schema: base,
        revisions: vec![revision],
        target_schema: target.id().unwrap(),
    }
}

#[test]
fn rejects_wrong_parent_skipped_revision_and_stale_schema() {
    let a = schema(false, false);
    let b = schema(true, false);
    let op = Operation::new(
        a.id().unwrap(),
        b.clone(),
        OperationKind::AddColumn {
            table: n("items"),
            column: b.tables()[0].columns()[1].clone(),
        },
    );
    let wrong = Revision::new(
        rid("r1"),
        Some(rid("other")),
        b.id().unwrap(),
        vec![op.clone()],
    );
    assert_eq!(
        admit(program(a.clone(), b.clone(), wrong)).unwrap_err(),
        MigrationError::WrongParent
    );
    let skipped = Revision::new(
        rid("r2"),
        Some(rid("r0")),
        b.id().unwrap(),
        vec![op.clone()],
    );
    assert_eq!(
        admit(program(a.clone(), b.clone(), skipped)).unwrap_err(),
        MigrationError::WrongParent
    );
    let stale = Operation::new(rid("stale"), b.clone(), op.kind().clone());
    let rev = Revision::new(rid("r1"), Some(rid("base")), b.id().unwrap(), vec![stale]);
    assert_eq!(
        admit(program(a, b, rev)).unwrap_err(),
        MigrationError::StaleBefore
    );
}

#[test]
fn rejects_incomplete_coverage_and_target_claims() {
    let a = schema(false, false);
    let b = schema(true, false);
    let lie = Operation::new(
        a.id().unwrap(),
        b.clone(),
        OperationKind::DropTable(n("missing")),
    );
    let rev = Revision::new(rid("r1"), Some(rid("base")), b.id().unwrap(), vec![lie]);
    assert_eq!(
        admit(program(a.clone(), b.clone(), rev)).unwrap_err(),
        MigrationError::IncompleteOperationCoverage
    );
    let op = derive_lossless(&a, &b).unwrap().remove(0);
    let rev = Revision::new(rid("r1"), Some(rid("base")), rid("false-target"), vec![op]);
    assert_eq!(
        admit(program(a, b, rev)).unwrap_err(),
        MigrationError::RevisionTargetMismatch
    );
}

#[test]
fn safe_derivation_is_narrow_and_admits() {
    let a = schema(false, false);
    let b = schema(true, false);
    let ops = derive_lossless(&a, &b).unwrap();
    assert_eq!(ops.len(), 1);
    let rev = Revision::new(rid("r1"), Some(rid("base")), b.id().unwrap(), ops);
    assert_eq!(
        admit(program(a.clone(), b.clone(), rev))
            .unwrap()
            .program()
            .target_schema,
        b.id().unwrap()
    );
    let created = schema(false, true);
    assert_eq!(derive_lossless(&a, &created).unwrap().len(), 1);
    assert!(matches!(
        derive_lossless(&b, &a),
        Err(MigrationError::AuthoredOperationRequired)
    ));
}

#[test]
fn invalid_backfill_and_external_drift_fail_closed() {
    let a = schema(false, false);
    let b = schema(true, false);
    let mutation = admit_mutation(
        Mutation::Delete {
            table: n("items"),
            bind: n("i"),
            predicate: None,
            returning: vec![],
        },
        &a,
        &domains(),
        RowType::new([]).unwrap(),
        AdmissionLimits::default(),
    )
    .unwrap();
    let op = Operation::new(
        b.id().unwrap(),
        b.clone(),
        OperationKind::Backfill(Box::new(mutation)),
    );
    let rev = Revision::new(rid("r1"), Some(rid("base")), b.id().unwrap(), vec![op]);
    assert_eq!(
        admit(program(b.clone(), b.clone(), rev)).unwrap_err(),
        MigrationError::InvalidBackfill
    );
    let manifest = AdoptionManifest {
        logical_schema: a.id().unwrap(),
        physical_schema: rid("physical-a"),
    };
    assert_eq!(
        manifest.verify(&rid("physical-b")),
        Err(AdoptionError::ExternalDrift)
    );
    assert_eq!(manifest.verify(&rid("physical-a")), Ok(()));
}

#[test]
fn capabilities_and_attestation_are_exact() {
    assert!(
        MigrationCapabilities {
            transactional_ddl: true,
            post_apply_introspection: true
        }
        .require()
        .is_ok()
    );
    assert!(
        MigrationCapabilities {
            transactional_ddl: true,
            post_apply_introspection: false
        }
        .require()
        .is_err()
    );
    let attestation = SchemaAttestation {
        logical_schema: rid("logical"),
        physical_schema: rid("physical"),
        revision: rid("r1"),
    };
    assert_ne!(attestation.logical_schema, attestation.physical_schema);
}
