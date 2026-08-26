use crate::{AnchorId, IndexDoc, IndexError};
use std::collections::BTreeSet;

pub(crate) fn reject_dangling_doc_anchors(doc: &IndexDoc) -> Result<(), IndexError> {
    let anchors = doc
        .anchors
        .iter()
        .map(|record| record.id.as_str())
        .collect::<BTreeSet<_>>();
    for specimen in &doc.specimens {
        check_doc_anchor(&anchors, specimen.id.as_str(), specimen.doc_anchor.as_ref())?;
    }
    for draft in &doc.drafts {
        check_doc_anchor(&anchors, draft.id.as_str(), draft.doc_anchor.as_ref())?;
    }
    for feature in &doc.features {
        check_doc_anchor(&anchors, feature.id.as_str(), feature.doc_anchor.as_ref())?;
    }
    for route in &doc.routes {
        check_doc_anchor(&anchors, route.id.as_str(), route.doc_anchor.as_ref())?;
    }
    Ok(())
}

fn check_doc_anchor(
    anchors: &BTreeSet<&str>,
    owner: &str,
    anchor: Option<&AnchorId>,
) -> Result<(), IndexError> {
    let Some(anchor) = anchor else {
        return Ok(());
    };
    if anchors.contains(anchor.as_str()) {
        Ok(())
    } else {
        Err(IndexError::DanglingDocAnchor {
            owner: owner.to_owned(),
            id: anchor.to_string(),
        })
    }
}
