// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Owner Shapes and verifier for the selection-time neutral qualification input.
//!
//! The input is one bounded canonical [`Datum`] value of Shape
//! `qualification/neutral-input-v1`. It carries aggregate measures of retained
//! authoring evidence (`NeutralFacts`), a derivation receipt that names the
//! selected roadmap source, and nothing else: numbers, typed categories and
//! identities only. Decoding is strict: unknown tags, missing or extra fields,
//! wrong value kinds and any string value are refused with a named
//! [`NeutralInputGap`].
//!
//! Every fact in the roadmap's retained-measurement table has exactly one
//! registered representation here. Facts that are counts out of a denominator
//! use [`ReuseCount`] with a registered [`ReuseContrast`]; the registered
//! contrasts and their meaning are fixed by this module, not chosen by an
//! authoring agent:
//!
//! | contrast | `retained` of `denominator` |
//! | --- | --- |
//! | `control-only-repair` | proof receipts retained after a control-only repair, of all proof leaves |
//! | `public-head-and-pin-change` | proof receipts retained after a change to public heads and pins, of all proof leaves |
//! | `last-raw-epoch` | receipts reused in the last raw epoch, of that epoch's frontier leaves |
//! | `previous-fixed-point` | green leaves at the previous fixed point, of that fixed point's own all-green leaf population (`retained == denominator`) |

mod decode;
mod encode;
mod markdown;
mod table;
mod types;

#[cfg(test)]
mod tests;

pub use table::{RestatedMeasurements, SlotCodec, source_byte_identity};
pub use types::*;

/// Exact version carried by every input of this Shape.
pub const NEUTRAL_INPUT_VERSION: u64 = 1;
/// Maximum canonical encoded size of one input, in bytes.
pub const MAX_NEUTRAL_INPUT_BYTES: usize = 64 * 1024;

pub(super) const NS: &str = "qualification";
pub(super) const MS_PER_HOUR: u64 = 3_600_000;
