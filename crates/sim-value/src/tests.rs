//! Tests for the value ergonomics.

use sim_kernel::{CapabilityName, Error, Expr, NumberLiteral, Symbol};

use crate::access::{
    as_f64, as_i64, as_u64, entry_field, entry_field_any, entry_required, entry_required_bool,
    entry_required_bool_any, entry_required_list, entry_required_list_any, entry_required_str,
    entry_required_str_any, entry_required_sym, entry_required_sym_any, extra_fields, field,
    field_any, field_bool, field_str, remove, remove_strict, required, required_bool, required_map,
    required_str, required_sym, set, set_strict,
};
use crate::build::{entry, float, int, keyword, list, map, num_q, qsym, sym, text, uint, vector};
use crate::capability_names_from_expr;
use crate::edit::{edit, edit_lines};
use crate::kind::expr_kind;
use crate::path::{Path, PathError, get, remove_at, set_at};

#[test]
fn field_any_accepts_symbol_and_string_keys() {
    let symbol_keyed = map(vec![("role", text("user"))]);
    assert_eq!(field_any(&symbol_keyed, "role"), Some(&text("user")));

    // A string-keyed record, as provider JSON projections produce.
    let string_keyed = Expr::Map(vec![(text("role"), text("user"))]);
    assert_eq!(field_any(&string_keyed, "role"), Some(&text("user")));
    // Plain `field` only matches the bare-symbol key.
    assert_eq!(field(&string_keyed, "role"), None);
    assert_eq!(field_any(&string_keyed, "missing"), None);
}

#[test]
fn required_reports_context_on_missing_field() {
    let value = map(vec![("a", int(1))]);
    assert_eq!(required(&value, "a", "Record").unwrap(), &int(1));
    let err = required(&value, "b", "Record").unwrap_err();
    assert_eq!(
        err.to_string(),
        "evaluation error: Record is missing field b"
    );
}

#[test]
fn typed_required_readers_coerce_or_label_the_error() {
    let value = Expr::Map(vec![
        (sym("name"), text("widget")),
        (sym("kind"), sym("gadget")),
        (sym("on"), Expr::Bool(true)),
        (sym("attrs"), map(vec![("a", int(1))])),
        (sym("count"), int(7)),
    ]);
    assert_eq!(required_str(&value, "name", "Item").unwrap(), "widget");
    assert_eq!(
        required_sym(&value, "kind", "Item").unwrap(),
        Symbol::new("gadget")
    );
    assert!(required_bool(&value, "on", "Item").unwrap());
    assert_eq!(required_map(&value, "attrs", "Item").unwrap().len(), 1);

    // Wrong type -> context-labeled Error::Eval (not a panic, not a silent None).
    let err = required_str(&value, "count", "Item").unwrap_err();
    assert_eq!(
        err.to_string(),
        "evaluation error: Item field count is not a string"
    );
    // Missing -> the `required` missing-field message.
    let miss = required_bool(&value, "gone", "Item").unwrap_err();
    assert_eq!(
        miss.to_string(),
        "evaluation error: Item is missing field gone"
    );
}

#[test]
fn field_bool_reads_expr_bool_only() {
    let value = map(vec![("on", Expr::Bool(true)), ("name", text("x"))]);
    assert_eq!(field_bool(&value, "on"), Some(true));
    assert_eq!(field_bool(&value, "name"), None); // a string is not a bool
    assert_eq!(field_bool(&value, "missing"), None);
    // Accepts a string key too, matching field_any.
    let string_keyed = Expr::Map(vec![(text("on"), Expr::Bool(false))]);
    assert_eq!(field_bool(&string_keyed, "on"), Some(false));
}

#[test]
fn keyword_builds_a_bare_symbol() {
    assert_eq!(keyword("ready"), Symbol::new("ready"));
    assert_eq!(sym("ready"), Expr::Symbol(keyword("ready")));
}

