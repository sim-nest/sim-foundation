//! Canonical semantic and byte-address identity roles.

use sha2::{Digest, Sha256};
use sim_kernel::{ContentId, Datum, Symbol};
use std::{collections::BTreeSet, fmt, marker::PhantomData};

use crate::ConformanceError;

/// Declares the exact canonical node tag for a semantic id role.
pub trait IdKind {
    /// Domain tag included in the semantic preimage.
    const DOMAIN: &'static str;
}

/// A role-safe semantic identity produced only from a canonical kernel datum.
pub struct SemanticId<K: IdKind> {
    raw: ContentId,
    marker: PhantomData<fn() -> K>,
}

impl<K: IdKind> Clone for SemanticId<K> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            marker: PhantomData,
        }
    }
}
impl<K: IdKind> PartialEq for SemanticId<K> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<K: IdKind> Eq for SemanticId<K> {}
impl<K: IdKind> PartialOrd for SemanticId<K> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<K: IdKind> Ord for SemanticId<K> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}
impl<K: IdKind> std::hash::Hash for SemanticId<K> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}
impl<K: IdKind> fmt::Debug for SemanticId<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(K::DOMAIN).field(&self.raw).finish()
    }
}

impl<K: IdKind> SemanticId<K> {
    /// Constructs this role from one bounded canonical text value.
    pub fn from_text(value: &str) -> Result<Self, ConformanceError> {
        if value.is_empty() || value.len() > 16_384 || value.chars().any(char::is_control) {
            return Err(ConformanceError::BoundExceeded("semantic id text"));
        }
        Self::from_fields(vec![field("value", text(value))?])
    }

    /// Constructs an id from the complete ordered fields of its declared node.
    pub fn from_fields(fields: Vec<(Symbol, Datum)>) -> Result<Self, ConformanceError> {
        reject_duplicate_fields(&fields)?;
        let raw = Datum::Node {
            tag: qualified(K::DOMAIN)?,
            fields,
        }
        .content_id()
        .map_err(|_| ConformanceError::NoncanonicalDatum)?;
        Ok(Self {
            raw,
            marker: PhantomData,
        })
    }

    /// Recomputes this role's id from a datum with the exact required tag.
    pub fn from_datum(datum: &Datum) -> Result<Self, ConformanceError> {
        let Datum::Node { tag, fields } = datum else {
            return Err(ConformanceError::WrongIdentityDomain);
        };
        if tag != &qualified(K::DOMAIN)? {
            return Err(ConformanceError::WrongIdentityDomain);
        }
        Self::from_fields(fields.clone())
    }

    /// Returns the canonical kernel identity without permitting role changes.
    pub const fn content_id(&self) -> &ContentId {
        &self.raw
    }

    /// Projects the typed id into a canonical datum field.
    pub fn to_datum(&self) -> Datum {
        content_id_datum(&self.raw)
    }
}

/// An opaque byte location. It cannot be converted to a semantic role.
///
/// ```compile_fail
/// use sim_conformance_core::{CheckedSubjectId, StorageId};
/// let location = StorageId::for_bytes(b"opaque");
/// let _: CheckedSubjectId = location;
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StorageId(ContentId);

impl StorageId {
    /// Addresses exact bytes under the registered storage byte law.
    pub fn for_bytes(bytes: &[u8]) -> Self {
        Self(ContentId::from_bytes(
            Symbol::qualified("storage", "sha256-bytes-v1"),
            Sha256::digest(bytes).into(),
        ))
    }

    /// Returns the byte-address identity.
    pub const fn content_id(&self) -> &ContentId {
        &self.0
    }

    /// Verifies retrieved bytes against this location.
    pub fn verify(&self, bytes: &[u8]) -> Result<(), ConformanceError> {
        if Self::for_bytes(bytes) == *self {
            Ok(())
        } else {
            Err(ConformanceError::StorageDigestMismatch)
        }
    }
}

/// A semantic object paired with its replaceable byte location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredSemanticObject<K: IdKind> {
    /// Meaning recomputed from the decoded value.
    pub meaning: SemanticId<K>,
    /// Exact encoded-byte location.
    pub location: StorageId,
}

/// Converts a content id to an ordinary canonical datum.
pub fn content_id_datum(id: &ContentId) -> Datum {
    Datum::Node {
        tag: Symbol::qualified("conformance", "content-id-v1"),
        fields: vec![
            (
                Symbol::qualified("conformance", "algorithm"),
                Datum::Symbol(id.algorithm.clone()),
            ),
            (
                Symbol::qualified("conformance", "digest"),
                Datum::Bytes(id.bytes.to_vec()),
            ),
        ],
    }
}

