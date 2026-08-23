use sim_kernel::{Datum, Ref, Symbol};
use sim_relation_core::*;

fn domain(name: &str) -> DomainId {
    DomainId::new(Symbol::qualified("test", name)).unwrap()
}

#[test]
fn names_are_sql_dialect_neutral_but_structurally_valid() {
    assert!(TableName::new(Symbol::new("select / odd-name \"quoted\"")).is_ok());
    assert_eq!(TableName::new(Symbol::new("")), Err(NameError::Empty));
    assert_eq!(
        TableName::new(Symbol::new("bad\nname")),
        Err(NameError::Control)
    );
}

#[test]
fn catalogs_reject_duplicates_bad_refs_and_incoherent_traits() {
    let id = domain("uuid");
    let spec = DomainSpec::new(
        id.clone(),
        StorageRepr::Text,
        Ref::Symbol(Symbol::new("uuid-shape")),
        [DomainTrait::Equatable],
    )
    .unwrap();
    assert!(matches!(
        DomainCatalog::new([spec.clone(), spec]),
        Err(DomainError::DuplicateId(_))
    ));
    assert_eq!(
        DomainSpec::new(
            id.clone(),
            StorageRepr::Text,
            Ref::Handle(sim_kernel::HandleId(1)),
            []
        ),
        Err(DomainError::InvalidShapeRef)
    );
    assert_eq!(
        DomainSpec::new(
            id,
            StorageRepr::Text,
            Ref::Symbol(Symbol::new("shape")),
            [DomainTrait::Ordered]
        ),
        Err(DomainError::IncoherentTraits)
    );
}

#[test]
fn typed_null_is_absence_and_rows_enforce_types() {
    let id = domain("text");
    let ty = RowType::new([FieldType {
        name: FieldName::new(Symbol::new("nickname")).unwrap(),
        domain: id.clone(),
        nullable: true,
    }])
    .unwrap();
    let row = Row::new(ty, [Cell::null(id)]).unwrap();
    assert_eq!(row.cells()[0].value(), None);
    assert!(matches!(row.to_datum(), Datum::Node { .. }));
}

#[test]
fn base_storage_conversions_are_exact_and_float_edges_are_closed() {
    let builtins = DomainCatalog::new([
        BaseDomain::Bool.spec(),
        BaseDomain::I64.spec(),
        BaseDomain::F64.spec(),
        BaseDomain::Text.spec(),
        BaseDomain::Bytes.spec(),
    ])
    .unwrap();
    assert_eq!(builtins.iter().count(), 5);
    for (base, value) in [
        (BaseDomain::Bool, StorageValue::Bool(true)),
        (BaseDomain::I64, StorageValue::I64(i64::MIN)),
        (BaseDomain::F64, StorageValue::F64(-0.0)),
        (BaseDomain::Text, StorageValue::Text("hello".into())),
        (BaseDomain::Bytes, StorageValue::Bytes(vec![0, 255])),
    ] {
        let datum = base.to_datum(value.clone()).unwrap();
        assert_eq!(
            base.from_datum(&datum).unwrap(),
            value_with_canonical_zero(value)
        );
    }
    assert_eq!(
        BaseDomain::F64.to_datum(StorageValue::F64(f64::NAN)),
        Err(DomainError::NonFiniteFloat)
    );
    assert_eq!(
        BaseDomain::F64.to_datum(StorageValue::F64(f64::INFINITY)),
        Err(DomainError::NonFiniteFloat)
    );
    assert_eq!(
        BaseDomain::F64.to_datum(StorageValue::F64(-0.0)).unwrap(),
        BaseDomain::F64.to_datum(StorageValue::F64(0.0)).unwrap()
    );
}

fn value_with_canonical_zero(value: StorageValue) -> StorageValue {
    match value {
        StorageValue::F64(v) => StorageValue::F64(v + 0.0),
        other => other,
    }
}

#[test]
fn custom_domains_need_no_provider_change_and_identity_is_kernel_identity() {
    let uuid = DomainSpec::new(
        domain("uuid"),
        StorageRepr::Text,
        Ref::Symbol(Symbol::new("uuid-shape")),
        [DomainTrait::Equatable],
    )
    .unwrap();
    let catalog = DomainCatalog::new([uuid.clone()]).unwrap();
    assert_eq!(catalog.get(uuid.id()), Some(&uuid));
    assert_eq!(uuid.card_datum(), uuid.lisp_datum());
    assert_eq!(
        RelationId::of(&uuid).unwrap().content_id(),
        &uuid.to_datum().content_id().unwrap()
    );
    assert_eq!(
        RelationId::of(&uuid).unwrap(),
        RelationId::of(&uuid.clone()).unwrap()
    );
}
