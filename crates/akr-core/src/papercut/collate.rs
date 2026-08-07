//! Collating papercuts from sister projects into a master record (D-030).
//!
//! `akr papercut collate` reads the live papercut heads of every workspace under a scan
//! directory — by default the siblings of the AKR workspace — and gathers every one not
//! already absorbed into a single new master papercut record in the AKR ledger. The
//! source keys land in the record's `collated` slot, which is the dedup set the next run
//! checks: a source papercut is processed once, and the sisters are never written to.
//!
//! Reading is all this module does; serialising and writing are the caller's, through the
//! one write pipeline of `docs/07-cli.md` §4, exactly as `akr papercut` itself works
//! (D-027).

use crate::model::{
    Commit, ContentSlot, ContentValue, Date, Kind, Ledger, LogicalKey, Record, RevisionId, State,
};
use crate::resolve::load_workspace;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// One source papercut, as the master record will list it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollatedPapercut {
    /// The sibling project's directory name.
    pub project: String,
    /// The source papercut's logical key, e.g. `bpg.papercut.fts5-error`.
    pub key: LogicalKey,
    /// The source papercut's one-line title.
    pub title: String,
}

/// What a scan of one directory found, and what remains to be collated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collate {
    /// The scan directory that was read.
    pub source: String,
    /// Directories under `source` that hold a workspace.
    pub projects: Vec<String>,
    /// Workspaces that failed to load and were skipped, by directory name.
    pub skipped: Vec<String>,
    /// Papercuts not yet collated anywhere, sorted by project then key.
    pub entries: Vec<CollatedPapercut>,
}

/// The keys a ledger has already absorbed, from the `collated` slot of its live
/// papercuts. Plain `akr papercut` records never carry the slot, so they contribute
/// nothing; only collation records do.
#[must_use]
pub fn already_collated(ledger: &Ledger) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for record in ledger.records() {
        if record.kind != Kind::Papercut || !record.is_live() {
            continue;
        }
        if let Some(ContentValue::Strings(keys)) = record.get(ContentSlot::Collated) {
            out.extend(keys.iter().cloned());
        }
    }
    out
}

