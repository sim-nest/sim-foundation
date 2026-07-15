//! Canonical host-effect capability names shared by table-like backends.
//!
//! Filesystem, search, edit, process, and HTTP host effects use these names as
//! their canonical tokens. The alias helpers accept compatibility spellings for
//! host grant files.

use sim_kernel::{CapabilityName, Cx, Error, Result};

/// The capability gating read-only filesystem and directory access.
pub fn fs_read() -> CapabilityName {
    CapabilityName::new("fs/read")
}

/// The capability gating filesystem and directory mutation.
pub fn fs_write() -> CapabilityName {
    CapabilityName::new("fs/write")
}

/// The capability gating directory search operations.
pub fn find() -> CapabilityName {
    CapabilityName::new("find")
}

/// The capability gating in-place edit operations.
pub fn edit() -> CapabilityName {
    CapabilityName::new("edit")
}

/// The capability gating bounded host-process execution.
pub fn exec() -> CapabilityName {
    CapabilityName::new("exec")
}

/// The capability gating effectful HTTP access.
pub fn net_http() -> CapabilityName {
    CapabilityName::new("net/http")
}

/// Compatibility filesystem-read aliases accepted by call sites.
pub fn fs_read_aliases() -> &'static [&'static str] {
    &["table.fs.read", "stream.file.read", "file-read"]
}

/// Compatibility filesystem-write aliases accepted by call sites.
pub fn fs_write_aliases() -> &'static [&'static str] {
    &[
        "table.fs.write",
        "table.fs.mkdir",
        "table.fs.rmdir",
        "stream.file.write",
        "file-write",
    ]
}

/// Compatibility host-process aliases accepted by call sites.
pub fn exec_aliases() -> &'static [&'static str] {
    &["host.process"]
}

/// Compatibility HTTP/network aliases accepted by call sites.
pub fn net_http_aliases() -> &'static [&'static str] {
    &["net.http", "net-connect", "network"]
}

/// Return the granted canonical capability or one of its accepted aliases.
///
/// This is useful for effect records, which carry a flat required-capability
/// list and therefore cannot express "canonical or alias" directly.
pub fn granted_capability_or_alias(
    cx: &Cx,
    canonical: CapabilityName,
    aliases: &[&'static str],
) -> Result<CapabilityName> {
    if cx.capabilities().contains(&canonical) {
        return Ok(canonical);
    }
    for alias in aliases {
        let alias = CapabilityName::new(*alias);
        if cx.capabilities().contains(&alias) {
            return Ok(alias);
        }
    }
    Err(Error::CapabilityDenied {
        capability: canonical,
    })
}

/// Require a canonical capability, accepting compatibility aliases.
pub fn require_with_aliases(
    cx: &Cx,
    canonical: CapabilityName,
    aliases: &[&'static str],
) -> Result<()> {
    granted_capability_or_alias(cx, canonical, aliases).map(|_| ())
}
