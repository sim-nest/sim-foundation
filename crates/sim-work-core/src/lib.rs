//! Pure bounded-work and implementation-packet contracts.
//!
//! Packet construction reads exact declared inputs through an injected port,
//! checks released dependencies independently from funded targets, and grants
//! no filesystem, process, Git, network, delivery, or approval authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod packet;
mod work;

pub use packet::*;
pub use work::*;
