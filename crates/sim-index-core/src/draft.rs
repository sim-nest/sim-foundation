//! Helpers for materializing authored feature drafts.

use crate::{
    FeatureDraft, FeatureRecord,
    key::{CanonicalFeatureKey, canonical_feature_key},
};

/// Materializes a draft with the default canonical key.
pub fn materialize_draft(draft: FeatureDraft) -> FeatureRecord {
    let key = canonical_feature_key(&draft.subject, draft.id.as_str());
    materialize_draft_with_key(draft, key)
}

/// Materializes a draft with an explicit canonical key.
pub fn materialize_draft_with_key(draft: FeatureDraft, key: CanonicalFeatureKey) -> FeatureRecord {
    FeatureRecord {
        id: draft.id,
        key,
        subject: draft.subject,
        title: draft.title,
        summary: draft.summary,
        anchors: draft.claims_anchors,
        surfaces: draft.claims_surfaces,
        specimens: draft.claims_specimens,
        grammar_contracts: draft.grammar_contracts,
        doc_anchor: draft.doc_anchor,
    }
}
