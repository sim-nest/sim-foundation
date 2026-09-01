use super::*;
use sim_kernel::{ContentId, Datum};

fn raw(body: &[u8]) -> ContentId {
    Datum::Bytes(body.to_vec()).content_id().unwrap()
}
#[test]
fn unicode_selectors_are_scalar_checked_and_round_trip() {
    let rep = WebRepresentation::checked(
        raw(b"raw"),
        "aé🦀z".into(),
        RepresentationMetadata {
            codec: "text".into(),
            codec_version: "1".into(),
            media_type: "text/plain".into(),
            charset: None,
            language: None,
            fidelity_warnings: vec![],
        },
        DecodeLimits::default(),
    )
    .unwrap();
    let s = rep.select(1, 3).unwrap().with_context(
        Some("a".into()),
        Some("z".into()),
        Some(vec!["p".into()]),
    );
    assert_eq!(s.exact, "é🦀");
    assert_eq!(
        EvidenceSelector::from_datum(&s.to_datum(), &rep, DecodeLimits::default()).unwrap(),
        s
    );
    assert!(
        EvidenceSelector::checked(rep.content_id.clone(), 1, 3, "bad".into(), &rep.text).is_err()
    );
}
#[test]
fn raw_and_representation_digests_are_separate() {
    let id = raw(b"hello");
    let rep = WebRepresentation::checked(
        id.clone(),
        "hello".into(),
        RepresentationMetadata {
            codec: "utf8".into(),
            codec_version: "1".into(),
            media_type: "text/plain".into(),
            charset: Some("utf-8".into()),
            language: None,
            fidelity_warnings: vec![],
        },
        DecodeLimits::default(),
    )
    .unwrap();
    assert_ne!(rep.content_id, id);
}
#[test]
fn absent_policy_is_denial() {
    assert!(matches!(
        PolicyReceipt::checked(vec![]),
        Err(WebRecordError::MissingDecision(_))
    ));
}