#[test]
fn slice_accessors_match_the_map_level_ones() {
    let entries = vec![
        (sym("role"), text("user")),
        (text("name"), text("x")), // a string key, as provider records produce
    ];
    // Bare lookup matches only the symbol key; *_any also matches the string key.
    assert_eq!(entry_field(&entries, "role"), Some(&text("user")));
    assert_eq!(entry_field(&entries, "name"), None);
    assert_eq!(entry_field_any(&entries, "name"), Some(&text("x")));
    assert_eq!(
        entry_required(&entries, "role", "Rec").unwrap(),
        &text("user")
    );
    assert_eq!(
        entry_required(&entries, "gone", "Rec")
            .unwrap_err()
            .to_string(),
        "evaluation error: Rec is missing field gone"
    );
    // field/field_any now delegate to the slice primitives: identical results.
    let as_map = Expr::Map(entries.clone());
    assert_eq!(field(&as_map, "role"), entry_field(&entries, "role"));
    assert_eq!(
        field_any(&as_map, "name"),
        entry_field_any(&entries, "name")
    );
}

#[test]
fn extra_fields_reports_only_unknown_names() {
    let value = Expr::Map(vec![
        (sym("known"), int(1)),
        (text("alsoknown"), int(2)),
        (sym("surprise"), int(3)),
    ]);
    assert_eq!(
        extra_fields(&value, &["known", "alsoknown"]),
        vec!["surprise"]
    );
    assert!(extra_fields(&value, &["known", "alsoknown", "surprise"]).is_empty());
}

#[test]
fn entry_builds_a_bare_symbol_key() {
    assert_eq!(entry("a", int(1)), (sym("a"), int(1)));
    assert_eq!(
        Expr::Map(vec![entry("a", int(1))]),
        map(vec![("a", int(1))])
    );
}

#[test]
fn num_q_preserves_qualified_and_unqualified_domains() {
    // Unqualified is identical to `num`.
    assert_eq!(num_q(None, "f64", "1.5"), crate::build::num("f64", "1.5"));
    // Qualified emits the `numbers/f64` domain spelling.
    assert_eq!(
        num_q(Some("numbers"), "f64", "1.5"),
        Expr::Number(NumberLiteral {
            domain: Symbol::qualified("numbers", "f64"),
            canonical: "1.5".to_owned(),
        })
    );
}

#[test]
fn builders_and_readers_round_trip() {
    let value = map(vec![
        ("a", int(1)),
        ("pi", float(3.5)),
        ("name", text("x")),
        ("xs", list(vec![int(1), int(2)])),
    ]);
    assert_eq!(field(&value, "a").and_then(as_i64), Some(1));
    assert_eq!(field(&value, "pi").and_then(as_f64), Some(3.5));
    assert_eq!(field_str(&value, "name"), Some("x"));
    assert!(matches!(field(&value, "xs"), Some(Expr::List(items)) if items.len() == 2));
    assert_eq!(field(&value, "missing"), None);
}

#[test]
fn integer_readers_respect_number_domains() {
    let zero = uint(0);
    let max_i64 = uint(i64::MAX as u64);
    let above_i64 = uint(i64::MAX as u64 + 1);
    let max_u64 = uint(u64::MAX);
    let qualified_i64 = num_q(Some("numbers"), "i64", "7");
    let qualified_u64 = num_q(Some("numbers"), "u64", "9");

    assert_eq!(zero, crate::build::num("u64", "0"));
    assert_eq!(as_u64(&zero), Some(0));
    assert_eq!(as_u64(&max_i64), Some(i64::MAX as u64));
    assert_eq!(as_u64(&above_i64), Some(i64::MAX as u64 + 1));
    assert_eq!(as_u64(&max_u64), Some(u64::MAX));
    assert_eq!(as_u64(&qualified_u64), Some(9));

    assert_eq!(as_i64(&int(7)), Some(7));
    assert_eq!(as_i64(&qualified_i64), Some(7));
    assert_eq!(as_i64(&zero), None);
    assert_eq!(as_i64(&max_i64), None);
    assert_eq!(as_i64(&above_i64), None);
    assert_eq!(as_i64(&max_u64), None);
    assert_eq!(as_u64(&int(7)), None);
    assert_eq!(as_i64(&float(7.0)), None);
}

