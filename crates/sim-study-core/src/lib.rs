//! Pure, domain-neutral records for repeatable studies.
//!
//! All identity is a canonical kernel [`sim_kernel::Datum`]. Operational
//! context is separate and identity-neutral. Callers install whichever
//! general-purpose SIM codec they need for `Datum`; this crate creates no
//! competing syntax or JSON authority.

#![forbid(unsafe_code)]

mod encoding;
mod records;

pub use records::*;
