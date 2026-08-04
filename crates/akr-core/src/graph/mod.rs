//! Graph algorithms over the ledger: cycles, reachability, and staleness propagation.
//!
//! Every function here is **deterministic in the strong sense**: its result depends only
//! on the set of nodes and edges, never on the order they were supplied in. That is not a
//! nicety. A cycle report that changes between runs is a diagnostic nobody can put in a
//! test, and a propagation path that changes between runs is a review-queue entry nobody
//! can act on.
//!
//! Determinism is achieved the same way everywhere: sort the node set, sort each
//! adjacency list, and never iterate a hash container. `tests/graph_determinism.rs`
//! asserts it by shuffling inputs with a seeded generator and comparing results.

use crate::model::{Ledger, Record, Relation, RevisionId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The relations along which staleness propagates to dependents (D-024).
///
/// Three, and no others. Propagating along `part_of` or `after` would flag half the
/// project every time a file changed, and a warning that always fires is not a warning.
pub const PROPAGATING_RELATIONS: &[Relation] = &[
    Relation::SupportedBy,
    Relation::DependsOn,
    Relation::DerivedFrom,
];

/// A directed graph over sorted, cloneable nodes.
///
/// Built once and queried many times, which is what the resolver wants: the same
/// adjacency is walked by cycle detection, reachability, and topological ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiGraph<N: Ord + Clone> {
    nodes: BTreeSet<N>,
    edges: BTreeMap<N, BTreeSet<N>>,
}

impl<N: Ord + Clone> Default for DiGraph<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N: Ord + Clone> DiGraph<N> {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: BTreeSet::new(),
            edges: BTreeMap::new(),
        }
    }

    /// Adds a node with no edges. Idempotent.
    pub fn add_node(&mut self, node: N) {
        self.nodes.insert(node);
    }

    /// Adds an edge, adding both endpoints as nodes. Idempotent.
    pub fn add_edge(&mut self, from: N, to: N) {
        self.nodes.insert(from.clone());
        self.nodes.insert(to.clone());
        self.edges.entry(from).or_default().insert(to);
    }

    /// Every node, in sorted order.
    pub fn nodes(&self) -> impl Iterator<Item = &N> {
        self.nodes.iter()
    }

    /// How many nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// How many distinct edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(BTreeSet::len).sum()
    }

    /// The successors of a node, in sorted order.
    pub fn successors(&self, node: &N) -> impl Iterator<Item = &N> {
        self.edges.get(node).into_iter().flatten()
    }

    /// The graph with every edge reversed.
    ///
    /// This is what staleness propagation walks: an edge `a -> b` means "a rests on b",
    /// so doubt travels from `b` back to `a`.
    #[must_use]
    pub fn reversed(&self) -> Self {
        let mut out = Self::new();
        for node in &self.nodes {
            out.add_node(node.clone());
        }
        for (from, tos) in &self.edges {
            for to in tos {
                out.add_edge(to.clone(), from.clone());
            }
        }
        out
    }

    /// Finds one cycle, deterministically, or `None` if the graph is acyclic.
    ///
    /// The returned path starts and ends at the same node, so `[a, b, a]` reads as
    /// `a -> b -> a`. A graph with several cycles always reports the same one: nodes are
    /// visited in sorted order, successors in sorted order, and the first back-edge found
    /// wins.
    #[must_use]
    pub fn find_cycle(&self) -> Option<Vec<N>> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Mark {
            Open,
            Done,
        }

        let mut marks: BTreeMap<&N, Mark> = BTreeMap::new();

        for start in &self.nodes {
            if marks.contains_key(start) {
                continue;
            }
            // An explicit stack of (node, remaining successors) rather than recursion, so
            // that a deep chain cannot blow the stack on a large ledger.
            let mut path: Vec<&N> = vec![start];
            let mut pending: Vec<Vec<&N>> = vec![self.successors(start).collect()];
            marks.insert(start, Mark::Open);

            while let Some(top) = pending.last_mut() {
                if let Some(next) = top.first().copied() {
                    top.remove(0);
                    match marks.get(next) {
                        Some(Mark::Open) => {
                            let at = path.iter().position(|n| *n == next).unwrap_or(0);
                            let mut cycle: Vec<N> =
                                path[at..].iter().map(|n| (*n).clone()).collect();
                            cycle.push(next.clone());
                            return Some(cycle);
                        }
                        Some(Mark::Done) => {}
                        None => {
                            marks.insert(next, Mark::Open);
                            path.push(next);
                            pending.push(self.successors(next).collect());
                        }
                    }
                } else {
                    pending.pop();
                    if let Some(node) = path.pop() {
                        marks.insert(node, Mark::Done);
                    }
                }
            }
        }
        None
    }

    /// Whether the graph is acyclic.
    #[must_use]
    pub fn is_acyclic(&self) -> bool {
        self.find_cycle().is_none()
    }

    /// Every node reachable from `start` by following at least one edge, in sorted order.
    ///
    /// `start` itself appears only when it is genuinely reachable — that is, when it sits
    /// on a cycle. Excluding it unconditionally would make `reaches(a, a)` false for a
    /// self-loop, which is the one case where the answer obviously matters.
    #[must_use]
    pub fn reachable_from(&self, start: &N) -> BTreeSet<N> {
        let mut seen = BTreeSet::new();
        let mut queue: VecDeque<&N> = VecDeque::new();
        queue.push_back(start);
        while let Some(node) = queue.pop_front() {
            for next in self.successors(node) {
                if seen.insert(next.clone()) {
                    queue.push_back(next);
                }
            }
        }
        seen
    }

    /// Whether `to` is reachable from `from`.
    #[must_use]
    pub fn reaches(&self, from: &N, to: &N) -> bool {
        self.reachable_from(from).contains(to)
    }

    /// A topological order, or `None` if the graph has a cycle.
    ///
    /// Kahn's algorithm over a sorted ready set, so a graph with several valid orders
    /// always yields the same one (`docs/06-compiler-pipeline.md` §11).
    #[must_use]
    pub fn topological_order(&self) -> Option<Vec<N>> {
        let mut indegree: BTreeMap<&N, usize> = self.nodes.iter().map(|n| (n, 0usize)).collect();
        for tos in self.edges.values() {
            for to in tos {
                if let Some(slot) = indegree.get_mut(to) {
                    *slot += 1;
                }
            }
        }
        let mut ready: BTreeSet<&N> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(n, _)| *n)
            .collect();
        let mut out: Vec<N> = Vec::with_capacity(self.nodes.len());

        while let Some(node) = ready.iter().next().copied() {
            ready.remove(node);
            out.push(node.clone());
            for next in self.successors(node) {
                if let Some(slot) = indegree.get_mut(next) {
                    *slot -= 1;
                    if *slot == 0 {
                        ready.insert(next);
                    }
                }
            }
        }
        (out.len() == self.nodes.len()).then_some(out)
    }
}

