//! Acyclic support-graph validation.

use std::collections::{BTreeMap, BTreeSet};

/// Stable graph node name used before dispatch or receipt admission.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SupportNode(pub String);

/// Directed support relationships among declarations, subjects, calls, and receipts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SupportGraph {
    edges: BTreeMap<SupportNode, BTreeSet<SupportNode>>,
}

impl SupportGraph {
    /// Adds an identity even when it has no dependencies.
    pub fn add_node(&mut self, node: SupportNode) {
        self.edges.entry(node).or_default();
    }

    /// Adds `subject -> support`; direct self-support is rejected immediately.
    pub fn add_support(
        &mut self,
        subject: SupportNode,
        support: SupportNode,
    ) -> Result<(), ConformanceError> {
        if subject == support {
            return Err(ConformanceError::SupportCycle(vec![subject.0]));
        }
        self.edges.entry(support.clone()).or_default();
        self.edges.entry(subject).or_default().insert(support);
        Ok(())
    }

    /// Verifies every reachable support edge is acyclic.
    pub fn validate(&self) -> Result<(), ConformanceError> {
        let mut permanent = BTreeSet::new();
        let mut active = BTreeSet::new();
        let mut path = Vec::new();
        for node in self.edges.keys() {
            visit(self, node, &mut permanent, &mut active, &mut path)?;
        }
        Ok(())
    }
}

fn visit(
    graph: &SupportGraph,
    node: &SupportNode,
    permanent: &mut BTreeSet<SupportNode>,
    active: &mut BTreeSet<SupportNode>,
    path: &mut Vec<SupportNode>,
) -> Result<(), ConformanceError> {
    if permanent.contains(node) {
        return Ok(());
    }
    if !active.insert(node.clone()) {
        let start = path.iter().position(|item| item == node).unwrap_or(0);
        let mut cycle = path[start..]
            .iter()
            .map(|item| item.0.clone())
            .collect::<Vec<_>>();
        cycle.push(node.0.clone());
        return Err(ConformanceError::SupportCycle(cycle));
    }
    path.push(node.clone());
    for support in graph.edges.get(node).into_iter().flatten() {
        visit(graph, support, permanent, active, path)?;
    }
    path.pop();
    active.remove(node);
    permanent.insert(node.clone());
    Ok(())
}

/// A typed refusal from the neutral conformance layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConformanceError {
    /// A semantic record used the wrong canonical tag.
    WrongIdentityDomain,
    /// A name was not a valid plain or one-level qualified symbol.
    InvalidSymbol,
    /// Two fields had the same key.
    DuplicateField(String),
    /// Kernel canonicalization refused the value.
    NoncanonicalDatum,
    /// Retrieved bytes did not match their byte address.
    StorageDigestMismatch,
    /// A support path contains a direct or indirect cycle.
    SupportCycle(Vec<String>),
    /// A binding or packet contains an unresolved design decision.
    UnresolvedBinding,
    /// A declared surface or dependency is absent.
    MissingSurface(String),
    /// A target is owned by another binding.
    WrongOwner,
    /// A target is scheduled for another producing phase.
    WrongProducingPhase,
    /// A dependency has no current qualification at the required scope.
    UnqualifiedDependency(String),
    /// An activation-only receipt was offered as produced evidence.
    ActivationIsNotProductionEvidence,
    /// A check scope was not authorized by its static binding.
    UnauthorizedScope,
    /// A template slot was missing or duplicated.
    InvalidTemplate(&'static str),
    /// Invocation and receipt identities or fields disagree.
    InvocationMismatch(&'static str),
    /// The result does not represent a passing check.
    CheckFailed,
    /// Revocation authority is absent or explicitly revokes this result.
    RevocationUnknownOrActive,
    /// A bounded collection or text value exceeded its ceiling.
    BoundExceeded(&'static str),
    /// A digest-construction key or source anchor was duplicated.
    DuplicateConstruction(String),
    /// A construction has no funded disposition.
    UnfundedConstruction(String),
}

impl std::fmt::Display for ConformanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ConformanceError {}
