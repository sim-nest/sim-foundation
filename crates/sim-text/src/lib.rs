//! Exact UTF-16 code-unit text with explicit scalar projections.
//!
//! [`CodeUnitString`] preserves every `u16`, including lone surrogates and NUL.
//! It deliberately carries no JavaScript, JVM, codec, or interning policy.

use core::fmt;
use core::ops::Range;

/// The largest code-unit allocation representable by Rust's collection APIs.
///
/// This bound also makes multiplication by the size of a code unit safe.
pub const MAX_CODE_UNITS: usize = isize::MAX as usize / size_of::<u16>();

/// An offset in the exact UTF-16 code-unit sequence.
///
/// This is intentionally not interchangeable with [`ScalarOffset`].
///
/// ```compile_fail
/// use sim_text::{CodeUnitOffset, CodeUnitString, ScalarOffset};
/// let text = CodeUnitString::from_scalar("abc");
/// let scalar = ScalarOffset::new(1);
/// let _ = text.code_unit_at(scalar);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CodeUnitOffset(usize);

impl CodeUnitOffset {
    /// Construct an offset. Bounds are checked when it is used with a value.
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Return the numeric code-unit offset.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// An offset in the Unicode scalar sequence.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarOffset(usize);

impl ScalarOffset {
    /// Construct an offset. Bounds are checked during conversion.
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    /// Return the numeric scalar offset.
    pub const fn get(self) -> usize {
        self.0
    }
}

/// A half-open range in the exact UTF-16 code-unit sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CodeUnitRange {
    /// Inclusive start offset.
    pub start: CodeUnitOffset,
    /// Exclusive end offset.
    pub end: CodeUnitOffset,
}

impl CodeUnitRange {
    /// Construct a half-open code-unit range.
    pub const fn new(start: CodeUnitOffset, end: CodeUnitOffset) -> Self {
        Self { start, end }
    }
}

/// A half-open range in the Unicode scalar sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScalarRange {
    /// Inclusive start offset.
    pub start: ScalarOffset,
    /// Exclusive end offset.
    pub end: ScalarOffset,
}

impl ScalarRange {
    /// Construct a half-open scalar range.
    pub const fn new(start: ScalarOffset, end: ScalarOffset) -> Self {
        Self { start, end }
    }
}

/// Located evidence for the first invalid UTF-16 code unit.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InvalidSurrogate {
    /// Code-unit offset of the invalid surrogate.
    pub offset: CodeUnitOffset,
    /// Invalid raw code unit.
    pub unit: u16,
}

/// Failure to construct or project an exact code-unit string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeUnitStringError {
    /// The requested code-unit count cannot be represented by an allocation.
    TooLong {
        /// Requested number of code units.
        len: usize,
        /// Maximum supported number of code units.
        max: usize,
    },
    /// The unit sequence contains an unpaired surrogate.
    LoneSurrogate(InvalidSurrogate),
}

impl fmt::Display for CodeUnitStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLong { len, max } => {
                write!(
                    f,
                    "code-unit length {len} exceeds the supported maximum {max}"
                )
            }
            Self::LoneSurrogate(invalid) => {
                write!(
                    f,
                    "lone surrogate {:#06x} at code-unit index {}",
                    invalid.unit,
                    invalid.offset.get()
                )
            }
        }
    }
}

impl std::error::Error for CodeUnitStringError {}

/// An exact sequence of UTF-16 code units.
///
/// Unlike [`String`], this value admits lone surrogates. Conversion to scalar
/// Unicode is therefore explicit and checked.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub struct CodeUnitString {
    units: Vec<u16>,
}

impl CodeUnitString {
    /// Encode scalar Unicode text as UTF-16 code units.
    pub fn from_scalar(text: &str) -> Self {
        Self {
            units: text.encode_utf16().collect(),
        }
    }

    /// Preserve an exact sequence, rejecting an unrepresentable allocation.
    pub fn try_from_code_units(units: Vec<u16>) -> Result<Self, CodeUnitStringError> {
        if units.len() > MAX_CODE_UNITS {
            return Err(CodeUnitStringError::TooLong {
                len: units.len(),
                max: MAX_CODE_UNITS,
            });
        }
        Ok(Self { units })
    }

    /// Preserve an exact sequence including lone surrogates.
    ///
    /// A Rust `Vec<u16>` that already exists necessarily satisfies the
    /// allocation limit, so this compatibility constructor is infallible.
    pub fn from_code_units(units: Vec<u16>) -> Self {
        Self::try_from_code_units(units).expect("an existing Vec satisfies allocation limits")
    }

    /// Borrow all exact code units.
    pub fn as_code_units(&self) -> &[u16] {
        &self.units
    }

    /// Return the length in code units.
    pub fn len(&self) -> usize {
        self.units.len()
    }

    /// Whether the value has no code units.
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Index one code unit.
    pub fn code_unit_at(&self, offset: CodeUnitOffset) -> Option<u16> {
        self.units.get(offset.get()).copied()
    }

