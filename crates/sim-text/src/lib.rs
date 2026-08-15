//! Exact UTF-16 code-unit text with explicit scalar projections.
//!
//! [`CodeUnitString`] preserves every `u16`, including lone surrogates and NUL.
//! It deliberately carries no JavaScript, JVM, codec, or interning policy.

use core::fmt;

/// The largest code-unit allocation representable by Rust's collection APIs.
///
/// This bound also makes multiplication by the size of a code unit safe.
pub const MAX_CODE_UNITS: usize = isize::MAX as usize / size_of::<u16>();

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
    LoneSurrogate {
        /// Code-unit index of the first invalid surrogate.
        index: usize,
        /// Invalid code unit.
        unit: u16,
    },
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
            Self::LoneSurrogate { index, unit } => {
                write!(f, "lone surrogate {unit:#06x} at code-unit index {index}")
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
    pub fn code_unit_at(&self, index: usize) -> Option<u16> {
        self.units.get(index).copied()
    }

    /// Slice by nonnegative code-unit indices, clamping both bounds.
    pub fn slice(&self, start: usize, end: usize) -> Self {
        let start = start.min(self.len());
        let end = end.max(start).min(self.len());
        Self::from_code_units(self.units[start..end].to_vec())
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
            let (index, unit) =
                first_lone(&self.units).expect("invalid UTF-16 has a lone surrogate");
            CodeUnitStringError::LoneSurrogate { index, unit }
        })
    }
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

fn first_lone(units: &[u16]) -> Option<(usize, u16)> {
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
            return Some((index, unit));
        }
        if (0xdc00..=0xdfff).contains(&unit) {
            return Some((index, unit));
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod law_fixtures {
    use super::*;

    #[test]
    fn length_index_slice_and_code_unit_iteration_are_utf16() {
        let string = CodeUnitString::from_scalar("A😀B");
        assert_eq!(string.len(), 4);
        assert_eq!(string.code_unit_at(1), Some(0xd83d));
        assert_eq!(string.as_code_units(), [0x0041, 0xd83d, 0xde00, 0x0042]);
        assert_eq!(string.slice(1, 3).as_code_units(), [0xd83d, 0xde00]);
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
        assert_eq!(string.code_unit_at(0), Some(0xd800));
        assert_eq!(
            string.iter_code_points().next().unwrap().as_code_units(),
            [0xd800]
        );
        assert_eq!(
            string.to_scalar(),
            Err(CodeUnitStringError::LoneSurrogate {
                index: 0,
                unit: 0xd800,
            })
        );
    }

    #[test]
    fn lone_low_surrogate_is_preserved_and_rejected_by_scalar_face() {
        let string = CodeUnitString::from_code_units(vec![0xdc00]);
        assert_eq!(string.slice(0, 1), string);
        assert_eq!(
            string.to_scalar(),
            Err(CodeUnitStringError::LoneSurrogate {
                index: 0,
                unit: 0xdc00,
            })
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
}
