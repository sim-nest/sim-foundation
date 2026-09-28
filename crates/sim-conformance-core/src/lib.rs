// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Neutral, effect-free conformance records for SIM.
//!
//! The crate separates immutable declarations from checked invocations and
//! receipts. Every record uses canonical kernel [`sim_kernel::Datum`] identity,
//! every scope is explicit, and support graphs must be acyclic before a result
//! can be admitted.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod binding;
mod checker;
mod digest;
mod fake;
mod graph;
mod identity;
mod live_authority;
mod qualification;
mod receipt;
mod receipt_claim;
mod revocation;

pub use binding::*;
pub use checker::*;
pub use digest::*;
pub use fake::*;
pub use graph::*;
pub use identity::*;
pub use live_authority::*;
pub use qualification::*;
pub use receipt::*;
pub use receipt_claim::*;
pub use revocation::*;