    /// Slice by a code-unit range, clamping both bounds.
    pub fn slice(&self, range: CodeUnitRange) -> Self {
        let start = range.start.get().min(self.len());
        let end = range.end.get().max(start).min(self.len());
        Self::from_code_units(self.units[Range { start, end }].to_vec())
    }

    /// Iterate exact code units (the indexing face).
    pub fn code_units(&self) -> impl ExactSizeIterator<Item = u16> + '_ {
        self.units.iter().copied()
    }

    /// Iterate chunks consisting of one surrogate pair or one unpaired unit.
    pub fn iter_code_points(&self) -> CodePointIter<'_> {
        CodePointIter {
            units: &self.units,
            at: 0,
        }
    }

    /// Convert well-formed UTF-16 to scalar Unicode text.
    pub fn to_scalar(&self) -> Result<String, CodeUnitStringError> {
        String::from_utf16(&self.units).map_err(|_| {
            let invalid = first_lone(&self.units).expect("invalid UTF-16 has a lone surrogate");
            CodeUnitStringError::LoneSurrogate(invalid)
        })
    }

    /// Convert a bounded scalar offset to its code-unit offset.
    pub fn code_unit_offset(
        &self,
        scalar: ScalarOffset,
    ) -> Result<CodeUnitOffset, OffsetConversionError> {
        let text = self
            .to_scalar()
            .map_err(OffsetConversionError::InvalidText)?;
        let mut scalar_at = 0;
        for (byte_at, _) in text.char_indices() {
            if scalar_at == scalar.get() {
                return Ok(CodeUnitOffset::new(text[..byte_at].encode_utf16().count()));
            }
            scalar_at += 1;
        }
        if scalar_at == scalar.get() {
            return Ok(CodeUnitOffset::new(self.len()));
        }
        Err(OffsetConversionError::ScalarOutOfBounds {
            offset: scalar,
            len: scalar_at,
        })
    }

    /// Convert a bounded code-unit offset at a scalar boundary.
    pub fn scalar_offset(
        &self,
        code_unit: CodeUnitOffset,
    ) -> Result<ScalarOffset, OffsetConversionError> {
        if code_unit.get() > self.len() {
            return Err(OffsetConversionError::CodeUnitOutOfBounds {
                offset: code_unit,
                len: self.len(),
            });
        }
        let text = self
            .to_scalar()
            .map_err(OffsetConversionError::InvalidText)?;
        let mut units = 0;
        let mut scalars = 0;
        for scalar in text.chars() {
            if units == code_unit.get() {
                return Ok(ScalarOffset::new(scalars));
            }
            units += scalar.len_utf16();
            scalars += 1;
            if units > code_unit.get() {
                return Err(OffsetConversionError::NotScalarBoundary(code_unit));
            }
        }
        Ok(ScalarOffset::new(scalars))
    }
}

/// Failure to convert between bounded offset domains.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OffsetConversionError {
    /// The exact unit sequence is not scalar Unicode text.
    InvalidText(CodeUnitStringError),
    /// The requested code-unit offset exceeds the sequence.
    CodeUnitOutOfBounds { offset: CodeUnitOffset, len: usize },
    /// The requested scalar offset exceeds the sequence.
    ScalarOutOfBounds { offset: ScalarOffset, len: usize },
    /// The code-unit offset splits a surrogate pair.
    NotScalarBoundary(CodeUnitOffset),
}

impl AsRef<[u16]> for CodeUnitString {
    fn as_ref(&self) -> &[u16] {
        self.as_code_units()
    }
}

/// Iterator over code-point chunks represented as exact code-unit strings.
pub struct CodePointIter<'a> {
    units: &'a [u16],
    at: usize,
}

impl Iterator for CodePointIter<'_> {
    type Item = CodeUnitString;

    fn next(&mut self) -> Option<Self::Item> {
        let first = *self.units.get(self.at)?;
        let width = if (0xd800..=0xdbff).contains(&first)
            && self
                .units
                .get(self.at + 1)
                .is_some_and(|unit| (0xdc00..=0xdfff).contains(unit))
        {
            2
        } else {
            1
        };
        let out = CodeUnitString::from_code_units(self.units[self.at..self.at + width].to_vec());
        self.at += width;
        Some(out)
    }
}

