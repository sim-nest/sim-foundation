use crate::{MAX_BINDING_FIELD_BYTES, ProtectError};

/// Caller-owned binding authenticated with a protected state value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateBinding {
    pub(crate) purpose: Vec<u8>,
    pub(crate) audience: Vec<u8>,
    pub(crate) subject: Vec<u8>,
    pub(crate) context_digest: Vec<u8>,
    pub(crate) expires_at_ms: u64,
}

impl StateBinding {
    /// Validates and constructs a binding. Fields are opaque bytes and are encoded
    /// with independent lengths, so no concatenation can collide with another tuple.
    pub fn new(
        purpose: impl Into<Vec<u8>>,
        audience: impl Into<Vec<u8>>,
        subject: impl Into<Vec<u8>>,
        context_digest: impl Into<Vec<u8>>,
        expires_at_ms: u64,
    ) -> Result<Self, ProtectError> {
        let binding = Self {
            purpose: purpose.into(),
            audience: audience.into(),
            subject: subject.into(),
            context_digest: context_digest.into(),
            expires_at_ms,
        };
        for field in [
            &binding.purpose,
            &binding.audience,
            &binding.subject,
            &binding.context_digest,
        ] {
            if field.len() > MAX_BINDING_FIELD_BYTES {
                return Err(ProtectError::LimitExceeded);
            }
        }
        Ok(binding)
    }

    /// Returns the exclusive Unix-millisecond expiry boundary.
    #[must_use]
    pub const fn expires_at_ms(&self) -> u64 {
        self.expires_at_ms
    }

    pub(crate) fn associated_data(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            40 + self.purpose.len()
                + self.audience.len()
                + self.subject.len()
                + self.context_digest.len(),
        );
        out.extend_from_slice(b"sim/protected-state/binding/v1");
        for field in [
            &self.purpose,
            &self.audience,
            &self.subject,
            &self.context_digest,
        ] {
            out.extend_from_slice(&(field.len() as u32).to_be_bytes());
            out.extend_from_slice(field);
        }
        out.extend_from_slice(&self.expires_at_ms.to_be_bytes());
        out
    }
}
