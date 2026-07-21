//! Canonical feature keys.

use std::fmt;

use crate::SubjectId;

/// Canonical key for one feature, independent of projections and routes.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CanonicalFeatureKey(String);

impl CanonicalFeatureKey {
    /// Builds a key from already-normalized text.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the key text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns true when the key satisfies the canonical key grammar.
    pub fn is_valid(&self) -> bool {
        crate::shape::is_canonical_key(&self.0)
    }
}

impl fmt::Display for CanonicalFeatureKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Builds the canonical feature key for a subject and feature label.
pub fn canonical_feature_key(subject: &SubjectId, label: &str) -> CanonicalFeatureKey {
    CanonicalFeatureKey::new(format!("{}/{}", subject.as_str(), slug(label)))
}

fn slug(value: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for byte in value.bytes() {
        let ch = byte as char;
        let normalized = match ch {
            'A'..='Z' => ch.to_ascii_lowercase(),
            'a'..='z' | '0'..='9' | '_' | '.' => ch,
            '-' | '/' | ':' | ' ' => '-',
            _ => '-',
        };
        if normalized == '-' {
            if !last_dash && !out.is_empty() {
                out.push('-');
            }
            last_dash = true;
        } else {
            out.push(normalized);
            last_dash = false;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("feature");
    }
    out
}