fn first_lone(units: &[u16]) -> Option<InvalidSurrogate> {
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if (0xd800..=0xdbff).contains(&unit) {
            if units
                .get(index + 1)
                .is_some_and(|next| (0xdc00..=0xdfff).contains(next))
            {
                index += 2;
                continue;
            }
            return Some(InvalidSurrogate {
                offset: CodeUnitOffset::new(index),
                unit,
            });
        }
        if (0xdc00..=0xdfff).contains(&unit) {
            return Some(InvalidSurrogate {
                offset: CodeUnitOffset::new(index),
                unit,
            });
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod law_fixtures {
    use super::*;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    #[test]
    fn length_index_slice_and_code_unit_iteration_are_utf16() {
        let string = CodeUnitString::from_scalar("A😀B");
        assert_eq!(string.len(), 4);
        assert_eq!(string.code_unit_at(CodeUnitOffset::new(1)), Some(0xd83d));
        assert_eq!(string.as_code_units(), [0x0041, 0xd83d, 0xde00, 0x0042]);
        assert_eq!(
            string
                .slice(CodeUnitRange::new(
                    CodeUnitOffset::new(1),
                    CodeUnitOffset::new(3)
                ))
                .as_code_units(),
            [0xd83d, 0xde00]
        );
        assert_eq!(string.code_units().count(), 4);
    }

    #[test]
    fn scalar_conversion_and_paired_iteration_are_exact() {
        let string = CodeUnitString::from_code_units(vec![0xd83d, 0xde00]);
        assert_eq!(string.to_scalar().unwrap(), "😀");
        assert_eq!(
            string.iter_code_points().next().unwrap().as_code_units(),
            [0xd83d, 0xde00]
        );
    }

    #[test]
    fn lone_high_surrogate_is_preserved_and_rejected_by_scalar_face() {
        let string = CodeUnitString::from_code_units(vec![0xd800]);
        assert_eq!(string.code_unit_at(CodeUnitOffset::new(0)), Some(0xd800));
        assert_eq!(
            string.iter_code_points().next().unwrap().as_code_units(),
            [0xd800]
        );
        assert_eq!(
            string.to_scalar(),
            Err(CodeUnitStringError::LoneSurrogate(InvalidSurrogate {
                offset: CodeUnitOffset::new(0),
                unit: 0xd800,
            }))
        );
    }

    #[test]
    fn lone_low_surrogate_is_preserved_and_rejected_by_scalar_face() {
        let string = CodeUnitString::from_code_units(vec![0xdc00]);
        assert_eq!(
            string.slice(CodeUnitRange::new(
                CodeUnitOffset::new(0),
                CodeUnitOffset::new(1)
            )),
            string
        );
        assert_eq!(
            string.to_scalar(),
            Err(CodeUnitStringError::LoneSurrogate(InvalidSurrogate {
                offset: CodeUnitOffset::new(0),
                unit: 0xdc00,
            }))
        );
    }

    #[test]
    fn pair_iteration_preserves_lone_units_and_nul() {
        let string =
            CodeUnitString::from_code_units(vec![0xd800, 0x0061, 0xd83d, 0xde00, 0xdc00, 0x0000]);
        let chunks: Vec<Vec<u16>> = string
            .iter_code_points()
            .map(|chunk| chunk.code_units().collect())
            .collect();
        assert_eq!(
            chunks,
            vec![
                vec![0xd800],
                vec![0x0061],
                vec![0xd83d, 0xde00],
                vec![0xdc00],
                vec![0x0000],
            ]
        );
    }

    #[test]
    fn offset_conversion_is_bounded_and_never_splits_a_pair() {
        let string = CodeUnitString::from_scalar("A😀B");
        assert_eq!(
            string.code_unit_offset(ScalarOffset::new(2)),
            Ok(CodeUnitOffset::new(3))
        );
        assert_eq!(
            string.scalar_offset(CodeUnitOffset::new(3)),
            Ok(ScalarOffset::new(2))
        );
        assert_eq!(
            string.scalar_offset(CodeUnitOffset::new(2)),
            Err(OffsetConversionError::NotScalarBoundary(
                CodeUnitOffset::new(2)
            ))
        );
        assert_eq!(
            string.code_unit_offset(ScalarOffset::new(4)),
            Err(OffsetConversionError::ScalarOutOfBounds {
                offset: ScalarOffset::new(4),
                len: 3,
            })
        );
    }

    #[test]
    fn scalar_text_round_trips_for_generated_unicode_sequences() {
        let alphabet = ['\0', 'a', 'ß', '中', '😀', '\u{10ffff}'];
        for seed in 0usize..512 {
            let text: String = (0..seed % 17)
                .map(|at| alphabet[(seed.wrapping_mul(17) ^ at.wrapping_mul(31)) % alphabet.len()])
                .collect();
            assert_eq!(
                CodeUnitString::from_scalar(&text).to_scalar().unwrap(),
                text
            );
        }
    }

    #[test]
    fn arbitrary_raw_units_are_preserved_by_value_equality_and_hash() {
        let mut state = 0x5eed_cafe_dead_beefu64;
        for len in 0..128 {
            let units: Vec<u16> = (0..len)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u16
                })
                .collect();
            let left = CodeUnitString::from_code_units(units.clone());
            let right = CodeUnitString::from_code_units(units.clone());
            assert_eq!(left.as_code_units(), units);
            assert_eq!(left, right);
            let mut left_hash = DefaultHasher::new();
            let mut right_hash = DefaultHasher::new();
            left.hash(&mut left_hash);
            right.hash(&mut right_hash);
            assert_eq!(left_hash.finish(), right_hash.finish());
        }
    }
}
