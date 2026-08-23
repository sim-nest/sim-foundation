//! One blocking HTTP policy boundary for SIM.
//!
//! Protocol parsing stays in `sim-lib-net-core`; sockets and DNS arrive through
//! the bound capsule's `sim-transport-ports` services. No ambient network,
//! proxy, cookie, credential, redirect, or logging behavior is hidden here.

mod client;
mod response;
mod transport;
mod types;

pub use client::Client;
pub use transport::{Connection, Connector, TcpConnector};
pub use types::{
    Cancellation, Error, Header, Method, Policy, ProxyPolicy, RedirectPolicy, Request, RequestBody,
    Response, Result, TlsRoots, Url,
};

pub(crate) use transport::connect_tls;
pub(crate) use types::{host_header, io_error};

/// Cookbook descriptors embedded for documentation and runtime discovery.
pub static RECIPES: sim_cookbook::EmbeddedDir =
    include!(concat!(env!("OUT_DIR"), "/cookbook_recipes.rs"));

#[cfg(test)]
mod tests;
