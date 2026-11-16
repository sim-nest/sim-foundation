// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
    assert_eq!(
        resolved.to_string(),
        format!("/{}", ["project", "estimate", "final", "cost"].join("/"))
    );
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
