use crate::{
    ALGORITHM_XCHACHA20_POLY1305, FIXED_HEADER_BYTES, FORMAT_VERSION, MAGIC, MAX_ENVELOPE_BYTES,
    MAX_KEY_ID_BYTES, MAX_PLAINTEXT_BYTES, NONCE_BYTES, OpenError, TAG_BYTES,
};

pub(crate) struct ParsedEnvelope<'a> {
    pub(crate) key_id: &'a str,
    pub(crate) nonce: &'a [u8],
    pub(crate) ciphertext: &'a [u8],
}

impl<'a> ParsedEnvelope<'a> {
    pub(crate) fn parse(input: &'a [u8]) -> Result<Self, OpenError> {
        if input.len() < FIXED_HEADER_BYTES + NONCE_BYTES + TAG_BYTES
            || input.len() > MAX_ENVELOPE_BYTES
        {
            return Err(OpenError);
        }
        if &input[..4] != MAGIC
            || input[4] != FORMAT_VERSION
            || input[5] != ALGORITHM_XCHACHA20_POLY1305
        {
            return Err(OpenError);
        }
        let key_len = u16::from_be_bytes([input[6], input[7]]) as usize;
        let cipher_len = u32::from_be_bytes([input[8], input[9], input[10], input[11]]) as usize;
        if key_len == 0
            || key_len > MAX_KEY_ID_BYTES
            || !(TAG_BYTES..=MAX_PLAINTEXT_BYTES + TAG_BYTES).contains(&cipher_len)
        {
            return Err(OpenError);
        }
        let expected = FIXED_HEADER_BYTES
            .checked_add(key_len)
            .and_then(|n| n.checked_add(NONCE_BYTES))
            .and_then(|n| n.checked_add(cipher_len))
            .ok_or(OpenError)?;
        if input.len() != expected {
            return Err(OpenError);
        }
        let key_end = FIXED_HEADER_BYTES + key_len;
        let nonce_end = key_end + NONCE_BYTES;
        let key_id =
            std::str::from_utf8(&input[FIXED_HEADER_BYTES..key_end]).map_err(|_| OpenError)?;
        Ok(Self {
            key_id,
            nonce: &input[key_end..nonce_end],
            ciphertext: &input[nonce_end..],
        })
    }
}