pub(crate) fn qualified(value: &str) -> Result<Symbol, ConformanceError> {
    let Some((namespace, name)) = value.split_once('/') else {
        return Symbol::checked(value).map_err(|_| ConformanceError::InvalidSymbol);
    };
    if namespace.is_empty() || name.is_empty() || name.contains('/') {
        return Err(ConformanceError::InvalidSymbol);
    }
    let namespace = Symbol::checked(namespace).map_err(|_| ConformanceError::InvalidSymbol)?;
    let name = Symbol::checked(name).map_err(|_| ConformanceError::InvalidSymbol)?;
    Ok(Symbol::qualified(namespace.name, name.name))
}

pub(crate) fn field(name: &str, value: Datum) -> Result<(Symbol, Datum), ConformanceError> {
    Ok((qualified(&format!("conformance/{name}"))?, value))
}

pub(crate) fn text(value: impl Into<String>) -> Datum {
    Datum::String(value.into())
}

pub(crate) fn ids<K: IdKind>(values: &[SemanticId<K>]) -> Datum {
    Datum::Vector(values.iter().map(SemanticId::to_datum).collect())
}

pub(crate) fn strings(values: impl IntoIterator<Item = String>) -> Datum {
    Datum::Vector(values.into_iter().map(Datum::String).collect())
}

fn reject_duplicate_fields(fields: &[(Symbol, Datum)]) -> Result<(), ConformanceError> {
    let mut seen = BTreeSet::new();
    for (name, _) in fields {
        if !seen.insert(name) {
            return Err(ConformanceError::DuplicateField(name.as_qualified_str()));
        }
    }
    Ok(())
}

macro_rules! id_kinds {
    ($($(#[$meta:meta])* $kind:ident, $alias:ident, $domain:literal;)+) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $kind;
        impl IdKind for $kind { const DOMAIN: &'static str = $domain; }
        $(#[$meta])*
        pub type $alias = SemanticId<$kind>;
    )+};
}

id_kinds! {
    /// Immutable owner binding identity.
    OwnerBindingKind, OwnerBindingId, "conformance/owner-binding-v1";
    /// Activation map identity.
    ActivationMapKind, ActivationMapId, "conformance/activation-map-v1";
    /// Dependency-use set identity.
    DependencyUseSetKind, DependencyUseSetId, "conformance/dependency-use-set-v1";
    /// Produced-surface set identity.
    SurfaceSetKind, SurfaceSetId, "conformance/surface-set-v1";
    /// Checked subject identity.
    CheckedSubjectKind, CheckedSubjectId, "conformance/checked-subject-v1";
    /// Checker binding identity.
    CheckerBindingKind, CheckerBindingId, "conformance/checker-binding-v1";
    /// Check template identity.
    CheckTemplateKind, CheckTemplateId, "conformance/check-template-v1";
    /// Check invocation identity.
    CheckInvocationKind, CheckInvocationId, "conformance/check-invocation-v1";
    /// Checker receipt identity.
    CheckerReceiptKind, CheckerReceiptId, "conformance/checker-receipt-v1";
    /// Checker result identity.
    CheckerResultKind, CheckerResultId, "conformance/checker-result-v1";
    /// Exact executable call identity.
    ExactCheckCallKind, ExactCheckCallId, "conformance/exact-check-call-v1";
    /// Input-closure identity.
    CheckInputClosureKind, CheckInputClosureId, "conformance/check-input-closure-v1";
    /// Checker-code identity.
    ProofCodeKind, ProofCodeId, "conformance/proof-code-v1";
    /// Pack identity.
    ConformancePackKind, ConformancePackId, "conformance/pack-v1";
    /// Scope identity.
    CheckScopeKind, CheckScopeId, "conformance/check-scope-v1";
    /// Command identity.
    CommandKind, CommandId, "conformance/command-v1";
    /// Working-directory policy identity.
    WorkingDirectoryPolicyKind, WorkingDirectoryPolicyId, "conformance/cwd-policy-v1";
    /// Environment policy identity.
    EnvironmentPolicyKind, EnvironmentPolicyId, "conformance/environment-policy-v1";
    /// Output Shape identity.
    OutputShapeKind, OutputShapeId, "conformance/output-shape-v1";
    /// Revocation source identity.
    RevocationSourceKind, RevocationSourceId, "conformance/revocation-source-v1";
    /// Evidence support set identity.
    EvidenceSetKind, EvidenceSetId, "conformance/evidence-set-v1";
    /// Evidence provenance identity.
    EvidenceProvenanceKind, EvidenceProvenanceId, "conformance/evidence-provenance-v1";
    /// Checker policy identity.
    PolicyKind, PolicyId, "conformance/policy-v1";
    /// Digest-construction record identity.
    DigestConstructionKind, DigestConstructionId, "conformance/digest-construction-v1";
    /// Complete digest register identity.
    DigestRegisterKind, DigestConstructionRegisterId, "conformance/digest-register-v1";
}
