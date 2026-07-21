//! Small shape predicates for index ids and keys.

/// Returns true when `value` is a stable index id.
pub fn is_index_id(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && segment.bytes().all(is_id_byte))
}

/// Returns true when `value` is a canonical feature key.
pub fn is_canonical_key(value: &str) -> bool {
    is_index_id(value)
}

fn is_id_byte(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
}
