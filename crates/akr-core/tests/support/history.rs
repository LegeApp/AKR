//! Materialising a worked example's synthetic git history as a real repository.
//!
//! Both worked examples freeze a commit table in their `MANIFEST.md` and then freeze what
//! the tools must say about it. Those expectations are only worth having if something
//! checks them, and the only honest way to check a freshness computation is against a real
//! repository — the whole point of phase P5 is that ancestry and changed-path detection are
//! answered by git rather than by a model that might drift from it.
//!
//! This builds such a repository from a table of commits, then repoints the example's
//! records at the hashes it produced.
//!
//! # The hashes cannot be the manifest's
//!
//! A manifest's commit hashes are invented, because the histories are fictional. A real
//! repository necessarily produces different ones. Everything *else* the manifest claims —
//! which records go stale, why, what goes at risk, at what depth, along which relation —
//! is asserted exactly, and those are the claims the manifest actually makes.

#![allow(dead_code)]

use super::TempRepo;
use akr_core::model::{Commit, ContentSlot, ContentValue, Ledger};
use std::collections::BTreeMap;

/// One commit of a manifest's history table: a message and the files it writes.
pub struct Step<'a> {
    /// What the commit does, for the git log.
    pub message: &'a str,
    /// `(path, contents)` pairs written before committing.
    pub writes: &'a [(&'a str, &'a str)],
}

impl<'a> Step<'a> {
    /// A step that writes the given files.
    pub fn new(message: &'a str, writes: &'a [(&'a str, &'a str)]) -> Self {
        Self { message, writes }
    }
}

/// A materialised history: the repository, and the commits in table order.
pub struct SyntheticHistory {
    repo: TempRepo,
    commits: Vec<Commit>,
}

impl SyntheticHistory {
    /// Replays the steps into a fresh repository, one commit each.
    pub fn build(name: &str, steps: &[Step<'_>]) -> Self {
        let mut repo = TempRepo::new(name);
        let mut commits = Vec::new();
        for step in steps {
            for (path, contents) in step.writes {
                repo.write(path, contents);
            }
            let hash = repo.commit(step.message);
            commits.push(Commit::new(&hash).expect("a full hash"));
        }
        Self { repo, commits }
    }

    /// The nth commit, 1-based to match a manifest's C1, C2, … numbering.
    pub fn at(&self, n: usize) -> &Commit {
        &self.commits[n - 1]
    }

    /// The last commit; a manifest's `HEAD`.
    pub fn head(&self) -> &Commit {
        self.commits.last().expect("at least one commit")
    }

    /// A repository handle.
    pub fn git(&self) -> akr_core::git::Repository {
        akr_core::git::Repository::open(self.repo.root()).expect("opens")
    }

    /// The underlying repository, for tests that add commits of their own.
    pub fn repo_mut(&mut self) -> &mut TempRepo {
        &mut self.repo
    }

    /// Rewrites every `observed_at` and `as_of` in a ledger from a manifest's invented
    /// hash to the real commit that stands in for it.
    ///
    /// `mapping` pairs each manifest hash with its 1-based position in the table. A commit
    /// slot the mapping does not mention is left alone, so a record that deliberately
    /// cites a stranded commit stays stranded.
    pub fn remap(&self, ledger: &Ledger, mapping: &[(&str, usize)]) -> Ledger {
        let table: BTreeMap<&str, &Commit> = mapping
            .iter()
            .map(|(hash, n)| (*hash, self.at(*n)))
            .collect();

        let records: Vec<_> = ledger
            .records()
            .iter()
            .map(|record| {
                let mut copy = record.clone();
                for slot in [ContentSlot::ObservedAt, ContentSlot::AsOf] {
                    if let Some(ContentValue::Commit(old)) = copy.content.get(&slot)
                        && let Some(real) = table.get(old.as_str())
                    {
                        copy.content
                            .insert(slot, ContentValue::Commit((*real).clone()));
                    }
                }
                copy
            })
            .collect();

        let mut out = Ledger::new(ledger.project.clone());
        out.facts = ledger.facts.clone();
        out.extend(records);
        out
    }
}