#[test]
fn float_canonical_drops_trailing_zero() {
    // Matches the hand-rolled number(f64) helpers this replaces.
    assert_eq!(float(80.0), crate::build::num("f64", "80"));
    assert_eq!(float(1.5), crate::build::num("f64", "1.5"));
}

#[test]
fn set_and_remove_preserve_siblings() {
    let value = map(vec![("a", int(1)), ("b", int(2)), ("c", int(3))]);
    let updated = set(&value, "b", int(9));
    assert_eq!(field(&updated, "b"), Some(&int(9)));
    assert_eq!(field(&updated, "a"), Some(&int(1)));
    assert_eq!(field(&updated, "c"), Some(&int(3)));
    // original unchanged
    assert_eq!(field(&value, "b"), Some(&int(2)));
    // insert a new key
    let inserted = set(&value, "d", int(4));
    assert_eq!(field(&inserted, "d"), Some(&int(4)));
    // remove
    let removed = remove(&value, "a");
    assert_eq!(field(&removed, "a"), None);
    assert_eq!(field(&removed, "b"), Some(&int(2)));
}

#[test]
fn provider_style_updates_replace_visible_keys_without_leaving_duplicates() {
    let string_keyed = Expr::Map(vec![(text("role"), text("user"))]);
    let updated = set(&string_keyed, "role", text("admin"));
    assert_eq!(updated, Expr::Map(vec![(text("role"), text("admin"))]));
    assert_eq!(field_any(&updated, "role"), Some(&text("admin")));

    let mixed = Expr::Map(vec![
        (text("role"), text("user")),
        (sym("role"), text("stale")),
        (sym("keep"), int(1)),
    ]);
    let normalized = set(&mixed, "role", text("admin"));
    assert_eq!(
        normalized,
        Expr::Map(vec![(text("role"), text("admin")), (sym("keep"), int(1))])
    );

    let removed = remove(&mixed, "role");
    assert_eq!(removed, Expr::Map(vec![(sym("keep"), int(1))]));
}

#[test]
fn strict_updates_only_touch_bare_symbol_fields() {
    let symbol_keyed = map(vec![("role", text("user"))]);
    assert_eq!(
        set_strict(&symbol_keyed, "role", text("admin")),
        map(vec![("role", text("admin"))])
    );
    assert_eq!(remove_strict(&symbol_keyed, "role"), map(vec![]));

    let string_keyed = Expr::Map(vec![(text("role"), text("user"))]);
    assert_eq!(
        set_strict(&string_keyed, "role", text("admin")),
        string_keyed
    );
    assert_eq!(remove_strict(&string_keyed, "role"), string_keyed);
}

#[test]
fn expr_kind_tokens_are_stable() {
    assert_eq!(expr_kind(&Expr::Nil), "nil");
    assert_eq!(expr_kind(&int(1)), "number");
    assert_eq!(expr_kind(&sym("x")), "symbol");
    assert_eq!(expr_kind(&text("y")), "string");
    assert_eq!(expr_kind(&list(vec![])), "list");
    assert_eq!(expr_kind(&vector(vec![])), "vector");
    assert_eq!(expr_kind(&map(vec![])), "map");
}

#[test]
fn capability_names_read_nil_singletons_lists_and_vectors() {
    assert_eq!(capability_names_from_expr(&Expr::Nil).unwrap(), Vec::new());
    assert_eq!(
        capability_names_from_expr(&sym("read-eval")).unwrap(),
        vec![CapabilityName::new("read-eval")]
    );
    assert_eq!(
        capability_names_from_expr(&list(vec![sym("read-eval"), text("net.http")])).unwrap(),
        vec![
            CapabilityName::new("read-eval"),
            CapabilityName::new("net.http")
        ]
    );
    assert_eq!(
        capability_names_from_expr(&vector(vec![sym("ai"), qsym("net", "http")])).unwrap(),
        vec![CapabilityName::new("ai"), CapabilityName::new("net/http")]
    );
}

#[test]
fn capability_names_reject_non_capability_items() {
    let err = capability_names_from_expr(&list(vec![int(1)])).unwrap_err();
    assert!(matches!(
        err,
        Error::TypeMismatch {
            expected: "capability symbol or string",
            found: "non-capability"
        }
    ));
}