/// Scans `scan_dir` for sibling workspaces and gathers their live papercut heads.
///
/// A sibling is a direct subdirectory of `scan_dir` that has an `.akr/` directory.
/// `exclude` names the caller's own workspace root, which is skipped even when it sits
/// inside `scan_dir`. Projects whose workspace fails to load are recorded in
/// `skipped` rather than failing the scan, so one broken sibling cannot block the rest.
/// `already` is the dedup set; keys in it are left out of `entries`.
#[must_use]
pub fn collect(scan_dir: &Path, exclude: &Path, already: &BTreeSet<String>) -> Collate {
    let source = scan_dir.to_string_lossy().into_owned();
    let mut projects = Vec::new();
    let mut skipped = Vec::new();
    let mut entries = Vec::new();

    let Ok(read) = std::fs::read_dir(scan_dir) else {
        return Collate {
            source,
            projects,
            skipped,
            entries,
        };
    };
    let mut dirs: Vec<std::path::PathBuf> = read
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();

    for dir in dirs {
        if same_dir(&dir, exclude) {
            continue;
        }
        let project = dir
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        let akr_dir = dir.join(".akr");
        if !akr_dir.is_dir() {
            continue;
        }
        let workspace = match load_workspace(&dir, &akr_dir) {
            Ok(workspace) => workspace,
            Err(_) => {
                skipped.push(project);
                continue;
            }
        };
        projects.push(project.clone());

        let mut keys: Vec<LogicalKey> = workspace
            .ledger
            .records()
            .iter()
            .filter(|record| record.kind == Kind::Papercut && record.is_live())
            .map(|record| record.id.key.clone())
            .collect();
        keys.sort();
        for key in keys {
            if already.contains(&key.to_string()) {
                continue;
            }
            let title = workspace
                .ledger
                .head(&key)
                .map_or_else(|_| String::new(), |record| record.title.clone());
            entries.push(CollatedPapercut {
                project: project.clone(),
                key,
                title,
            });
        }
    }

    Collate {
        source,
        projects,
        skipped,
        entries,
    }
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The master record `akr papercut collate` proposes.
///
/// Like [`super::LogPapercut`], this builds the record; serialising and writing are the
/// caller's. The message is generated here — the collation is the ceremony — and the
/// `collated` slot is the structured dedup key the next run reads back.
pub struct CollateRequest {
    /// The scan directory, named in the statement.
    pub source: String,
    /// Every sibling project that held a workspace, for the title.
    pub projects: Vec<String>,
    /// The papercuts being absorbed.
    pub entries: Vec<CollatedPapercut>,
    /// The commit the collation is observed at.
    pub observed_at: Commit,
    /// The authoring date.
    pub created_at: Date,
    /// Who ran it; lands in the `author` slot.
    pub author: Option<String>,
}

impl CollateRequest {
    /// Builds the record this request describes, without validating anything.
    #[must_use]
    pub fn to_record(&self, key: LogicalKey) -> Record {
        let mut content: BTreeMap<ContentSlot, ContentValue> = BTreeMap::new();
        content.insert(
            ContentSlot::Statement,
            ContentValue::Prose(self.statement()),
        );
        content.insert(
            ContentSlot::ObservedAt,
            ContentValue::Commit(self.observed_at.clone()),
        );
        content.insert(
            ContentSlot::Collated,
            ContentValue::Strings(self.entries.iter().map(|e| e.key.to_string()).collect()),
        );

        Record {
            id: RevisionId::new(key, 1),
            kind: Kind::Papercut,
            title: self.title(),
            // Empirical kinds have no proposal state (D-027).
            state: State::Verified,
            scope: Vec::new(),
            topic: None,
            content,
            claims: Vec::new(),
            retired_claims: Vec::new(),
            acceptance: None,
            dispositions: Vec::new(),
            relations: BTreeMap::new(),
            acknowledged: false,
            author: self.author.clone(),
            created_at: Some(self.created_at),
            sources: Vec::new(),
            file: None,
        }
    }

    /// The one-line label: how many papercuts, from which projects.
    fn title(&self) -> String {
        let projects = project_list(&self.projects, 4);
        format!(
            "Collated {} papercut{} from {}",
            self.entries.len(),
            if self.entries.len() == 1 { "" } else { "s" },
            projects
        )
    }

    /// The body: the provenance line, then one line per absorbed papercut.
    fn statement(&self) -> String {
        let mut out = format!(
            "Collated {} papercut{} from {} on {} (D-030). Each source key is in the \
             collated slot; see the owning project's ledger for the full statement.\n\n",
            self.entries.len(),
            if self.entries.len() == 1 { "" } else { "s" },
            self.source,
            self.created_at,
        );
        for entry in &self.entries {
            out.push_str(&format!("- {} @{} — {}\n", entry.project, entry.key, entry.title));
        }
        out
    }
}

/// A comma-separated list of names, truncated to `limit` with an "and N more" tail.
fn project_list(names: &[String], limit: usize) -> String {
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    match names.len() {
        0 => "no projects".to_owned(),
        1 => names[0].to_owned(),
        n if n <= limit => format!("{} and {}", names[..n - 1].join(", "), names[n - 1]),
        _ => format!("{} and {} more", names[..limit].join(", "), names.len() - limit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_list_truncates() {
        assert_eq!(project_list(&[], 4), "no projects");
        assert_eq!(project_list(&["a".to_owned()], 4), "a");
        assert_eq!(
            project_list(&["a".to_owned(), "b".to_owned()], 4),
            "a and b"
        );
        assert_eq!(
            project_list(&["a".to_owned(), "b".to_owned(), "c".to_owned()], 2),
            "a, b and 1 more"
        );
    }
}
