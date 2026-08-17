//! Exact UTF-16 code-unit text with explicit scalar projections.
//!
//! [`CodeUnitString`] preserves every `u16`, including lone surrogates and NUL.
//! It deliberately carries no JavaScript, JVM, codec, or interning policy.

mod exact;
mod projection;

pub use exact::{
    CodePointIter, CodeUnitOffset, CodeUnitRange, CodeUnitString, CodeUnitStringError,
    InvalidSurrogate, MAX_CODE_UNITS, OffsetConversionError, ScalarOffset, ScalarRange,
};
pub use projection::{
    CODE_UNIT_STRING_SYMBOL, CodeUnitStringReadConstructor, CodeUnitStringShape,
    code_unit_string_browse, code_unit_string_from_expr, code_unit_string_to_expr, scalar_text,
};

#[cfg(test)]
mod tests;
