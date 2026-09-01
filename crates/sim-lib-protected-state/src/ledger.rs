use std::fmt;

use sim_kernel::{Cx, Symbol, Table, TableExpected, TableReplacement};

/// Optional atomic single-use claim service.
pub trait ConsumptionLedger {
    /// Claims a caller-derived opaque key; `true` is returned only to the first winner.
    fn claim(&self, cx: &mut Cx, key: Symbol) -> Result<bool, ConsumptionError>;
}

/// Adapter using the canonical kernel Table compare-exchange contract.
pub struct TableConsumptionLedger<'a> {
    table: &'a dyn Table,
}

impl<'a> TableConsumptionLedger<'a> {
    /// Wraps a canonical Table backend.
    #[must_use]
    pub const fn new(table: &'a dyn Table) -> Self {
        Self { table }
    }
}

impl ConsumptionLedger for TableConsumptionLedger<'_> {
    fn claim(&self, cx: &mut Cx, key: Symbol) -> Result<bool, ConsumptionError> {
        let marker = cx.factory().bool(true).map_err(|_| ConsumptionError)?;
        self.table
            .compare_exchange(
                cx,
                key,
                TableExpected::Absent,
                TableReplacement::Value(marker),
            )
            .map(|result| result.exchanged)
            .map_err(|_| ConsumptionError)
    }
}

/// Bounded non-secret failure from the injected canonical Table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumptionError;

impl fmt::Display for ConsumptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("consumption ledger unavailable")
    }
}

impl std::error::Error for ConsumptionError {}
