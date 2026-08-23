//! Binding-authenticated, bounded opaque state envelopes for continuation protocols.
//!
//! This crate protects caller-owned bytes. It deliberately knows nothing about MCP,
//! payload serialization, canonical request digests, or production key storage.
//! [`ProtectedState`] receives a read-only [`KeyRing`], a secure [`NonceSource`],
//! and the platform [`WallClock`]. AEAD prevents undetected modification; replay
//! resistance exists only when a caller also uses
//! [`ConsumptionLedger`].
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::{fmt, sync::Arc};

use rand_core::{CryptoRng, Rng};
use sim_host_core::WallClock;
use zeroize::{Zeroize, Zeroizing};

mod binding;
mod crypto;
mod envelope;
mod ledger;

pub use binding::StateBinding;
use crypto::{open_xchacha20_poly1305, seal_xchacha20_poly1305};
use envelope::ParsedEnvelope;
pub use ledger::{ConsumptionError, ConsumptionLedger, TableConsumptionLedger};

/// Current binary-envelope format version.
pub const FORMAT_VERSION: u8 = 1;
/// Algorithm identifier for XChaCha20-Poly1305.
pub const ALGORITHM_XCHACHA20_POLY1305: u8 = 1;
/// XChaCha20 nonce size.
pub const NONCE_BYTES: usize = 24;
/// Symmetric-key size.
pub const KEY_BYTES: usize = 32;
/// Maximum caller plaintext size (one mebibyte).
pub const MAX_PLAINTEXT_BYTES: usize = 1_048_576;
/// Maximum UTF-8 key-id size.
pub const MAX_KEY_ID_BYTES: usize = 128;
/// Maximum size of each binding field.
pub const MAX_BINDING_FIELD_BYTES: usize = 4096;
/// Maximum accepted envelope size.
pub const MAX_ENVELOPE_BYTES: usize = MAX_PLAINTEXT_BYTES + 256;

const MAGIC: &[u8; 4] = b"SPS1";
const TAG_BYTES: usize = 16;
const FIXED_HEADER_BYTES: usize = 4 + 1 + 1 + 2 + 4;

/// A secret key copied out of a read-only key ring and zeroized on drop.
pub struct SecretKey(Zeroizing<[u8; KEY_BYTES]>);

impl SecretKey {
    /// Takes ownership of key bytes.
    #[must_use]
    pub fn new(bytes: [u8; KEY_BYTES]) -> Self {
        Self(Zeroizing::new(bytes))
    }
    fn bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }
}

/// Injected read-only key selection and retained-key lookup.
pub trait KeyRing: Send + Sync {
    /// Returns the bounded id selected for new envelopes.
    fn current_key_id(&self) -> Result<String, ProtectError>;
    /// Copies a configured key for use by one operation; retired or unknown ids return `None`.
    fn key(&self, key_id: &str) -> Result<Option<SecretKey>, ProtectError>;
}

/// Injected source of secure unique nonces.
pub trait NonceSource: Send + Sync {
    /// Fills one XChaCha20 nonce.
    fn fill_nonce(&self, nonce: &mut [u8; NONCE_BYTES]) -> Result<(), ProtectError>;
}

/// Mutex-serialized adapter for a reviewed `CryptoRng` implementation.
pub struct CryptoNonceSource<R>(std::sync::Mutex<R>);

impl<R> CryptoNonceSource<R> {
    /// Wraps an injected RNG without creating or persisting one.
    #[must_use]
    pub fn new(rng: R) -> Self {
        Self(std::sync::Mutex::new(rng))
    }
}

impl<R: Rng + CryptoRng + Send> NonceSource for CryptoNonceSource<R> {
    fn fill_nonce(&self, nonce: &mut [u8; NONCE_BYTES]) -> Result<(), ProtectError> {
        self.0
            .lock()
            .map_err(|_| ProtectError::Dependency)?
            .fill_bytes(nonce);
        Ok(())
    }
}

