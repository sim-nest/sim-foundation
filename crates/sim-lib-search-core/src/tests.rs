fn raw(b: &[u8]) -> ContentId {
    Datum::Bytes(b.to_vec()).content_id().unwrap()
}
#[test]
fn query_order_remains_distinct_observations() {
    let a = SearchObservation::checked("https://EXAMPLE.com/?a=1&b=2", None, None).unwrap();
    let b = SearchObservation::checked("https://example.com/?b=2&a=1", None, None).unwrap();
    assert_ne!(a, b)
}
#[test]
fn citation_round_trip_requires_exact_quote() {
    let rep = WebRepresentation::checked(
        raw(b"raw"),
        "alpha beta".into(),
        sim_lib_web_core::RepresentationMetadata {
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
    let citation = Citation::checked(&rep, rep.select(6, 10).unwrap()).unwrap();
    assert_eq!(
        Citation::from_datum(&citation.to_datum(), &rep, DecodeLimits::default()).unwrap(),
        citation
    );
    let other = WebRepresentation::checked(
        raw(b"other"),
        "alpha zeta".into(),
        sim_lib_web_core::RepresentationMetadata {
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
    assert!(Citation::from_datum(&citation.to_datum(), &other, DecodeLimits::default()).is_err())
}
#[test]
fn provider_claim_is_not_a_citation() {
    let claim = ProviderClaim {
        provider: "p".into(),
        uri: "https://example.com".into(),
        title: None,
        snippet: Some("beta".into()),
        position: Some(1),
    };
    assert_eq!(claim.snippet.as_deref(), Some("beta"));
    assert_eq!(
        RECORD_DESCRIPTORS
            .iter()
            .filter(|d| d.symbol.ends_with("Citation"))
            .count(),
        1
    )
}
use super::*;
