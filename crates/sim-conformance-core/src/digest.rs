//! Complete digest-construction accountability records.

use std::collections::BTreeSet;

use sim_kernel::Datum;

use crate::{
    CheckedSubjectId, ConformanceError, ConformancePackId, DigestConstructionId,
    DigestConstructionRegisterId, OwnerBindingId, SemanticId, field, ids, qualified, strings, text,
};

/// The authority role a digest construction actually serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityRole {
    /// Durable semantic meaning.
    Semantic,
    /// Exact opaque bytes only.
    ByteAddress,
    /// Process-local optimization only.
    Ephemeral,
    /// Synthetic test data only.
    Fixture,
}

impl IdentityRole {
    fn as_str(self) -> &'static str {
        match self {
            Self::Semantic => "semantic",
            Self::ByteAddress => "byte-address",
            Self::Ephemeral => "ephemeral",
            Self::Fixture => "fixture",
        }
    }
}

/// Funded resolution for one current digest construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstructionDisposition {
    /// The construction already obeys its exact declared law.
    Conforming {
        /// Pack that checks the retained construction.
        pack: ConformancePackId,
    },
    /// A reached semantic construction must be normalized in the named phase.
    Normalize {
        /// Funded producing phase.
        phase: String,
        /// Target semantic domain.
        target_domain: String,
        /// Every affected consumer.
        affected_consumers: Vec<String>,
        /// Exact compatibility-plan declaration.
        compatibility: String,
    },
    /// An old law is allowed only inside one bounded native reader.
    VerifyNativeInputOnly {
        /// Sole bounded native entrypoint allowed to recognize the old id.
        reader: String,
    },
    /// The construction is outside the selected semantic closure.
    Excluded {
        /// Checked dataflow/exclusion witness.
        proof: CheckedSubjectId,
    },
}

impl ConstructionDisposition {
    fn datum(&self) -> Result<Datum, ConformanceError> {
        let (name, fields) = match self {
            Self::Conforming { pack } => ("conforming", vec![field("pack", pack.to_datum())?]),
            Self::Normalize {
                phase,
                target_domain,
                affected_consumers,
                compatibility,
            } => (
                "normalize",
                vec![
                    field("phase", text(phase.clone()))?,
                    field("target-domain", text(target_domain.clone()))?,
                    field("affected-consumers", strings(affected_consumers.clone()))?,
                    field("compatibility", text(compatibility.clone()))?,
                ],
            ),
            Self::VerifyNativeInputOnly { reader } => (
                "verify-native-input-only",
                vec![field("reader", text(reader.clone()))?],
            ),
            Self::Excluded { proof } => ("excluded", vec![field("proof", proof.to_datum())?]),
        };
        Ok(Datum::Node {
            tag: qualified(&format!("conformance/construction-{name}-v1"))?,
            fields,
        })
    }

    fn funded(&self) -> bool {
        match self {
            Self::Normalize {
                phase,
                target_domain,
                compatibility,
                ..
            } => !phase.is_empty() && !target_domain.is_empty() && !compatibility.is_empty(),
            Self::VerifyNativeInputOnly { reader } => !reader.is_empty(),
            _ => true,
        }
    }
}

/// One constructor family and every site supplying its preimage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigestConstruction {
    id: DigestConstructionId,
    /// Stable register key.
    pub key: String,
    /// Owning architecture binding.
    pub owner: OwnerBindingId,
    /// Exact implementation symbol.
    pub symbol: String,
    /// Source identity of the constructor definition.
    pub constructor: CheckedSubjectId,
    /// Source identities of all reached callers.
    pub call_sites: Vec<CheckedSubjectId>,
    /// Rust or wire result role.
    pub result_type: String,
    /// Complete preimage law.
    pub preimage: String,
    /// Actual authority role.
    pub use_role: IdentityRole,
    /// Dataflow or exclusion witness.
    pub reachability: CheckedSubjectId,
    /// Funded resolution.
    pub disposition: ConstructionDisposition,
}

impl DigestConstruction {
    /// Validates and identifies a construction row.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: String,
        owner: OwnerBindingId,
        symbol: String,
        constructor: CheckedSubjectId,
        mut call_sites: Vec<CheckedSubjectId>,
        result_type: String,
        preimage: String,
        use_role: IdentityRole,
        reachability: CheckedSubjectId,
        disposition: ConstructionDisposition,
    ) -> Result<Self, ConformanceError> {
        if key.is_empty() || symbol.is_empty() || result_type.is_empty() || preimage.is_empty() {
            return Err(ConformanceError::UnfundedConstruction(key));
        }
        if !disposition.funded() {
            return Err(ConformanceError::UnfundedConstruction(key));
        }
        call_sites.sort();
        if call_sites.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ConformanceError::DuplicateConstruction(key));
        }
        let id = SemanticId::from_fields(vec![
            field("key", text(key.clone()))?,
            field("owner", owner.to_datum())?,
            field("symbol", text(symbol.clone()))?,
            field("constructor", constructor.to_datum())?,
            field("call-sites", ids(&call_sites))?,
            field("result-type", text(result_type.clone()))?,
            field("preimage", text(preimage.clone()))?,
            field("use-role", text(use_role.as_str()))?,
            field("reachability", reachability.to_datum())?,
            field("disposition", disposition.datum()?)?,
        ])?;
        Ok(Self {
            id,
            key,
            owner,
            symbol,
            constructor,
            call_sites,
            result_type,
            preimage,
            use_role,
            reachability,
            disposition,
        })
    }

    /// Returns the construction identity.
    pub const fn id(&self) -> &DigestConstructionId {
        &self.id
    }
}

/// Complete, uniquely keyed digest-construction register.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigestConstructionRegister {
    id: DigestConstructionRegisterId,
    /// Sorted construction rows.
    pub constructions: Vec<DigestConstruction>,
}

impl DigestConstructionRegister {
    /// Sorts, checks, and identifies a complete register.
    pub fn new(mut constructions: Vec<DigestConstruction>) -> Result<Self, ConformanceError> {
        constructions.sort_by(|a, b| a.key.cmp(&b.key));
        let mut keys = BTreeSet::new();
        let mut constructors = BTreeSet::new();
        for row in &constructions {
            if !keys.insert(row.key.clone()) || !constructors.insert(row.constructor.clone()) {
                return Err(ConformanceError::DuplicateConstruction(row.key.clone()));
            }
        }
        let id = SemanticId::from_fields(vec![field(
            "constructions",
            ids(&constructions
                .iter()
                .map(|row| row.id.clone())
                .collect::<Vec<_>>()),
        )?])?;
        Ok(Self { id, constructions })
    }

    /// Returns the register identity.
    pub const fn id(&self) -> &DigestConstructionRegisterId {
        &self.id
    }
}
