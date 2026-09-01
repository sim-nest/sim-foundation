//! Canonical kernel-Datum helpers shared by the record shapes.

use crate::records::{MAX_ITEMS, MAX_TEXT, StudyError, VERSION};
use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};
use std::collections::BTreeSet;

pub(crate) fn sym(name: &str) -> Symbol {
    Symbol::qualified("study", name)
}
pub(crate) fn field(name: &str, value: Datum) -> (Symbol, Datum) {
    (sym(name), value)
}
pub(crate) fn node(name: &str, fields: Vec<(Symbol, Datum)>) -> Datum {
    Datum::Node {
        tag: sym(name),
        fields,
    }
}
pub(crate) fn version() -> (Symbol, Datum) {
    field("v", u32_datum(VERSION))
}
pub(crate) fn u32_datum(value: u32) -> Datum {
    Datum::Number(NumberLiteral {
        domain: Symbol::qualified("numbers", "u32"),
        canonical: value.to_string(),
    })
}
pub(crate) fn cid(value: &ContentId) -> Datum {
    node(
        "content-id",
        vec![
            field("algorithm", Datum::Symbol(value.algorithm.clone())),
            field("digest", Datum::Bytes(value.bytes.to_vec())),
        ],
    )
}

pub(crate) fn read_cid(value: &Datum, field_name: &'static str) -> Result<ContentId, StudyError> {
    let fields = exact_fields(value, "content-id", &["algorithm", "digest"])?;
    match (fields[0], fields[1]) {
        (Datum::Symbol(algorithm), Datum::Bytes(bytes)) if bytes.len() == 32 => {
            let mut digest = [0; 32];
            digest.copy_from_slice(bytes);
            Ok(ContentId::from_bytes(algorithm.clone(), digest))
        }
        _ => Err(StudyError::WrongField(field_name)),
    }
}

pub(crate) fn read_u32(value: &Datum, field_name: &'static str) -> Result<u32, StudyError> {
    match value {
        Datum::Number(number)
            if number.domain == Symbol::qualified("numbers", "u32")
                && !number.canonical.starts_with('+')
                && (number.canonical == "0" || !number.canonical.starts_with('0')) =>
        {
            number
                .canonical
                .parse()
                .map_err(|_| StudyError::BoundExceeded(field_name))
        }
        _ => Err(StudyError::WrongField(field_name)),
    }
}

pub(crate) fn exact_fields<'a>(
    value: &'a Datum,
    name: &'static str,
    expected: &[&str],
) -> Result<Vec<&'a Datum>, StudyError> {
    let Datum::Node { tag, fields } = value else {
        return Err(StudyError::NoncanonicalRecord(name));
    };
    if tag != &sym(name) || fields.len() != expected.len() {
        return Err(StudyError::NoncanonicalRecord(name));
    }
    let mut seen = BTreeSet::new();
    for ((actual, _), wanted) in fields.iter().zip(expected) {
        if !seen.insert(actual.clone()) {
            return Err(StudyError::DuplicateField(actual.to_string()));
        }
        if actual != &sym(wanted) {
            return Err(StudyError::NoncanonicalRecord(name));
        }
    }
    if expected.first() == Some(&"v") {
        let actual = read_u32(&fields[0].1, "v")?;
        if actual != VERSION {
            return Err(StudyError::UnknownSchemaVersion(actual));
        }
    }
    Ok(fields.iter().map(|(_, value)| value).collect())
}

pub(crate) fn bounded(value: &str, name: &'static str) -> Result<(), StudyError> {
    if value.len() > MAX_TEXT {
        Err(StudyError::BoundExceeded(name))
    } else {
        Ok(())
    }
}
pub(crate) fn bounded_items<T>(value: &[T], name: &'static str) -> Result<(), StudyError> {
    if value.len() > MAX_ITEMS {
        Err(StudyError::BoundExceeded(name))
    } else {
        Ok(())
    }
}
pub(crate) fn validate_number(value: &NumberLiteral) -> Result<(), StudyError> {
    bounded(&value.canonical, "number literal")?;
    if value.canonical.is_empty()
        || value.canonical.trim() != value.canonical
        || value.canonical.contains(['\n', '\r', '\t'])
    {
        Err(StudyError::NoncanonicalRecord("number literal"))
    } else {
        Ok(())
    }
}
pub(crate) fn validate_relative_path(path: &str) -> Result<(), StudyError> {
    bounded(path, "path")?;
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with('~')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.contains('\\')
    {
        Err(StudyError::UnsafePath)
    } else {
        Ok(())
    }
}
pub(crate) fn contains_secret_shape(value: &Datum) -> bool {
    match value {
        Datum::Node { tag, fields } => {
            tag == &sym("secret")
                || fields
                    .iter()
                    .any(|(key, value)| key == &sym("secret") || contains_secret_shape(value))
        }
        Datum::List(values) | Datum::Vector(values) | Datum::Set(values) => {
            values.iter().any(contains_secret_shape)
        }
        Datum::Map(values) => values
            .iter()
            .any(|(key, value)| contains_secret_shape(key) || contains_secret_shape(value)),
        _ => false,
    }
}
