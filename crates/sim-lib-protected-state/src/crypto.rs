use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, Nonce, UnboundKey};
use zeroize::{Zeroize, Zeroizing};

use crate::{KEY_BYTES, NONCE_BYTES, OpenError};

pub(crate) fn seal_xchacha20_poly1305(
    key: &[u8; KEY_BYTES],
    nonce: &[u8; NONCE_BYTES],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Vec<u8>, ()> {
    let aead = xchacha_aead(key, nonce)?;
    let mut in_out = plaintext.to_vec();
    aead.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(ietf_nonce(nonce)),
        Aad::from(aad),
        &mut in_out,
    )
    .map_err(|_| ())?;
    Ok(in_out)
}

pub(crate) fn open_xchacha20_poly1305(
    key: &[u8; KEY_BYTES],
    nonce: &[u8; NONCE_BYTES],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, OpenError> {
    let aead = xchacha_aead(key, nonce).map_err(|_| OpenError)?;
    let mut in_out = Zeroizing::new(ciphertext.to_vec());
    let plaintext_len = {
        let plaintext = aead
            .open_in_place(
                Nonce::assume_unique_for_key(ietf_nonce(nonce)),
                Aad::from(aad),
                in_out.as_mut(),
            )
            .map_err(|_| OpenError)?;
        plaintext.len()
    };
    in_out.truncate(plaintext_len);
    Ok(in_out)
}

fn xchacha_aead(key: &[u8; KEY_BYTES], nonce: &[u8; NONCE_BYTES]) -> Result<LessSafeKey, ()> {
    let mut subkey = hchacha20(key, nonce[..16].try_into().map_err(|_| ())?);
    let key = UnboundKey::new(&CHACHA20_POLY1305, &subkey).map(LessSafeKey::new);
    subkey.zeroize();
    key.map_err(|_| ())
}

fn ietf_nonce(nonce: &[u8; NONCE_BYTES]) -> [u8; 12] {
    let mut out = [0_u8; 12];
    out[4..].copy_from_slice(&nonce[16..]);
    out
}

fn hchacha20(key: &[u8; KEY_BYTES], nonce: &[u8; 16]) -> [u8; KEY_BYTES] {
    let constants = [
        u32::from_le_bytes(*b"expa"),
        u32::from_le_bytes(*b"nd 3"),
        u32::from_le_bytes(*b"2-by"),
        u32::from_le_bytes(*b"te k"),
    ];
    let mut state = [0_u32; 16];
    state[..4].copy_from_slice(&constants);
    for (slot, bytes) in state[4..12].iter_mut().zip(key.chunks_exact(4)) {
        *slot = u32::from_le_bytes(bytes.try_into().expect("chunk is four bytes"));
    }
    for (slot, bytes) in state[12..].iter_mut().zip(nonce.chunks_exact(4)) {
        *slot = u32::from_le_bytes(bytes.try_into().expect("chunk is four bytes"));
    }
    for _ in 0..10 {
        quarter_round(&mut state, 0, 4, 8, 12);
        quarter_round(&mut state, 1, 5, 9, 13);
        quarter_round(&mut state, 2, 6, 10, 14);
        quarter_round(&mut state, 3, 7, 11, 15);
        quarter_round(&mut state, 0, 5, 10, 15);
        quarter_round(&mut state, 1, 6, 11, 12);
        quarter_round(&mut state, 2, 7, 8, 13);
        quarter_round(&mut state, 3, 4, 9, 14);
    }
    let words = [
        state[0], state[1], state[2], state[3], state[12], state[13], state[14], state[15],
    ];
    let mut out = [0_u8; KEY_BYTES];
    for (chunk, word) in out.chunks_exact_mut(4).zip(words) {
        chunk.copy_from_slice(&word.to_le_bytes());
    }
    state.zeroize();
    out
}

fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}
