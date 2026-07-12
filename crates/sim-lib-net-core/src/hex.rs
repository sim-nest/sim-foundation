//! Lowercase hexadecimal encoding helpers.

/// Encode bytes as lowercase hexadecimal text.
///
/// # Examples
///
/// ```
/// use sim_lib_net_core::hex_encode;
///
/// assert_eq!(hex_encode(&[0xde, 0xad]), "dead");
/// ```
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::hex_encode;

    #[test]
    fn encodes_lowercase_hex() {
        assert_eq!(hex_encode(&[]), "");
        assert_eq!(hex_encode(&[0xde, 0xad]), "dead");
        assert_eq!(hex_encode(&[0x00, 0x0f, 0x10, 0xff]), "000f10ff");
    }
}
