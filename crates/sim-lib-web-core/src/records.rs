use crate::{EvidenceSelector, PolicyKind, wire::representation_identity};
use sim_kernel::{ContentId, Datum};
use sim_lib_net_core::RetrievalUri;
use std::{error::Error, fmt};

/// Conservative public decode limits for canonical record projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_text_bytes: usize,
    pub max_body_bytes: usize,
    pub max_items: usize,
}
impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_text_bytes: 1_048_576,
            max_body_bytes: 16_777_216,
            max_items: 4_096,
        }
    }
}

/// Validation or bounded decoding refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebRecordError {
    BoundExceeded(&'static str),
    InvalidSelector,
    ContentIdentity,
    MissingDecision(PolicyKind),
    InvalidRecord(&'static str),
}
impl fmt::Display for WebRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl Error for WebRecordError {}

/// Raw retrieved bytes with their own content identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebCapture {
    pub retrieval_uri: RetrievalUri,
    pub content_id: ContentId,
    pub body: Vec<u8>,
    pub exchange: WebExchange,
}
impl WebCapture {
    pub fn checked(
        retrieval_uri: RetrievalUri,
        content_id: ContentId,
        body: Vec<u8>,
        exchange: WebExchange,
        limits: DecodeLimits,
    ) -> Result<Self, WebRecordError> {
        if body.len() > limits.max_body_bytes {
            return Err(WebRecordError::BoundExceeded("raw body"));
        }
        let actual = Datum::Bytes(body.clone())
            .content_id()
            .map_err(|_| WebRecordError::ContentIdentity)?;
        if actual != content_id {
            return Err(WebRecordError::ContentIdentity);
        }
        Ok(Self {
            retrieval_uri,
            content_id,
            body,
            exchange,
        })
    }
}

/// Provider-neutral request/response exchange facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebExchange {
    pub method: String,
    pub status: u16,
    pub final_uri: String,
    pub media_type: Option<String>,
    pub received_bytes: u64,
}

/// Immutable normalized text and the provenance required to interpret it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebRepresentation {
    pub content_id: ContentId,
    pub raw_source_id: ContentId,
    pub text: String,
    pub codec: String,
    pub codec_version: String,
    pub media_type: String,
    pub charset: Option<String>,
    pub language: Option<String>,
    pub fidelity_warnings: Vec<String>,
}
/// Codec and fidelity provenance for one normalized representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepresentationMetadata {
    pub codec: String,
    pub codec_version: String,
    pub media_type: String,
    pub charset: Option<String>,
    pub language: Option<String>,
    pub fidelity_warnings: Vec<String>,
}
impl WebRepresentation {
    pub fn checked(
        raw_source_id: ContentId,
        text: String,
        metadata: RepresentationMetadata,
        limits: DecodeLimits,
    ) -> Result<Self, WebRecordError> {
        if text.len() > limits.max_text_bytes || metadata.fidelity_warnings.len() > limits.max_items
        {
            return Err(WebRecordError::BoundExceeded("representation"));
        }
        let identity = representation_identity(&raw_source_id, &text, &metadata);
        let content_id = identity
            .content_id()
            .map_err(|_| WebRecordError::ContentIdentity)?;
        if content_id == raw_source_id {
            return Err(WebRecordError::ContentIdentity);
        }
        Ok(Self {
            content_id,
            raw_source_id,
            text,
            codec: metadata.codec,
            codec_version: metadata.codec_version,
            media_type: metadata.media_type,
            charset: metadata.charset,
            language: metadata.language,
            fidelity_warnings: metadata.fidelity_warnings,
        })
    }
    /// Select Unicode scalar offsets from this immutable representation.
    pub fn select(&self, start: u32, end: u32) -> Result<EvidenceSelector, WebRecordError> {
        let count = self.text.chars().count();
        if start > end || end as usize > count {
            return Err(WebRecordError::InvalidSelector);
        }
        let exact = self
            .text
            .chars()
            .skip(start as usize)
            .take((end - start) as usize)
            .collect();
        EvidenceSelector::checked(self.content_id.clone(), start, end, exact, &self.text)
    }
}
