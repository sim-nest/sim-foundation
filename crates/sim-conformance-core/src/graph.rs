// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Acyclic support-graph validation.

use std::collections::{BTreeMap, BTreeSet};

use sim_kernel::{Datum, Symbol};

use crate::{EvidenceSetId, field, qualified, text};

/// Domain tag for one serialized evidence support graph.
pub const EVIDENCE_SUPPORT_GRAPH_DOMAIN: &str = "conformance/evidence-support-graph-v1";

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

    /// Encodes the declared nodes and edges. A cycle remains readable so admission can refuse it.
    pub fn to_datum(&self) -> Result<Datum, ConformanceError> {
        let nodes = Datum::Vector(self.edges.keys().map(|node| text(node.0.clone())).collect());
        let mut edges = Vec::new();
        for (subject, supports) in &self.edges {
            for support in supports {
                edges.push(edge_datum(subject, support)?);
            }
        }
        Ok(Datum::Node {
            tag: qualified(EVIDENCE_SUPPORT_GRAPH_DOMAIN)?,
            fields: vec![
                field("edges", Datum::Vector(edges))?,
                field("nodes", nodes)?,
            ],
        })
    }

    /// Decodes one evidence support graph and refuses a cycle before the caller can hash it.
    pub fn from_datum(datum: &Datum) -> Result<Self, ConformanceError> {
        let Datum::Node { tag, fields } = datum else {
            return Err(ConformanceError::WrongIdentityDomain);
        };
        if tag != &qualified(EVIDENCE_SUPPORT_GRAPH_DOMAIN)? {
            return Err(ConformanceError::WrongIdentityDomain);
        }
        let [edges, nodes] = exact_fields(fields, &["edges", "nodes"])?;
        let mut graph = Self::default();
        for node in vector_items(nodes)? {
            let name = datum_string(node)?;
            let node = SupportNode(name);
            if graph.edges.contains_key(&node) {
                return Err(ConformanceError::DuplicateField(node.0));
            }
            graph.add_node(node);
        }
        for edge in vector_items(edges)? {
            let (subject, support) = edge_endpoints(edge)?;
            let subject = SupportNode(subject);
            let support = SupportNode(support);
            if !graph.edges.contains_key(&subject) || !graph.edges.contains_key(&support) {
                return Err(ConformanceError::UnresolvedBinding);
            }
            if graph
                .edges
                .get(&subject)
                .is_some_and(|targets| targets.contains(&support))
            {
                return Err(ConformanceError::DuplicateField(subject.0));
            }
            graph.add_support(subject, support)?;
        }
        graph.validate()?;
        Ok(graph)
    }

    /// Consumes canonical support bytes. The decoded graph keeps its domain and its refusals.
    pub fn consume(stored: &[u8], datum: &Datum) -> Result<Self, ConformanceError> {
        if stored.is_empty() {
            return Err(ConformanceError::UnresolvedBinding);
        }
        let canonical = datum
            .canonical_bytes()
            .map_err(|_| ConformanceError::NoncanonicalDatum)?;
        if canonical.as_slice() != stored {
            return Err(ConformanceError::StorageDigestMismatch);
        }
        Self::from_datum(datum)
    }

    /// Identifies consumed support only after the graph has been accepted.
    pub fn evidence_id(stored: &[u8], datum: &Datum) -> Result<EvidenceSetId, ConformanceError> {
        let _accepted = Self::consume(stored, datum)?;
        EvidenceSetId::from_fields(vec![field("graph", datum.clone())?])
    }
}

fn edge_datum(subject: &SupportNode, support: &SupportNode) -> Result<Datum, ConformanceError> {
    Ok(Datum::Node {
        tag: qualified("conformance/evidence-support-edge-v1")?,
        fields: vec![
            field("subject", text(subject.0.clone()))?,
            field("support", text(support.0.clone()))?,
        ],
    })
}

fn exact_fields<'a>(
    fields: &'a [(Symbol, Datum)],
    names: &[&str],
) -> Result<[&'a Datum; 2], ConformanceError> {
    if fields.len() != names.len() {
        return Err(ConformanceError::UnresolvedBinding);
    }
    Ok([
        require_field(fields, names[0])?,
        require_field(fields, names[1])?,
    ])
}

fn require_field<'a>(
    fields: &'a [(Symbol, Datum)],
    name: &str,
) -> Result<&'a Datum, ConformanceError> {
    let key = qualified(&format!("conformance/{name}"))?;
    let mut found = None;
    for (field_name, value) in fields {
        if field_name == &key {
            if found.is_some() {
                return Err(ConformanceError::DuplicateField(name.to_owned()));
            }
            found = Some(value);
        }
    }
    found.ok_or(ConformanceError::UnresolvedBinding)
}

fn vector_items(value: &Datum) -> Result<&[Datum], ConformanceError> {
    match value {
        Datum::Vector(items) => Ok(items),
        _ => Err(ConformanceError::InvalidTemplate("support node")),
    }
}

fn datum_string(value: &Datum) -> Result<String, ConformanceError> {
    match value {
        Datum::String(name) if !name.is_empty() => Ok(name.clone()),
        Datum::String(_) => Err(ConformanceError::UnresolvedBinding),
        _ => Err(ConformanceError::InvalidTemplate("support node")),
    }
}

fn edge_endpoints(value: &Datum) -> Result<(String, String), ConformanceError> {
    let Datum::Node { tag, fields } = value else {
        return Err(ConformanceError::WrongIdentityDomain);
    };
    if tag != &qualified("conformance/evidence-support-edge-v1")? {
        return Err(ConformanceError::WrongIdentityDomain);
    }
    let [subject, support] = exact_fields(fields, &["subject", "support"])?;
    Ok((datum_string(subject)?, datum_string(support)?))
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
    /// An owner snapshot tried to record `Unknown` as an affirmative decision.
    InvalidRevocationDecision,
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
