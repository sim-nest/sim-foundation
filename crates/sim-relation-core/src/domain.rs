use crate::{DomainId, ToRelationDatum};
use sim_kernel::{Datum, NumberLiteral, Ref, Symbol};
use std::{collections::BTreeMap, fmt};

/// Exact physical representation requested from a storage provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageRepr {
    /// Boolean bit.
    Bool,
    /// Signed 64-bit integer.
    I64,
    /// Finite IEEE-754 binary64 value.
    F64,
    /// UTF-8 text.
    Text,
    /// Arbitrary bytes.
    Bytes,
}

/// An exact value at the provider boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum StorageValue {
    /// Boolean value.
    Bool(bool),
    /// Signed integer value.
    I64(i64),
    /// Finite float value (negative zero is normalized).
    F64(f64),
    /// Text value.
    Text(String),
    /// Byte value.
    Bytes(Vec<u8>),
}

/// A semantic promise made by a logical domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DomainTrait {
    /// Equality is supported.
    Equatable,
    /// Total ordering is supported.
    Ordered,
}

/// Domain validation or conversion failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    /// Two specs used the same id.
    DuplicateId(DomainId),
    /// A Shape reference cannot be resolved durably.
    InvalidShapeRef,
    /// Traits contradict one another.
    IncoherentTraits,
    /// A storage value used the wrong representation.
    StorageMismatch,
    /// A floating value was not finite.
    NonFiniteFloat,
    /// A Datum did not exactly represent this base domain.
    DatumMismatch,
}
impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for DomainError {}

/// An open logical-domain declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainSpec {
    id: DomainId,
    storage: StorageRepr,
    shape: Ref,
    traits: Vec<DomainTrait>,
}
impl DomainSpec {
    /// Constructs and validates a domain declaration.
    pub fn new(
        id: DomainId,
        storage: StorageRepr,
        shape: Ref,
        traits: impl IntoIterator<Item = DomainTrait>,
    ) -> Result<Self, DomainError> {
        if matches!(shape, Ref::Handle(_) | Ref::Coord(_)) {
            return Err(DomainError::InvalidShapeRef);
        }
        let mut traits: Vec<_> = traits.into_iter().collect();
        traits.sort();
        traits.dedup();
        if traits.contains(&DomainTrait::Ordered) && !traits.contains(&DomainTrait::Equatable) {
            return Err(DomainError::IncoherentTraits);
        }
        Ok(Self {
            id,
            storage,
            shape,
            traits,
        })
    }
    /// Returns the domain id.
    pub fn id(&self) -> &DomainId {
        &self.id
    }
    /// Returns its physical storage representation.
    pub const fn storage(&self) -> StorageRepr {
        self.storage
    }
    /// Returns its unresolved Shape reference.
    pub const fn shape(&self) -> &Ref {
        &self.shape
    }
    /// Returns its normalized semantic traits.
    pub fn traits(&self) -> &[DomainTrait] {
        &self.traits
    }
}

/// A validated collection of open domain declarations.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DomainCatalog(BTreeMap<DomainId, DomainSpec>);
impl DomainCatalog {
    /// Validates and builds a catalog.
    pub fn new(specs: impl IntoIterator<Item = DomainSpec>) -> Result<Self, DomainError> {
        let mut map = BTreeMap::new();
        for spec in specs {
            let id = spec.id.clone();
            if map.insert(id.clone(), spec).is_some() {
                return Err(DomainError::DuplicateId(id));
            }
        }
        Ok(Self(map))
    }
    /// Looks up a declaration without provider-specific dispatch.
    pub fn get(&self, id: &DomainId) -> Option<&DomainSpec> {
        self.0.get(id)
    }
    /// Iterates in stable domain-id order.
    pub fn iter(&self) -> impl Iterator<Item = &DomainSpec> {
        self.0.values()
    }
}

impl ToRelationDatum for DomainCatalog {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation", "domain-catalog"),
            fields: vec![(
                Symbol::new("domains"),
                Datum::Vector(self.iter().map(ToRelationDatum::to_datum).collect()),
            )],
        }
    }
}