#[test]
fn edit_absent_pattern_errors() {
    let err = edit("alpha beta", "gamma", "delta", false).unwrap_err();
    assert_eq!(
        err.to_string(),
        "evaluation error: edit: pattern not found: \"gamma\""
    );
}

#[test]
fn edit_ambiguous_pattern_errors() {
    let err = edit("alpha beta alpha", "alpha", "delta", false).unwrap_err();
    assert_eq!(
        err.to_string(),
        "evaluation error: edit: pattern is not unique (2 matches); pass replace_all"
    );
}

#[test]
fn edit_unique_pattern_round_trips() {
    assert_eq!(
        edit("alpha beta", "beta", "gamma", false).unwrap(),
        "alpha gamma"
    );
}

#[test]
fn edit_replace_all_replaces_every_match() {
    assert_eq!(
        edit("alpha beta alpha", "alpha", "delta", true).unwrap(),
        "delta beta delta"
    );
}

#[test]
fn edit_lines_replaces_inclusive_range() {
    assert_eq!(
        edit_lines("a\nb\nc\n", 2, 3, "B\nC\n").unwrap(),
        "a\nB\nC\n"
    );
}

#[test]
fn path_wire_form_round_trips() {
    let path = Path::new().key(sym("nodes")).index(0).key(sym("title"));
    let wire = path.to_expr();
    assert_eq!(Path::from_expr(&wire).unwrap(), path);
}

#[test]
fn get_and_set_at_navigate_nested_maps_and_sequences() {
    let root = map(vec![(
        "nodes",
        list(vec![
            map(vec![("title", text("a"))]),
            map(vec![("title", text("b"))]),
        ]),
    )]);
    let path = Path::new().key(sym("nodes")).index(1).key(sym("title"));
    assert_eq!(get(&root, &path), Some(&text("b")));

    let updated = set_at(&root, &path, text("z")).unwrap();
    assert_eq!(get(&updated, &path), Some(&text("z")));
    // sibling element preserved
    let sibling = Path::new().key(sym("nodes")).index(0).key(sym("title"));
    assert_eq!(get(&updated, &sibling), Some(&text("a")));
    // original unchanged
    assert_eq!(get(&root, &path), Some(&text("b")));
}

#[test]
fn set_at_empty_path_replaces_root() {
    let replaced = set_at(&int(1), &Path::new(), text("new")).unwrap();
    assert_eq!(replaced, text("new"));
}

#[test]
fn set_at_inserts_a_missing_final_key() {
    let root = map(vec![("a", int(1))]);
    let updated = set_at(&root, &Path::new().key(sym("b")), int(2)).unwrap();
    assert_eq!(field(&updated, "b"), Some(&int(2)));
    assert_eq!(field(&updated, "a"), Some(&int(1)));
}

#[test]
fn set_at_errors_on_bad_navigation() {
    let root = map(vec![("a", int(1))]);
    // missing intermediate key
    let deep = Path::new().key(sym("missing")).key(sym("x"));
    assert_eq!(set_at(&root, &deep, int(0)), Err(PathError::MissingKey));
    // index into a non-sequence
    let idx = Path::new().key(sym("a")).index(0);
    assert_eq!(set_at(&root, &idx, int(0)), Err(PathError::NotASequence));
}

#[test]
fn remove_at_removes_a_nested_key() {
    let root = map(vec![("outer", map(vec![("x", int(1)), ("y", int(2))]))]);
    let path = Path::new().key(sym("outer")).key(sym("x"));
    let removed = remove_at(&root, &path).unwrap();
    assert_eq!(get(&removed, &path), None);
    assert_eq!(
        get(&removed, &Path::new().key(sym("outer")).key(sym("y"))),
        Some(&int(2))
    );
}

