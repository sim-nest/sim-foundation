use sim_kernel::{Expr, Symbol};

use crate::citizen_fields::{path_segments, table_op_expr};
use crate::op::{TableOp, TableOpError, decode_table_op, encode_table_op};
use crate::path::{TablePath, TablePathError, is_legal_table_segment};

#[test]
fn legal_segment_accepts_normal_name() {
    assert!(is_legal_table_segment("alpha"));
    assert!(is_legal_table_segment("a.b"));
}

#[test]
fn legal_segment_rejects_special_names() {
    assert!(!is_legal_table_segment(""));
    assert!(!is_legal_table_segment("."));
    assert!(!is_legal_table_segment(".."));
    assert!(!is_legal_table_segment("a/b"));
    assert!(!is_legal_table_segment("a\\b"));
}

#[test]
fn table_path_rejects_illegal_segment() {
    let mut path = TablePath::new();
    assert_eq!(
        path.push(".."),
        Err(TablePathError::IllegalSegment("..".to_owned()))
    );
    assert!(path.segments().is_empty());
}

#[test]
fn table_path_joins_with_slash() {
    let mut path = TablePath::new();
    path.push("a").unwrap();
    path.push("b").unwrap();
    path.push("c").unwrap();
    assert_eq!(path.segments(), ["a", "b", "c"]);
    assert_eq!(path.join(), "a/b/c");
}

fn key() -> Symbol {
    Symbol::new("k")
}

fn all_ops() -> Vec<TableOp> {
    vec![
        TableOp::Get(key()),
        TableOp::Set(key(), Expr::String("v".to_owned())),
        TableOp::Has(key()),
        TableOp::Delete(key()),
        TableOp::Keys,
        TableOp::Entries,
        TableOp::Len,
        TableOp::Clear,
        TableOp::Mkdir(key()),
        TableOp::Opendir(key()),
        TableOp::Rmdir(key()),
        TableOp::IsDir(key()),
    ]
}

#[test]
fn every_op_round_trips() {
    for op in all_ops() {
        let encoded = encode_table_op(&op);
        let decoded = decode_table_op(&encoded).unwrap();
        assert_eq!(decoded, op, "round trip failed for {op:?}");
    }
}

#[test]
fn wire_spellings_match_remote() {
    // Deliberately non-obvious spellings shared with sim-table-remote.
    assert_eq!(
        encode_table_op(&TableOp::Delete(key())),
        Expr::Call {
            operator: Box::new(Expr::Symbol(Symbol::qualified("table", "del"))),
            args: vec![Expr::Symbol(key())],
        }
    );
    assert_eq!(
        encode_table_op(&TableOp::IsDir(key())),
        Expr::Call {
            operator: Box::new(Expr::Symbol(Symbol::qualified("table", "dir?"))),
            args: vec![Expr::Symbol(key())],
        }
    );
}

#[test]
fn decode_rejects_non_table_call() {
    let expr = Expr::Call {
        operator: Box::new(Expr::Symbol(Symbol::qualified("other", "get"))),
        args: vec![Expr::Symbol(key())],
    };
    assert_eq!(decode_table_op(&expr), Err(TableOpError::NotATableCall));

    assert_eq!(
        decode_table_op(&Expr::Symbol(key())),
        Err(TableOpError::NotATableCall)
    );
}

#[test]
fn decode_rejects_unknown_op() {
    let expr = Expr::Call {
        operator: Box::new(Expr::Symbol(Symbol::qualified("table", "frobnicate"))),
        args: Vec::new(),
    };
    assert_eq!(
        decode_table_op(&expr),
        Err(TableOpError::UnknownOp("frobnicate".to_owned()))
    );
}

#[test]
fn citizen_path_segments_reject_malformed_path() {
    let expr = Expr::List(vec![
        Expr::String("ok".to_owned()),
        Expr::String("..".to_owned()),
    ]);

    let err = path_segments::decode(&expr).unwrap_err();
    assert!(err.to_string().contains("illegal segment"));
}

#[test]
fn citizen_operation_spec_rejects_malformed_op() {
    let expr = Expr::Call {
        operator: Box::new(Expr::Symbol(Symbol::qualified("table", "get"))),
        args: Vec::new(),
    };

    let err = table_op_expr::decode(&expr).unwrap_err();
    assert!(err.to_string().contains("invalid table operation"));
}
