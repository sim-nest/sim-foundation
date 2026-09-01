use sim_kernel::Symbol;
use std::fmt;

/// Validation failure for a relational symbolic name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameError {
    /// A symbol component was empty.
    Empty,
    /// A symbol component contained a control character.
    Control,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "relational names cannot be empty",
            Self::Control => "relational names cannot contain control characters",
        })
    }
}
impl std::error::Error for NameError {}

fn validate(symbol: &Symbol) -> Result<(), NameError> {
    if symbol.name.is_empty() || symbol.namespace.as_ref().is_some_and(|v| v.is_empty()) {
        return Err(NameError::Empty);
    }
    if symbol.name.chars().any(char::is_control)
        || symbol
            .namespace
            .as_ref()
            .is_some_and(|v| v.chars().any(char::is_control))
    {
        return Err(NameError::Control);
    }
    Ok(())
}

macro_rules! name_type {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Symbol);
        impl $name {
            /// Validates and constructs the name. SQL punctuation and keywords are accepted.
            pub fn new(symbol: Symbol) -> Result<Self, NameError> {
                validate(&symbol)?;
                Ok(Self(symbol))
            }
            /// Returns the underlying open symbol.
            pub fn symbol(&self) -> &Symbol {
                &self.0
            }
        }
        impl TryFrom<Symbol> for $name {
            type Error = NameError;
            fn try_from(value: Symbol) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
        impl From<$name> for Symbol {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

name_type!(TableName, "A table name.");
name_type!(ColumnName, "A column name.");
name_type!(SourceName, "A relational source name.");
name_type!(BindingName, "A query binding name.");
name_type!(FieldName, "A projected field name.");
name_type!(ParameterName, "A parameter name.");
name_type!(ConstraintName, "A constraint name.");
name_type!(IndexName, "An index name.");
name_type!(RevisionName, "A revision name.");
name_type!(ProviderName, "A storage provider name.");
name_type!(DomainId, "An open logical-domain identifier.");
name_type!(SchemaName, "A logical schema name.");
name_type!(ViewName, "A relational view name.");
