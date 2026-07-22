use std::sync::Arc;

// conformance: Table and Dir core expressions round-trip through shared helpers.

use sim_kernel::{CapabilityName, Cx, DefaultFactory, Expr, GrantSeat, NoopEvalPolicy, Symbol};

use crate::capabilities::{
    edit, exec, find, fs_read, fs_read_aliases, fs_write, fs_write_aliases,
    granted_capability_or_alias, net_http, net_http_aliases, require_with_aliases,
};
use crate::citizen_fields::{path_segments, table_op_expr};
use crate::op::{TableOp, TableOpError, decode_table_op, encode_table_op};
use crate::path::{TablePath, TablePathError, is_legal_table_segment};

#[test]
fn legal_segment_accepts_normal_name() {
    assert!(is_legal_table_segment("alpha"));
    assert!(is_legal_table_segment("a.b"));
}

fn seated_cx() -> (Cx, GrantSeat) {
    Cx::new_seated(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory))
}

trait GrantOutcome {
    fn expect_granted(self);
}

impl GrantOutcome for () {
    fn expect_granted(self) {}
}

impl GrantOutcome for sim_kernel::Result<()> {
    fn expect_granted(self) {
        self.unwrap();
    }
}

macro_rules! expect_granted {
    ($grant:expr) => {{
        #[allow(clippy::let_unit_value)]
        let grant_result = $grant;
        #[allow(clippy::unit_arg)]
        grant_result.expect_granted();
    }};
}

#[test]
fn canonical_capability_names_are_stable() {
    assert_eq!(fs_read().as_str(), "fs/read");
    assert_eq!(fs_write().as_str(), "fs/write");
    assert_eq!(find().as_str(), "find");
    assert_eq!(edit().as_str(), "edit");
    assert_eq!(exec().as_str(), "exec");
    assert_eq!(net_http().as_str(), "net/http");
}

#[test]
fn require_with_aliases_accepts_canonical_and_compatibility_names() {
    let (mut cx, seat) = seated_cx();
    expect_granted!(seat.grant(&mut cx, CapabilityName::new("stream.file.write")));
    assert!(require_with_aliases(&cx, fs_write(), fs_write_aliases()).is_ok());

    let granted = granted_capability_or_alias(&cx, fs_write(), fs_write_aliases()).unwrap();
    assert_eq!(granted.as_str(), "stream.file.write");

    let (mut cx, seat) = seated_cx();
    expect_granted!(seat.grant(&mut cx, net_http()));
    assert!(require_with_aliases(&cx, net_http(), net_http_aliases()).is_ok());
}

#[test]
fn require_with_aliases_denies_with_canonical_name() {
    let (cx, _) = seated_cx();
    let err = require_with_aliases(&cx, fs_read(), fs_read_aliases()).unwrap_err();
    assert!(matches!(
        err,
        sim_kernel::Error::CapabilityDenied { capability }
            if capability == fs_read()
    ));
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

fn qualified_key() -> Symbol {
    Symbol::qualified("cfg", "root")
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
fn keyed_ops_preserve_qualified_symbols() {
    let key = qualified_key();
    for op in [
        TableOp::Get(key.clone()),
        TableOp::Set(key.clone(), Expr::String("v".to_owned())),
        TableOp::Has(key.clone()),
        TableOp::Delete(key.clone()),
    ] {
        let encoded = encode_table_op(&op);
        let decoded = decode_table_op(&encoded).unwrap();
        assert_eq!(decoded, op, "qualified key changed for {op:?}");
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
fn dir_ops_reject_illegal_segments() {
    for (label, key) in [
        ("empty", Symbol::new("")),
        ("dot", Symbol::new(".")),
        ("dotdot", Symbol::new("..")),
        ("slash", Symbol::new("a/b")),
        ("backslash", Symbol::new("a\\b")),
        ("qualified", qualified_key()),
    ] {
        for (op_name, op) in [
            ("mkdir", TableOp::Mkdir(key.clone())),
            ("opendir", TableOp::Opendir(key.clone())),
            ("rmdir", TableOp::Rmdir(key.clone())),
            ("dir?", TableOp::IsDir(key.clone())),
        ] {
            let encoded = encode_table_op(&op);
            assert_eq!(
                decode_table_op(&encoded),
                Err(TableOpError::BadArg(op_name.to_owned())),
                "{op_name} accepted {label} dir segment"
            );
        }
    }
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

#[test]
fn backend_manifest_is_empty_host_registered_abi_0_1() {
    use sim_kernel::{AbiVersion, LibTarget, Version};

    let manifest = crate::backend_manifest(Symbol::qualified("table", "hash"), "9.9.9");
    assert_eq!(manifest.id, Symbol::qualified("table", "hash"));
    assert_eq!(manifest.version, Version("9.9.9".to_owned()));
    assert_eq!(manifest.abi, AbiVersion { major: 0, minor: 1 });
    assert_eq!(manifest.target, LibTarget::HostRegistered);
    assert!(manifest.requires.is_empty());
    assert!(manifest.capabilities.is_empty());
    assert!(manifest.exports.is_empty());
}
