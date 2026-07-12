//! Deterministic result-digest encoders for Category C cookbook recipes.
//!
//! COOKBOOK_7 Category C recipes produce effectful/floating artifacts (rendered
//! audio, encoded frames) whose raw form drifts across platforms and so cannot be
//! a stable `expect`. The convention is to encode the artifact as a DETERMINISTIC
//! digest: an audio buffer becomes `(audio-digest (samples N) (peak P) (rms R))`
//! over its integer-quantized samples, and an encoded frame becomes
//! `(frame (bytes N) (hash H))` over a fixed content hash. Both use only integer
//! math, so two runs of the same recipe reproduce byte-for-byte -- which is what
//! `cookbook-verify`'s twice-run determinism guard asserts.
//!
//! These helpers are kernel-free and pure; a Category C domain op quantizes its
//! artifact and calls one of them to yield its recipe result. The crate is
//! already a dependency of every recipe-shipping domain (via [`crate::write_embed`]),
//! so no new dependency is introduced.

/// The deterministic digest of a quantized audio buffer.
///
/// `samples` are integer-quantized PCM values (the caller scales floats to a
/// fixed integer range first, so the digest does not depend on float
/// representation). The digest reports the sample count, the peak absolute
/// amplitude, and the integer root-mean-square. All arithmetic is integer, so
/// the result is identical on every platform and every run.
///
/// The sum of squares accumulates in `u128` with a saturating add, so an
/// extreme buffer (many samples near `i64::MAX`) saturates to `u128::MAX`
/// rather than overflowing -- still deterministic. Callers quantize to a small
/// fixed range (e.g. 16-bit PCM), so saturation never occurs in practice.
///
/// # Examples
///
/// ```
/// # use sim_cookbook::audio_digest;
/// // sum of squares = 9 + 16 = 25; mean = 12; isqrt(12) = 3.
/// assert_eq!(
///     audio_digest(&[3, -4]),
///     "(audio-digest (samples 2) (peak 4) (rms 3))"
/// );
/// assert_eq!(audio_digest(&[]), "(audio-digest (samples 0) (peak 0) (rms 0))");
/// ```
pub fn audio_digest(samples: &[i64]) -> String {
    let n = samples.len();
    let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    let sum_sq: u128 = samples.iter().fold(0u128, |acc, s| {
        let a = i128::from(*s);
        acc.saturating_add((a * a) as u128)
    });
    let rms = if n == 0 {
        0
    } else {
        (sum_sq / n as u128).isqrt()
    };
    format!("(audio-digest (samples {n}) (peak {peak}) (rms {rms}))")
}

/// The deterministic content digest of an encoded frame's bytes.
///
/// Reports the byte length and a fixed 64-bit FNV-1a content hash (lowercase
/// hex). An encoded frame (a MIDI SMF chunk, a view surface frame, a stream
/// chunk) is already deterministic bytes; this renders a stable, compact summary
/// of it for use as a recipe result.
///
/// # Examples
///
/// ```
/// # use sim_cookbook::frame_digest;
/// // FNV-1a offset basis over an empty input is the basis itself.
/// assert_eq!(frame_digest(&[]), "(frame (bytes 0) (hash cbf29ce484222325))");
/// // Different content yields a different hash; the same content is stable.
/// assert_eq!(frame_digest(b"abc"), frame_digest(b"abc"));
/// assert_ne!(frame_digest(b"abc"), frame_digest(b"abd"));
/// ```
pub fn frame_digest(bytes: &[u8]) -> String {
    format!(
        "(frame (bytes {}) (hash {}))",
        bytes.len(),
        fnv1a64_hex(bytes)
    )
}

/// Compute the 64-bit FNV-1a hash for `bytes`.
///
/// This is the same fixed content hash used by [`frame_digest`], exposed so
/// other deterministic cookbook and recipe helpers can share one implementation
/// instead of carrying local copies.
///
/// # Examples
///
/// ```
/// # use sim_cookbook::fnv1a64;
/// assert_eq!(fnv1a64(&[]), 0xcbf2_9ce4_8422_2325);
/// assert_eq!(fnv1a64(b"abc"), 0xe71f_a219_0541_574b);
/// ```
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    bytes.iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

/// Compute the lowercase 16-digit hexadecimal FNV-1a digest for `bytes`.
///
/// # Examples
///
/// ```
/// # use sim_cookbook::fnv1a64_hex;
/// assert_eq!(fnv1a64_hex(&[]), "cbf29ce484222325");
/// ```
pub fn fnv1a64_hex(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a64(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_digest_uses_integer_rms_and_peak() {
        assert_eq!(
            audio_digest(&[3, -4]),
            "(audio-digest (samples 2) (peak 4) (rms 3))"
        );
        // Constant buffer: rms equals the magnitude.
        assert_eq!(
            audio_digest(&[-5, 5, -5, 5]),
            "(audio-digest (samples 4) (peak 5) (rms 5))"
        );
        assert_eq!(
            audio_digest(&[]),
            "(audio-digest (samples 0) (peak 0) (rms 0))"
        );
    }

    #[test]
    fn digests_are_deterministic_across_repeated_calls() {
        let samples = [1, -2, 3, -4, 5];
        assert_eq!(audio_digest(&samples), audio_digest(&samples));
        let bytes = b"a deterministic frame";
        assert_eq!(frame_digest(bytes), frame_digest(bytes));
    }

    #[test]
    fn frame_digest_is_content_sensitive() {
        assert_eq!(
            frame_digest(&[]),
            "(frame (bytes 0) (hash cbf29ce484222325))"
        );
        assert_eq!(frame_digest(b"abc"), frame_digest(b"abc"));
        assert_ne!(frame_digest(b"abc"), frame_digest(b"abd"));
        assert!(frame_digest(b"abc").starts_with("(frame (bytes 3) (hash "));
    }

    #[test]
    fn fnv1a64_uses_standard_basis_and_is_content_sensitive() {
        assert_eq!(fnv1a64(&[]), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64_hex(&[]), "cbf29ce484222325");
        assert_eq!(fnv1a64_hex(b"abc"), "e71fa2190541574b");
        assert_eq!(fnv1a64(b"abc"), fnv1a64(b"abc"));
        assert_ne!(fnv1a64(b"abc"), fnv1a64(b"abd"));
    }
}
