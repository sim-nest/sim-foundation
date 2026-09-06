//! Hostile-return decoding into immutable checked proposals.

use sim_conformance_core::StorageId;
use sim_kernel::Datum;

use crate::{FacetPlanId, ImplementationPacket, OutputContractId, PacketId, ReceiptId, WorkError};

/// Immutable result of hostile-return decoding and pure facet checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedProposal {
    packet: PacketId,
    raw: StorageId,
    decoded: Datum,
    facet_plan: FacetPlanId,
    receipts: Vec<ReceiptId>,
}

impl CheckedProposal {
    /// Returns the packet that bounded this proposal.
    pub const fn packet(&self) -> &PacketId {
        &self.packet
    }

    /// Returns the address of the exact hostile return bytes.
    pub const fn raw(&self) -> &StorageId {
        &self.raw
    }

    /// Returns the decoded value accepted by the packet's output Shape.
    pub const fn decoded(&self) -> &Datum {
        &self.decoded
    }

    /// Returns the pure facet plan identity.
    pub const fn facet_plan(&self) -> &FacetPlanId {
        &self.facet_plan
    }

    /// Returns every supporting pure receipt identity.
    pub fn receipts(&self) -> &[ReceiptId] {
        &self.receipts
    }
}

/// Decodes hostile bytes under a caller-supplied parser and exact Shape predicate.
pub fn decode_proposal(
    packet: &ImplementationPacket,
    bytes: &[u8],
    decode: impl FnOnce(&[u8]) -> Result<Datum, String>,
    shape_matches: impl FnOnce(&Datum, &OutputContractId) -> Result<(), String>,
    facet_plan: FacetPlanId,
    receipts: Vec<ReceiptId>,
) -> Result<CheckedProposal, WorkError> {
    if bytes.len() as u64 > packet.draft().input_budget.output_bytes {
        return Err(WorkError::OutputBudget);
    }
    let raw = StorageId::for_bytes(bytes);
    let decoded = decode(bytes).map_err(|error| WorkError::MalformedReturn(vec![error]))?;
    shape_matches(&decoded, &packet.draft().output_contract)
        .map_err(|error| WorkError::MalformedReturn(vec![error]))?;
    Ok(CheckedProposal {
        packet: packet.id().clone(),
        raw,
        decoded,
        facet_plan,
        receipts,
    })
}