#[test]
fn set_at_matches_the_scene_and_editor_semantics() {
    // The exact shape the scene differ / universal editor produce: a keyed set
    // deep in a map preserves every other entry and clones rather than mutates.
    let root = map(vec![
        ("a", int(1)),
        ("nested", map(vec![("x", int(10)), ("y", int(20))])),
    ]);
    let path = Path::new().key(sym("nested")).key(sym("x"));
    let updated = set_at(&root, &path, int(99)).unwrap();
    let expected = map(vec![
        ("a", int(1)),
        ("nested", map(vec![("x", int(99)), ("y", int(20))])),
    ]);
    assert_eq!(updated, expected);
}

#[test]
fn entry_required_str_reads_string_and_rejects_wrong_type_with_expr_kind() {
    let ok = vec![entry("name", text("hi"))];
    assert_eq!(
        entry_required_str(&ok, "name", "string field").unwrap(),
        "hi"
    );

    let wrong = vec![entry("name", int(3))];
    let err = entry_required_str(&wrong, "name", "string field").unwrap_err();
    let text = err.to_string();
    assert!(text.contains("string field"), "expected label: {text}");
    assert!(text.contains("number"), "expr_kind found label: {text}");
}

#[test]
fn entry_required_str_reports_missing_field_with_context() {
    let entries = vec![entry("other", text("x"))];
    let err = entry_required_str(&entries, "name", "string field").unwrap_err();
    assert!(err.to_string().contains("missing field name"));
}

#[test]
fn entry_required_sym_reads_symbol() {
    let entries = vec![entry("kind", sym("packet"))];
    assert_eq!(
        entry_required_sym(&entries, "kind", "symbol field").unwrap(),
        &Symbol::new("packet")
    );
    let wrong = vec![entry("kind", text("packet"))];
    assert!(entry_required_sym(&wrong, "kind", "symbol field").is_err());
}

#[test]
fn entry_required_bool_reads_bool() {
    let entries = vec![entry("live", Expr::Bool(true))];
    assert!(entry_required_bool(&entries, "live", "bool field").unwrap());
    let wrong = vec![entry("live", int(1))];
    assert!(entry_required_bool(&wrong, "live", "bool field").is_err());
}

#[test]
fn entry_required_list_borrows_items() {
    let entries = vec![entry("items", list(vec![int(1), int(2)]))];
    assert_eq!(
        entry_required_list(&entries, "items", "list field").unwrap(),
        &[int(1), int(2)]
    );
    let wrong = vec![entry("items", vector(vec![int(1)]))];
    assert!(entry_required_list(&wrong, "items", "list field").is_err());
}

#[test]
fn entry_required_str_matches_bare_symbol_key_only() {
    // A string key is NOT matched by the strict reader; the _any variant is.
    let string_keyed = vec![(text("name"), text("hi"))];
    assert!(entry_required_str(&string_keyed, "name", "string field").is_err());
    assert_eq!(
        entry_required_str_any(&string_keyed, "name", "string field").unwrap(),
        "hi"
    );
}

#[test]
fn entry_required_any_variants_match_symbol_and_string_keys() {
    let symbol_keyed = vec![entry("role", text("user"))];
    let string_keyed = vec![(text("role"), text("user"))];
    assert_eq!(
        entry_required_str_any(&symbol_keyed, "role", "string field").unwrap(),
        "user"
    );
    assert_eq!(
        entry_required_str_any(&string_keyed, "role", "string field").unwrap(),
        "user"
    );

    let sym_any = vec![(text("kind"), sym("packet"))];
    assert_eq!(
        entry_required_sym_any(&sym_any, "kind", "symbol field").unwrap(),
        &Symbol::new("packet")
    );
    let bool_any = vec![(text("live"), Expr::Bool(false))];
    assert!(!entry_required_bool_any(&bool_any, "live", "bool field").unwrap());
    let list_any = vec![(text("items"), list(vec![int(7)]))];
    assert_eq!(
        entry_required_list_any(&list_any, "items", "list field").unwrap(),
        &[int(7)]
    );
}

#[test]
fn entry_required_any_rejects_wrong_type_with_expr_kind() {
    let entries = vec![(text("live"), text("nope"))];
    let err = entry_required_bool_any(&entries, "live", "bool field").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("bool field") && msg.contains("string"),
        "{msg}"
    );
}