// -------------------------------------------------------------------------------------
// Propagation
// -------------------------------------------------------------------------------------

/// One record flagged `at_risk`, with the evidence for why.
///
/// Carrying the path is not decoration: `REVIEW-REQUIRED.md` and `akr context` both print
/// it, and "at risk, but I cannot tell you why" is a flag nobody acts on
/// (`docs/10-freshness-and-git.md` §4).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AtRisk {
    /// The flagged revision.
    pub id: RevisionId,
    /// Propagation distance from the nearest stale record. Always at least 1.
    pub depth: usize,
    /// The relation the doubt arrived along, on the last hop.
    pub via: Relation,
    /// The path from the flagged record to the stale record it rests on, exclusive of the
    /// flagged record itself and inclusive of the stale one.
    pub path: Vec<RevisionId>,
}

/// Propagates staleness from `stale` to its dependents along `supported_by`,
/// `depends_on` and `derived_from` (D-024).
///
/// Breadth-first over the reversed dependency graph, so every dependent is reported at
/// its **shortest** distance from a stale record. Transitive, unbounded in depth, and
/// cycle-safe — a cyclic dependency graph is V-015's problem, and this must not hang
/// while that diagnostic is being produced.
///
/// A record that is itself stale is never additionally marked at risk: it is already
/// the thing to fix.
///
/// # Only live records are flagged, and doubt does not travel through a terminal one
///
/// `docs/02-data-model.md` §6 defines `at_risk` over **live** records. A superseded,
/// withdrawn or disproven record rests on whatever it rested on when it was settled;
/// flagging it asks somebody to review a decision the project has already moved past,
/// and a warning nobody can act on is a warning that trains people to ignore the rest.
///
/// The same reasoning stops the walk at a terminal record rather than passing through it.
/// The one relation that can point from a live record at a terminal one is `derived_from`
/// (V-019 forbids the others), and `derived_from` is provenance: a record derived from a
/// retired finding was derived from what that finding said at the time, and a change
/// beneath the retired finding does not reach back through it.
///
/// # Determinism
///
/// The frontier is a sorted queue and adjacency is sorted, so two runs over the same
/// ledger produce identical depths and identical paths even when a record is reachable
/// from two stale sources at the same distance.
#[must_use]
pub fn propagate_staleness(ledger: &Ledger, stale: &BTreeSet<RevisionId>) -> Vec<AtRisk> {
    let forward = dependency_graph(ledger);
    let reverse = forward.reversed();

    // (depth, via, path) for each flagged revision, keyed so the first (shortest, then
    // lexicographically smallest) assignment wins.
    let mut flagged: BTreeMap<RevisionId, AtRisk> = BTreeMap::new();
    let mut frontier: Vec<RevisionId> = stale.iter().cloned().collect();
    frontier.sort();
    let mut depth = 0usize;

    while !frontier.is_empty() {
        depth += 1;
        let mut next: BTreeSet<RevisionId> = BTreeSet::new();
        for source in &frontier {
            for dependent in reverse.successors(source) {
                if stale.contains(dependent) || flagged.contains_key(dependent) {
                    continue;
                }
                // Live records only, both as a destination and as a route onward.
                if !ledger.get(dependent).is_some_and(Record::is_live) {
                    continue;
                }
                let via = edge_relation(ledger, dependent, source).unwrap_or(Relation::DependsOn);
                let mut path = vec![source.clone()];
                if let Some(parent) = flagged.get(source) {
                    path.extend(parent.path.iter().cloned());
                }
                next.insert(dependent.clone());
                flagged.entry(dependent.clone()).or_insert(AtRisk {
                    id: dependent.clone(),
                    depth,
                    via,
                    path,
                });
            }
        }
        frontier = next.into_iter().collect();
    }

    let mut out: Vec<AtRisk> = flagged.into_values().collect();
    out.sort_by(|a, b| (a.depth, &a.id).cmp(&(b.depth, &b.id)));
    out
}

