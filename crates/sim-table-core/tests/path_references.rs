use sim_table_core::{TablePath, TablePathRef, TablePathRefError};

// conformance: table path references parse, normalize, resolve, and fail closed.

#[test]
fn relative_references_resolve_against_canonical_paths() {
    let base = TablePath::from_segments(["project", "estimate", "draft"]).unwrap();
    let reference = TablePathRef::parse("../final/./cost").unwrap();

    let resolved = base.resolve(&reference).unwrap();

    assert_eq!(
        resolved.segments(),
        ["project", "estimate", "final", "cost"]
    );
    assert_eq!(resolved.to_string(), "/project/estimate/final/cost");
    assert_eq!(
        TablePath::parse_absolute(&resolved.to_string()).unwrap(),
        resolved
    );
}

#[test]
fn references_reject_root_escape_and_ambiguous_segments() {
    let base = TablePath::from_segments(["project"]).unwrap();

    assert_eq!(
        base.resolve(&TablePathRef::parse("../../cost").unwrap()),
        Err(TablePathRefError::RootEscape)
    );
    assert_eq!(
        TablePathRef::parse("alpha//beta"),
        Err(TablePathRefError::EmptySegment)
    );
    assert_eq!(
        TablePathRef::parse("alpha%2Fbeta"),
        Err(TablePathRefError::IllegalSegment("alpha/beta".to_owned()))
    );
}