/// The five portable base domains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseDomain {
    /// Boolean.
    Bool,
    /// Signed 64-bit integer.
    I64,
    /// Finite binary64 float.
    F64,
    /// UTF-8 text.
    Text,
    /// Bytes.
    Bytes,
}
impl BaseDomain {
    /// Returns the stable open domain id.
    pub fn id(self) -> DomainId {
        DomainId::new(Symbol::qualified(
            "relation",
            match self {
                Self::Bool => "bool",
                Self::I64 => "i64",
                Self::F64 => "f64",
                Self::Text => "text",
                Self::Bytes => "bytes",
            },
        ))
        .expect("built-in id")
    }
    /// Returns the complete built-in declaration, suitable for any catalog.
    pub fn spec(self) -> DomainSpec {
        let storage = match self {
            Self::Bool => StorageRepr::Bool,
            Self::I64 => StorageRepr::I64,
            Self::F64 => StorageRepr::F64,
            Self::Text => StorageRepr::Text,
            Self::Bytes => StorageRepr::Bytes,
        };
        DomainSpec::new(
            self.id(),
            storage,
            Ref::Symbol(Symbol::qualified(
                "relation",
                match self {
                    Self::Bool => "BoolShape",
                    Self::I64 => "I64Shape",
                    Self::F64 => "FiniteF64Shape",
                    Self::Text => "TextShape",
                    Self::Bytes => "BytesShape",
                },
            )),
            [DomainTrait::Equatable, DomainTrait::Ordered],
        )
        .expect("built-in domain declaration is coherent")
    }
    /// Converts a provider value into its exact kernel datum.
    pub fn to_datum(self, value: StorageValue) -> Result<Datum, DomainError> {
        match (self, value) {
            (Self::Bool, StorageValue::Bool(v)) => Ok(Datum::Bool(v)),
            (Self::I64, StorageValue::I64(v)) => Ok(Datum::Number(NumberLiteral {
                domain: Symbol::qualified("core", "i64"),
                canonical: v.to_string(),
            })),
            (Self::F64, StorageValue::F64(v)) if v.is_finite() => {
                let v = if v == 0.0 { 0.0 } else { v };
                Ok(Datum::Number(NumberLiteral {
                    domain: Symbol::qualified("core", "f64"),
                    canonical: v.to_string(),
                }))
            }
            (Self::F64, StorageValue::F64(_)) => Err(DomainError::NonFiniteFloat),
            (Self::Text, StorageValue::Text(v)) => Ok(Datum::String(v)),
            (Self::Bytes, StorageValue::Bytes(v)) => Ok(Datum::Bytes(v)),
            _ => Err(DomainError::StorageMismatch),
        }
    }
    /// Converts an exact kernel datum back to the provider representation.
    pub fn from_datum(self, datum: &Datum) -> Result<StorageValue, DomainError> {
        match (self, datum) {
            (Self::Bool, Datum::Bool(v)) => Ok(StorageValue::Bool(*v)),
            (Self::I64, Datum::Number(v)) if v.domain == Symbol::qualified("core", "i64") => v
                .canonical
                .parse()
                .map(StorageValue::I64)
                .map_err(|_| DomainError::DatumMismatch),
            (Self::F64, Datum::Number(v)) if v.domain == Symbol::qualified("core", "f64") => {
                let n: f64 = v
                    .canonical
                    .parse()
                    .map_err(|_| DomainError::DatumMismatch)?;
                if !n.is_finite() {
                    Err(DomainError::NonFiniteFloat)
                } else {
                    Ok(StorageValue::F64(if n == 0.0 { 0.0 } else { n }))
                }
            }
            (Self::Text, Datum::String(v)) => Ok(StorageValue::Text(v.clone())),
            (Self::Bytes, Datum::Bytes(v)) => Ok(StorageValue::Bytes(v.clone())),
            _ => Err(DomainError::DatumMismatch),
        }
    }
}

impl ToRelationDatum for DomainSpec {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation", "domain"),
            fields: vec![
                (Symbol::new("id"), Datum::Symbol(self.id.symbol().clone())),
                (
                    Symbol::new("storage"),
                    Datum::Symbol(Symbol::qualified(
                        "relation",
                        match self.storage {
                            StorageRepr::Bool => "bool",
                            StorageRepr::I64 => "i64",
                            StorageRepr::F64 => "f64",
                            StorageRepr::Text => "text",
                            StorageRepr::Bytes => "bytes",
                        },
                    )),
                ),
                (Symbol::new("shape"), ref_datum(&self.shape)),
                (
                    Symbol::new("traits"),
                    Datum::Vector(
                        self.traits
                            .iter()
                            .map(|v| {
                                Datum::Symbol(Symbol::qualified(
                                    "relation",
                                    match v {
                                        DomainTrait::Equatable => "equatable",
                                        DomainTrait::Ordered => "ordered",
                                    },
                                ))
                            })
                            .collect(),
                    ),
                ),
            ],
        }
    }
}
fn ref_datum(value: &Ref) -> Datum {
    match value {
        Ref::Symbol(v) => Datum::Node {
            tag: Symbol::qualified("core", "ref-symbol"),
            fields: vec![(Symbol::new("symbol"), Datum::Symbol(v.clone()))],
        },
        Ref::Content(v) => Datum::Node {
            tag: Symbol::qualified("core", "ref-content"),
            fields: vec![
                (Symbol::new("algorithm"), Datum::Symbol(v.algorithm.clone())),
                (Symbol::new("bytes"), Datum::Bytes(v.bytes.to_vec())),
            ],
        },
        _ => unreachable!("validated durable ref"),
    }
}