/// Construction/protection failure. Variants reveal no secret material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectError {
    /// A documented input bound was exceeded.
    LimitExceeded,
    /// An injected key, clock, or nonce dependency failed.
    Dependency,
    /// The selected key id was invalid or had no configured key.
    KeyUnavailable,
}
impl fmt::Display for ProtectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LimitExceeded => "protected state input exceeds limit",
            Self::Dependency => "protected state dependency failed",
            Self::KeyUnavailable => "protected state key unavailable",
        })
    }
}
impl std::error::Error for ProtectError {}

/// Uniform open failure for malformed, unauthentic, misbound, expired, unknown-key,
/// unsupported-version, and unsupported-algorithm envelopes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenError;
impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("protected state rejected")
    }
}
impl std::error::Error for OpenError {}

/// Opened secret bytes, zeroized when dropped.
#[derive(Debug)]
pub struct SecretBytes(Zeroizing<Vec<u8>>);
impl SecretBytes {
    /// Borrows the plaintext.
    #[must_use]
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}

/// Protect/open service assembled entirely from injected capabilities.
pub struct ProtectedState {
    keys: Arc<dyn KeyRing>,
    nonces: Arc<dyn NonceSource>,
    clock: Arc<dyn WallClock>,
}

impl ProtectedState {
    /// Composes a service. No key or RNG is generated or persisted here.
    #[must_use]
    pub fn new(
        keys: Arc<dyn KeyRing>,
        nonces: Arc<dyn NonceSource>,
        clock: Arc<dyn WallClock>,
    ) -> Self {
        Self {
            keys,
            nonces,
            clock,
        }
    }

    /// Protects caller-serialized plaintext under the exact supplied binding.
    pub fn protect(
        &self,
        plaintext: &[u8],
        binding: &StateBinding,
    ) -> Result<Vec<u8>, ProtectError> {
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(ProtectError::LimitExceeded);
        }
        let key_id = self.keys.current_key_id()?;
        if key_id.is_empty() || key_id.len() > MAX_KEY_ID_BYTES {
            return Err(ProtectError::KeyUnavailable);
        }
        let key = self
            .keys
            .key(&key_id)?
            .ok_or(ProtectError::KeyUnavailable)?;
        let mut nonce = [0_u8; NONCE_BYTES];
        self.nonces.fill_nonce(&mut nonce)?;
        let ciphertext =
            seal_xchacha20_poly1305(key.bytes(), &nonce, plaintext, &binding.associated_data())
                .map_err(|_| ProtectError::Dependency)?;
        let mut out =
            Vec::with_capacity(FIXED_HEADER_BYTES + key_id.len() + NONCE_BYTES + ciphertext.len());
        out.extend_from_slice(MAGIC);
        out.push(FORMAT_VERSION);
        out.push(ALGORITHM_XCHACHA20_POLY1305);
        out.extend_from_slice(&(key_id.len() as u16).to_be_bytes());
        out.extend_from_slice(&(ciphertext.len() as u32).to_be_bytes());
        out.extend_from_slice(key_id.as_bytes());
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        nonce.zeroize();
        Ok(out)
    }

    /// Opens into zeroizing secret bytes. All rejection causes share one result.
    pub fn open(&self, envelope: &[u8], binding: &StateBinding) -> Result<SecretBytes, OpenError> {
        let parsed = ParsedEnvelope::parse(envelope)?;
        let now = self.clock.now_ms().map_err(|_| OpenError)?;
        if now >= binding.expires_at_ms {
            return Err(OpenError);
        }
        let key = self
            .keys
            .key(parsed.key_id)
            .map_err(|_| OpenError)?
            .ok_or(OpenError)?;
        let plaintext = open_xchacha20_poly1305(
            key.bytes(),
            parsed.nonce.try_into().map_err(|_| OpenError)?,
            parsed.ciphertext,
            &binding.associated_data(),
        )?;
        if plaintext.len() > MAX_PLAINTEXT_BYTES {
            return Err(OpenError);
        }
        Ok(SecretBytes(plaintext))
    }
}

/// Embedded checked cookbook recipe tree.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
