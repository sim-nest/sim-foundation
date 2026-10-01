// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use sim_kernel::{ContentId, Symbol};

use super::NS;

/// Named reason a neutral input is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeutralInputGap {
    /// A value of the wrong kind or an unreadable structure.
    Malformed,
    /// A required field is absent.
    MissingField,
    /// A field outside the Shape is present, or a field is repeated.
    ExtraField,
    /// A node tag or symbol is not one this Shape registers.
    UnrecognizedTag,
    /// A string value is present; the input carries identities and numbers only.
    ForbiddenText,
    /// The version is not exactly 1.
    UnsupportedVersion,
    /// The encoded form exceeds [`MAX_NEUTRAL_INPUT_BYTES`].
    Oversize,
    /// A stated bound or arithmetic identity fails.
    Arithmetic,
    /// A recomputed identity differs from the stated one.
    Identity,
    /// The input does not match the selected source bytes it names.
    SourceMismatch,
    /// A fact required by the retained table is absent or differs.
    FactMismatch,
}

/// A refused neutral input: the gap and a short diagnostic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralInputError {
    /// The named gap.
    pub gap: NeutralInputGap,
    /// Human diagnostic; never part of the gate.
    pub detail: String,
}

impl NeutralInputError {
    pub(super) fn new(gap: NeutralInputGap, detail: impl Into<String>) -> Self {
        Self {
            gap,
            detail: detail.into(),
        }
    }
}

impl core::fmt::Display for NeutralInputError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "neutral input gap {:?}: {}", self.gap, self.detail)
    }
}

impl std::error::Error for NeutralInputError {}

pub(super) type Result<T> = core::result::Result<T, NeutralInputError>;

/// Closeout epoch counts. `failed + invalidated + interrupted_running == total`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpochCounts {
    /// Failed epochs.
    pub failed: u64,
    /// Invalidated epochs.
    pub invalidated: u64,
    /// Interrupted or still-running raw epochs.
    pub interrupted_running: u64,
    /// All epochs.
    pub total: u64,
}

/// Proof-leaf class counts. The four classes sum to `total`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofClassCounts {
    /// Generate leaves.
    pub generate: u64,
    /// Standalone leaves.
    pub standalone: u64,
    /// Workspace leaves.
    pub workspace: u64,
    /// Package leaves.
    pub package: u64,
    /// All proof leaves.
    pub total: u64,
}

/// Telemetry span counts. `finished <= total`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpanCounts {
    /// Finished spans.
    pub finished: u64,
    /// All spans.
    pub total: u64,
}

/// Frontier counts. `green + interrupted + pending == total`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrontierCounts {
    /// Green leaves.
    pub green: u64,
    /// Interrupted leaves.
    pub interrupted: u64,
    /// Pending leaves.
    pub pending: u64,
    /// All leaves.
    pub total: u64,
}

/// Registered contrasts a [`ReuseCount`] may name (see the module table).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReuseContrast {
    /// Retention after a control-only repair.
    ControlOnlyRepair,
    /// Retention after a public-head and pin change.
    PublicHeadAndPinChange,
    /// Receipts reused in the last raw epoch.
    LastRawEpoch,
    /// Green leaves at the previous fixed point.
    PreviousFixedPoint,
}

impl ReuseContrast {
    pub(super) const ALL: [Self; 4] = [
        Self::ControlOnlyRepair,
        Self::PublicHeadAndPinChange,
        Self::LastRawEpoch,
        Self::PreviousFixedPoint,
    ];

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::ControlOnlyRepair => "control-only-repair",
            Self::PublicHeadAndPinChange => "public-head-and-pin-change",
            Self::LastRawEpoch => "last-raw-epoch",
            Self::PreviousFixedPoint => "previous-fixed-point",
        }
    }

    pub(super) fn from_symbol(symbol: &Symbol) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|contrast| *symbol == Symbol::qualified(NS, contrast.name()))
            .ok_or_else(|| {
                NeutralInputError::new(NeutralInputGap::UnrecognizedTag, "unregistered contrast")
            })
    }
}

/// One count of retained items out of a denominator for a registered contrast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReuseCount {
    /// The registered contrast.
    pub contrast: ReuseContrast,
    /// Items retained. `retained <= denominator`.
    pub retained: u64,
    /// The denominator.
    pub denominator: u64,
}

/// Neutral numeric facts. The id hashes these fields only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralFacts {
    /// Scheduled items.
    pub scheduled: u64,
    /// Materialized items. `materialized <= scheduled`.
    pub materialized: u64,
    /// Plan revisions.
    pub revisions: u64,
    /// Closeout epochs.
    pub epochs: EpochCounts,
    /// Proof classes.
    pub proof_classes: ProofClassCounts,
    /// Lifecycle event rows.
    pub lifecycle_rows: u64,
    /// Telemetry spans.
    pub spans: SpanCounts,
    /// Sum of overlapping span durations in milliseconds.
    pub overlapping_span_milliseconds: u64,
    /// Repeated `unit.ready.carried` rows.
    pub repeated_ready_rows: u64,
    /// Repeated `focused.check.receipt.carried` rows.
    pub repeated_receipt_rows: u64,
    /// The last raw epoch's frontier.
    pub last_frontier: FrontierCounts,
    /// Reuse counts, each registered contrast at most once, ordered by contrast.
    pub contrast_reuse: Vec<ReuseCount>,
}

/// How the facts were obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivationBasis {
    /// Restated from an aggregate table in the selected source.
    RestatedMeasurements,
    /// Read-only aggregate queries.
    ReadOnlyAggregates,
}

/// Declared precision of the duration conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeasurementPrecision {
    /// Number of decimal places the source hours carry.
    pub source_decimal_places: u64,
}

/// Role of a digest named by the derivation receipt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivationSourceRole {
    /// The exact selected round source bytes.
    SelectedRoundSource,
    /// The exact measurements-table region.
    MeasurementsTableRegion,
}

/// One digest the derivation names (SHA-256 over exact bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DerivationSource {
    /// The role of the bytes.
    pub role: DerivationSourceRole,
    /// SHA-256 digest.
    pub digest: [u8; 32],
}

/// Derivation receipt; its id hashes its own fields with the id excluded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralFactDerivationReceipt {
    /// Basis of the derivation.
    pub basis: DerivationBasis,
    /// Exact identity of the derivation procedure record.
    pub procedure: ContentId,
    /// Producer of the facts.
    pub producer: Symbol,
    /// Commission that authorized the production.
    pub authority: Symbol,
    /// Review that verifies the production; naming it asserts no outcome.
    pub review: Symbol,
    /// Digests of the sources the facts were derived from.
    pub source_digests: Vec<DerivationSource>,
    /// Precision of the duration conversion.
    pub precision: MeasurementPrecision,
    /// Must be true: no runtime consumption of this slot.
    pub no_runtime_consumption: bool,
}

/// The complete private neutral qualification input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NeutralQualificationInput {
    /// Exact-byte identity of the selected round source.
    pub selected_source: ContentId,
    /// The facts.
    pub facts: NeutralFacts,
    /// The derivation receipt.
    pub derivation: NeutralFactDerivationReceipt,
}
