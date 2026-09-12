use super::*;

fn invocation(binding: &CheckerBinding, subject: &str, code: &str) -> CheckInvocation {
    binding
        .instantiate(
            sid(code),
            sid("pack/demo"),
            sid(subject),
            sid("scope/x"),
            sid("closure/one"),
        )
        .unwrap()
}

fn set(
    binding: &CheckerBinding,
    head: &str,
    decisions: Vec<(&CheckInvocation, RevocationStatus)>,
) -> CheckerRevocationSet {
    CheckerRevocationSet::from_owner_snapshot(
        binding.owner().clone(),
        binding.revocation_source().clone(),
        sid("policy/v1"),
        sid(head),
        decisions
            .into_iter()
            .map(|(invocation, status)| {
                CheckerRevocationDecision::new(
                    CheckerRevocationKey::for_invocation(
                        binding.revocation_source().clone(),
                        invocation,
                    )
                    .unwrap(),
                    status,
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn issue(
    binding: &CheckerBinding,
    invocation: &CheckInvocation,
    selected: &CheckerRevocationSet,
) -> CheckerReceipt {
    let key = CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), invocation)
        .unwrap();
    CheckerReceipt::passing(
        binding,
        invocation,
        sid("result/pass"),
        EvidenceGrade::Reproducible,
        sid("provenance/held-output"),
        sid("policy/v1"),
        sid("support/exact"),
        &selected.lookup(&key),
    )
    .unwrap()
}

fn claim<'a>(
    selected: &'a CheckerRevocationSet,
    result: &'a CheckerResultId,
    provenance: &'a EvidenceProvenanceId,
    policy: &'a PolicyId,
    support: &'a EvidenceSetId,
) -> CheckerResultClaim<'a> {
    CheckerResultClaim {
        result,
        minimum_grade: EvidenceGrade::Reproducible,
        provenance,
        policy,
        support,
        revocation_set: selected.id(),
        revocation_head: selected.head(),
    }
}

#[test]
fn missing_unknown_and_revoked_decisions_fail_closed() {
    let binding = checker_binding(owner_binding().id().clone());
    let invocation = invocation(&binding, "subject/a", "code/v1");
    let key =
        CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), &invocation)
            .unwrap();
    let missing = set(&binding, "head/missing", vec![]);
    assert_eq!(missing.lookup(&key).status(), RevocationStatus::Unknown);
    assert_eq!(
        CheckerReceipt::passing(
            &binding,
            &invocation,
            sid("result/pass"),
            EvidenceGrade::Reproducible,
            sid("provenance/held-output"),
            sid("policy/v1"),
            sid("support/exact"),
            &missing.lookup(&key),
        ),
        Err(ConformanceError::RevocationUnknownOrActive)
    );
    assert_eq!(
        CheckerRevocationDecision::new(key.clone(), RevocationStatus::Unknown),
        Err(ConformanceError::InvalidRevocationDecision)
    );
    let revoked = set(
        &binding,
        "head/revoked",
        vec![(&invocation, RevocationStatus::Revoked)],
    );
    assert_eq!(
        CheckerReceipt::passing(
            &binding,
            &invocation,
            sid("result/pass"),
            EvidenceGrade::Reproducible,
            sid("provenance/held-output"),
            sid("policy/v1"),
            sid("support/exact"),
            &revoked.lookup(&key),
        ),
        Err(ConformanceError::RevocationUnknownOrActive)
    );
}

#[test]
fn wrong_source_and_stale_owner_selection_are_refused() {
    let binding = checker_binding(owner_binding().id().clone());
    let invocation = invocation(&binding, "subject/a", "code/v1");
    let current = set(
        &binding,
        "head/one",
        vec![(&invocation, RevocationStatus::Current)],
    );
    let receipt = issue(&binding, &invocation, &current);
    let result = sid("result/pass");
    let provenance = sid("provenance/held-output");
    let policy = sid("policy/v1");
    let support = sid("support/exact");

    let next = set(
        &binding,
        "head/two",
        vec![(&invocation, RevocationStatus::Current)],
    );
    let key =
        CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), &invocation)
            .unwrap();
    let stale_observation = current.lookup(&key).bind_receipt(receipt.id().clone());
    assert_eq!(
        receipt.verify_claim(
            &binding,
            &invocation,
            &claim(&next, &result, &provenance, &policy, &support),
            &stale_observation,
        ),
        Err(ConformanceError::InvocationMismatch("revocation set"))
    );

    let wrong_source: RevocationSourceId = sid("revocation/other-owner");
    let wrong_key =
        CheckerRevocationKey::for_invocation(wrong_source.clone(), &invocation).unwrap();
    let wrong = CheckerRevocationSet::from_owner_snapshot(
        binding.owner().clone(),
        wrong_source,
        policy.clone(),
        sid("head/wrong-source"),
        vec![CheckerRevocationDecision::new(wrong_key.clone(), RevocationStatus::Current).unwrap()],
    )
    .unwrap();
    assert_eq!(
        CheckerReceipt::passing(
            &binding,
            &invocation,
            result.clone(),
            EvidenceGrade::Reproducible,
            provenance.clone(),
            policy.clone(),
            support.clone(),
            &wrong.lookup(&wrong_key),
        ),
        Err(ConformanceError::InvocationMismatch("revocation source"))
    );
    let wrong_observation = wrong.lookup(&wrong_key).bind_receipt(receipt.id().clone());
    assert_eq!(
        receipt.verify_claim(
            &binding,
            &invocation,
            &claim(&wrong, &result, &provenance, &policy, &support),
            &wrong_observation,
        ),
        Err(ConformanceError::InvocationMismatch("revocation source"))
    );
}