/// The dependency graph over resolved revisions: an edge `a -> b` means "a rests on b".
///
/// Only the three propagating relations contribute. Edges are resolved through
/// [`Ledger::resolve`], so a floating reference points at whatever the head is and a
/// pinned one at exactly that revision — the same resolution every other stage uses.
#[must_use]
pub fn dependency_graph(ledger: &Ledger) -> DiGraph<RevisionId> {
    let mut graph = DiGraph::new();
    for record in sorted_records(ledger) {
        graph.add_node(record.id.clone());
        for relation in PROPAGATING_RELATIONS {
            for reference in record.targets(*relation) {
                if let Ok(Some(target)) = ledger.resolve(reference) {
                    graph.add_edge(record.id.clone(), target.id.clone());
                }
            }
        }
        for claim in &record.claims {
            for reference in &claim.supported_by {
                if let Ok(Some(target)) = ledger.resolve(reference) {
                    graph.add_edge(record.id.clone(), target.id.clone());
                }
            }
        }
    }
    graph
}

/// Which propagating relation carries `from -> to`, preferring the declaration order of
/// [`PROPAGATING_RELATIONS`] when a record declares more than one.
fn edge_relation(ledger: &Ledger, from: &RevisionId, to: &RevisionId) -> Option<Relation> {
    let record = ledger.get(from)?;
    for relation in PROPAGATING_RELATIONS {
        let direct = record
            .targets(*relation)
            .iter()
            .filter_map(|r| ledger.resolve(r).ok().flatten())
            .any(|t| &t.id == to);
        if direct {
            return Some(*relation);
        }
    }
    let via_claim = record
        .claims
        .iter()
        .flat_map(|c| &c.supported_by)
        .filter_map(|r| ledger.resolve(r).ok().flatten())
        .any(|t| &t.id == to);
    via_claim.then_some(Relation::SupportedBy)
}

/// Records in revision-identifier order, independent of insertion order.
pub(crate) fn sorted_records(ledger: &Ledger) -> Vec<&Record> {
    let mut records: Vec<&Record> = ledger.records().iter().collect();
    records.sort_by(|a, b| a.id.cmp(&b.id));
    records
}
