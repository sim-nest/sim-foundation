use crate::WebRecordError;

/// Every independent policy question; a complete decision set contains all ten.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PolicyKind {
    EgressZone,
    Robots,
    Method,
    Domain,
    Media,
    Bytes,
    Redirects,
    Rate,
    CacheMode,
    ResearchBudget,
}
impl PolicyKind {
    pub const ALL: [Self; 10] = [
        Self::EgressZone,
        Self::Robots,
        Self::Method,
        Self::Domain,
        Self::Media,
        Self::Bytes,
        Self::Redirects,
        Self::Rate,
        Self::CacheMode,
        Self::ResearchBudget,
    ];
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PolicyVerdict {
    Allow,
    Deny,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyDecision {
    pub kind: PolicyKind,
    pub verdict: PolicyVerdict,
    pub rule: String,
    pub limit: Option<u64>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyReceipt {
    pub decisions: Vec<PolicyDecision>,
}
impl PolicyReceipt {
    pub fn checked(decisions: Vec<PolicyDecision>) -> Result<Self, WebRecordError> {
        for kind in PolicyKind::ALL {
            if decisions.iter().filter(|d| d.kind == kind).count() != 1 {
                return Err(WebRecordError::MissingDecision(kind));
            }
        }
        Ok(Self { decisions })
    }
    pub fn permits(&self) -> bool {
        PolicyKind::ALL.into_iter().all(|kind| {
            self.decisions
                .iter()
                .any(|d| d.kind == kind && d.verdict == PolicyVerdict::Allow)
        })
    }
}