#[test]
fn revocation_after_issue_invalidates_only_the_affected_receipt() {
    let binding = checker_binding(owner_binding().id().clone());
    let invocation_a = invocation(&binding, "subject/a", "code/v1");
    let invocation_b = invocation(&binding, "subject/b", "code/v1");
    let first = set(
        &binding,
        "head/one",
        vec![
            (&invocation_a, RevocationStatus::Current),
            (&invocation_b, RevocationStatus::Current),
        ],
    );
    let receipt_a = issue(&binding, &invocation_a, &first);
    let receipt_b = issue(&binding, &invocation_b, &first);
    let second = set(
        &binding,
        "head/two",
        vec![
            (&invocation_a, RevocationStatus::Revoked),
            (&invocation_b, RevocationStatus::Current),
        ],
    );
    let result = sid("result/pass");
    let provenance = sid("provenance/held-output");
    let policy = sid("policy/v1");
    let support = sid("support/exact");
    let expected = claim(&second, &result, &provenance, &policy, &support);
    let key_a =
        CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), &invocation_a)
            .unwrap();
    let key_b =
        CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), &invocation_b)
            .unwrap();
    assert_eq!(
        receipt_a.verify_claim(
            &binding,
            &invocation_a,
            &expected,
            &second.lookup(&key_a).bind_receipt(receipt_a.id().clone()),
        ),
        Err(ConformanceError::RevocationUnknownOrActive)
    );
    receipt_b
        .verify_claim(
            &binding,
            &invocation_b,
            &expected,
            &second.lookup(&key_b).bind_receipt(receipt_b.id().clone()),
        )
        .unwrap();
}

#[test]
fn checker_code_substitution_cannot_reuse_an_old_receipt() {
    let binding = checker_binding(owner_binding().id().clone());
    let original = invocation(&binding, "subject/a", "code/same-version-source-a");
    let substituted = invocation(&binding, "subject/a", "code/same-version-source-b");
    let selected = set(
        &binding,
        "head/one",
        vec![(&original, RevocationStatus::Current)],
    );
    let receipt = issue(&binding, &original, &selected);
    let result = sid("result/pass");
    let provenance = sid("provenance/held-output");
    let policy = sid("policy/v1");
    let support = sid("support/exact");
    let substituted_key =
        CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), &substituted)
            .unwrap();
    assert_ne!(original.id(), substituted.id());
    assert_eq!(
        selected.lookup(&substituted_key).status(),
        RevocationStatus::Unknown
    );
    assert!(
        receipt
            .verify_claim(
                &binding,
                &substituted,
                &claim(&selected, &result, &provenance, &policy, &support),
                &selected
                    .lookup(&substituted_key)
                    .bind_receipt(receipt.id().clone()),
            )
            .is_err()
    );
}

#[test]
fn exact_result_grade_provenance_policy_and_support_are_independent_claims() {
    let binding = checker_binding(owner_binding().id().clone());
    let invocation = invocation(&binding, "subject/a", "code/v1");
    let selected = set(
        &binding,
        "head/one",
        vec![(&invocation, RevocationStatus::Current)],
    );
    let key =
        CheckerRevocationKey::for_invocation(binding.revocation_source().clone(), &invocation)
            .unwrap();
    let expected_result = sid("result/pass");
    let expected_provenance = sid("provenance/held-output");
    let expected_policy = sid("policy/v1");
    let expected_support = sid("support/exact");
    for changed in 0..5 {
        let receipt = CheckerReceipt::passing(
            &binding,
            &invocation,
            if changed == 0 {
                sid("result/substituted")
            } else {
                expected_result.clone()
            },
            if changed == 1 {
                EvidenceGrade::Bootstrap
            } else {
                EvidenceGrade::Reproducible
            },
            if changed == 2 {
                sid("provenance/substituted")
            } else {
                expected_provenance.clone()
            },
            if changed == 3 {
                sid("policy/substituted")
            } else {
                expected_policy.clone()
            },
            if changed == 4 {
                sid("support/substituted")
            } else {
                expected_support.clone()
            },
            &if changed == 3 {
                let substituted_policy = CheckerRevocationSet::from_owner_snapshot(
                    binding.owner().clone(),
                    binding.revocation_source().clone(),
                    sid("policy/substituted"),
                    sid("head/one"),
                    vec![
                        CheckerRevocationDecision::new(key.clone(), RevocationStatus::Current)
                            .unwrap(),
                    ],
                )
                .unwrap();
                substituted_policy.lookup(&key)
            } else {
                selected.lookup(&key)
            },
        )
        .unwrap();
        assert!(
            receipt
                .verify_claim(
                    &binding,
                    &invocation,
                    &claim(
                        &selected,
                        &expected_result,
                        &expected_provenance,
                        &expected_policy,
                        &expected_support,
                    ),
                    &selected.lookup(&key).bind_receipt(receipt.id().clone()),
                )
                .is_err(),
            "substitution {changed} must fail",
        );
    }
}
