use crate::{DomainId, FieldName};
use sim_kernel::{ContentId, Datum, Symbol};
use std::fmt;

/// Canonical projection shared by identity, Card data, and Lisp data faces.
pub trait ToRelationDatum {
    /// Returns the record's single fixed `Datum::Node` projection.
    fn to_datum(&self) -> Datum;
    /// Returns the same ordinary-data projection for Card consumers.
    fn card_datum(&self) -> Datum {
        self.to_datum()
    }
    /// Returns the same ordinary-data projection for Lisp codecs.
    fn lisp_datum(&self) -> Datum {
        self.to_datum()
    }
}

/// Typed identity for any relational record.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RelationId(ContentId);
impl RelationId {
    /// Projects and hashes a record with the kernel algorithm.
    pub fn of(value: &impl ToRelationDatum) -> Result<Self, sim_kernel::Error> {
        Ok(Self(value.to_datum().content_id()?))
    }
    /// Returns the kernel content id.
    pub const fn content_id(&self) -> &ContentId {
        &self.0
    }
}

/// A cell: `None` is typed SQL NULL; `Some` contains ordinary SIM data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    domain: DomainId,
    value: Option<Datum>,
}
impl Cell {
    /// Constructs a typed cell.
    pub const fn new(domain: DomainId, value: Option<Datum>) -> Self {
        Self { domain, value }
    }
    /// Constructs typed NULL.
    pub const fn null(domain: DomainId) -> Self {
        Self::new(domain, None)
    }
    /// Returns the logical domain.
    pub const fn domain(&self) -> &DomainId {
        &self.domain
    }
    /// Returns the optional ordinary datum.
    pub const fn value(&self) -> Option<&Datum> {
        self.value.as_ref()
    }
}

/// A named field's logical type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldType {
    /// Field name.
    pub name: FieldName,
    /// Logical domain.
    pub domain: DomainId,
    /// Whether absence is accepted.
    pub nullable: bool,
}
/// A validated ordered row type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowType(Vec<FieldType>);
/// Row construction failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowError {
    /// Field names repeat.
    DuplicateField(FieldName),
    /// Cell count differs.
    Arity {
        /// Expected cells.
        expected: usize,
        /// Actual cells.
        actual: usize,
    },
    /// Cell's domain differs.
    Domain {
        /// Field offset.
        index: usize,
    },
    /// NULL occurred in a non-nullable field.
    Null {
        /// Field offset.
        index: usize,
    },
}
impl fmt::Display for RowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RowError {}
impl RowType {
    /// Validates unique field names.
    pub fn new(fields: impl IntoIterator<Item = FieldType>) -> Result<Self, RowError> {
        let fields: Vec<_> = fields.into_iter().collect();
        for (i, v) in fields.iter().enumerate() {
            if fields[..i].iter().any(|x| x.name == v.name) {
                return Err(RowError::DuplicateField(v.name.clone()));
            }
        }
        Ok(Self(fields))
    }
    /// Returns ordered fields.
    pub fn fields(&self) -> &[FieldType] {
        &self.0
    }
}
/// A row checked against a row type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    row_type: RowType,
    cells: Vec<Cell>,
}
impl Row {
    /// Checks arity, domains, and typed NULLability.
    pub fn new(row_type: RowType, cells: impl IntoIterator<Item = Cell>) -> Result<Self, RowError> {
        let cells: Vec<_> = cells.into_iter().collect();
        if cells.len() != row_type.0.len() {
            return Err(RowError::Arity {
                expected: row_type.0.len(),
                actual: cells.len(),
            });
        }
        for (i, (field, cell)) in row_type.0.iter().zip(&cells).enumerate() {
            if field.domain != cell.domain {
                return Err(RowError::Domain { index: i });
            }
            if !field.nullable && cell.value.is_none() {
                return Err(RowError::Null { index: i });
            }
        }
        Ok(Self { row_type, cells })
    }
    /// Returns the row type.
    pub const fn row_type(&self) -> &RowType {
        &self.row_type
    }
    /// Returns ordered cells.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }
}

impl ToRelationDatum for Cell {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation", "cell"),
            fields: vec![
                (
                    Symbol::new("domain"),
                    Datum::Symbol(self.domain.symbol().clone()),
                ),
                (
                    Symbol::new("value"),
                    self.value.clone().unwrap_or(Datum::Nil),
                ),
            ],
        }
    }
}
impl ToRelationDatum for FieldType {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation", "field-type"),
            fields: vec![
                (
                    Symbol::new("name"),
                    Datum::Symbol(self.name.symbol().clone()),
                ),
                (
                    Symbol::new("domain"),
                    Datum::Symbol(self.domain.symbol().clone()),
                ),
                (Symbol::new("nullable"), Datum::Bool(self.nullable)),
            ],
        }
    }
}
impl ToRelationDatum for RowType {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation", "row-type"),
            fields: vec![(
                Symbol::new("fields"),
                Datum::Vector(self.0.iter().map(ToRelationDatum::to_datum).collect()),
            )],
        }
    }
}
impl ToRelationDatum for Row {
    fn to_datum(&self) -> Datum {
        Datum::Node {
            tag: Symbol::qualified("relation", "row"),
            fields: vec![
                (Symbol::new("type"), self.row_type.to_datum()),
                (
                    Symbol::new("cells"),
                    Datum::Vector(self.cells.iter().map(ToRelationDatum::to_datum).collect()),
                ),
            ],
        }
    }
}
