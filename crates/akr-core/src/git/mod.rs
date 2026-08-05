//! Git, through the subprocess: commits, ancestry, and changed paths.
//!
//! Git is the clock. AKR has no notion of time other than commits and two dates, and
//! "is this observation still current?" is answered as "did any commit between its
//! `observed_at` and `HEAD` touch a path it watches?" — a question git answers exactly
//! (`docs/01-architecture.md` §5).
//!
//! # Subprocess, not a library
//!
//! `docs/13-implementation-roadmap.md` §4: "Git is invoked as a subprocess or through a
//! library; either is fine, and the subprocess route is one fewer build-time dependency."
//! Every call here is a plumbing command with a stable, machine-oriented output format —
//! `rev-parse`, `merge-base --is-ancestor`, `rev-list`, `log --name-status`, `status
//! --porcelain`, `cat-file` — and each is wrapped so that a failure becomes an
//! `AKR-G`-coded diagnostic rather than a panic.
//!
//! # Read-only, always
//!
//! Nothing in this module writes to the repository. The build never commits, never
//! stages, and never touches a `.akr` file (D-003, `docs/10-freshness-and-git.md` §8).
//! Every subcommand used here is a query.

use crate::diagnostics::{Diagnostic, RuleId, Subject};
use crate::model::{Commit, IdentError};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The freshness diagnostics this module raises.
///
/// Registered in `spec/diagnostics/codes-runtime.md`; the `G` range belongs to the runtime
/// half of the design set (`spec/diagnostics/README.md` §2), so they live here rather than
/// in [`crate::diagnostics::codes`], which is asserted to be language-stage only.
pub mod codes {
    use crate::diagnostics::Code;

    /// Not a git repository.
    pub const G001: Code = Code::new("AKR-G001");
    /// A git invocation failed.
    pub const G002: Code = Code::new("AKR-G002");
    /// History is shallow; ancestry is undecidable.
    pub const G003: Code = Code::new("AKR-G003");
    /// The working tree has uncommitted changes to watched paths.
    pub const G004: Code = Code::new("AKR-G004");
    /// An `observed_at` commit is not in the repository.
    pub const G011: Code = Code::new("AKR-G011");
    /// An `observed_at` commit is not an ancestor of HEAD.
    pub const G012: Code = Code::new("AKR-G012");
    /// An unknown revision argument.
    pub const G013: Code = Code::new("AKR-G013");
    /// A malformed watch glob.
    pub const G021: Code = Code::new("AKR-G021");
    /// A watch glob that matches nothing.
    pub const G022: Code = Code::new("AKR-G022");
    /// `review_after` precedes `created_at`.
    pub const G031: Code = Code::new("AKR-G031");
    /// The review queue is not empty, under `--review-clean`.
    pub const G041: Code = Code::new("AKR-G041");

    /// Every freshness code this crate can raise.
    pub const ALL: &[Code] = &[
        G001, G002, G003, G004, G011, G012, G013, G021, G022, G031, G041,
    ];
}

/// V-101 through V-104 (`docs/10-freshness-and-git.md` §9).
pub(crate) const V101: RuleId = RuleId(101);
pub(crate) const V102: RuleId = RuleId(102);
pub(crate) const V103: RuleId = RuleId(103);
pub(crate) const V104: RuleId = RuleId(104);

// -------------------------------------------------------------------------------------
// Errors
// -------------------------------------------------------------------------------------

/// Why a git query could not be answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitError {
    /// The path is not inside a git repository (`AKR-G001`).
    NotARepository(PathBuf),
    /// A `git` subcommand failed, or `git` is not installed (`AKR-G002`).
    CommandFailed {
        /// The subcommand, for the message.
        subcommand: String,
        /// What git said.
        stderr: String,
    },
    /// The repository has a truncated history (`AKR-G003`).
    ShallowHistory,
    /// A revision argument does not name a commit (`AKR-G013`).
    UnknownRevision(String),
    /// Git returned something this module could not read as a commit.
    MalformedOutput(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotARepository(path) => {
                write!(f, "{} is not inside a git repository", path.display())
            }
            Self::CommandFailed { subcommand, stderr } => {
                write!(f, "git {subcommand} failed: {}", stderr.trim())
            }
            Self::ShallowHistory => f.write_str("repository history is shallow"),
            Self::UnknownRevision(rev) => write!(f, "{rev} is not a commit in this repository"),
            Self::MalformedOutput(text) => write!(f, "unreadable git output: {text}"),
        }
    }
}

impl std::error::Error for GitError {}

impl From<IdentError> for GitError {
    fn from(error: IdentError) -> Self {
        Self::MalformedOutput(error.to_string())
    }
}

impl GitError {
    /// The diagnostic this error renders as.
    #[must_use]
    pub fn to_diagnostic(&self) -> Diagnostic {
        let (code, rule) = match self {
            Self::NotARepository(_) => (codes::G001, V101),
            Self::ShallowHistory => (codes::G003, V101),
            Self::UnknownRevision(_) => (codes::G013, V101),
            Self::CommandFailed { .. } | Self::MalformedOutput(_) => (codes::G002, V101),
        };
        Diagnostic::error(code, rule, Subject::Ledger, self.to_string())
    }
}

// -------------------------------------------------------------------------------------
// Repository
// -------------------------------------------------------------------------------------

/// One path touched by one commit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Touch {
    /// The commit that touched it.
    pub commit: Commit,
    /// The repo-root-relative path, with forward slashes.
    pub path: String,
}

/// A handle on a git repository.
///
/// Cheap to construct and cheap to clone: it holds a working directory and a memo of the
/// answers already fetched. Nothing is cached across instances, so a test that mutates a
/// repository makes a new handle and sees the new history.
#[derive(Debug, Clone)]
pub struct Repository {
    root: PathBuf,
}

impl Repository {
    /// Opens the repository containing `path`.
    ///
    /// # Errors
    /// [`GitError::NotARepository`] when there is no repository, and
    /// [`GitError::ShallowHistory`] when the clone is `--depth`-limited: a truncated
    /// history cannot answer the descendant question that D-016 and D-024 depend on.
    pub fn open(path: &Path) -> Result<Self, GitError> {
        let root = run_in(path, &["rev-parse", "--show-toplevel"])
            .map_err(|_| GitError::NotARepository(path.to_path_buf()))?;
        let root = PathBuf::from(root.trim());
        let repository = Self { root };
        if repository
            .run(&["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true"
        {
            return Err(GitError::ShallowHistory);
        }
        Ok(repository)
    }

    /// The repository root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves a revision to a commit.
    ///
    /// # Errors
    /// [`GitError::UnknownRevision`] when the revision does not name a commit.
    pub fn rev_parse(&self, revision: &str) -> Result<Commit, GitError> {
        let text = self
            .run(&["rev-parse", "--verify", &format!("{revision}^{{commit}}")])
            .map_err(|_| GitError::UnknownRevision(revision.to_owned()))?;
        Commit::new(text.trim()).map_err(GitError::from)
    }

    /// The current `HEAD` commit.
    ///
    /// # Errors
    /// [`GitError::UnknownRevision`] in a repository with no commits.
    pub fn head(&self) -> Result<Commit, GitError> {
        self.rev_parse("HEAD")
    }

    /// Whether a commit exists in this repository.
    #[must_use]
    pub fn contains(&self, commit: &Commit) -> bool {
        self.rev_parse(commit.as_str()).is_ok()
    }

    /// Whether `descendant` is `ancestor`, or is reachable from it.
    ///
    /// This is the D-016 condition — "evidence observed at a commit that descends from the
    /// last content change" — and the D-024 reachability test, asked of git rather than
    /// reconstructed, so merges and octopus merges are handled by the tool that
    /// understands them.
    ///
    /// # Errors
    /// [`GitError::UnknownRevision`] when either commit is absent.
    pub fn is_descendant(&self, descendant: &Commit, ancestor: &Commit) -> Result<bool, GitError> {
        if descendant == ancestor {
            return Ok(true);
        }
        for commit in [descendant, ancestor] {
            if !self.contains(commit) {
                return Err(GitError::UnknownRevision(commit.as_str().to_owned()));
            }
        }
        // `merge-base --is-ancestor A B` exits 0 when A is an ancestor of B.
        match self.status(&[
            "merge-base",
            "--is-ancestor",
            ancestor.as_str(),
            descendant.as_str(),
        ]) {
            Ok(0) => Ok(true),
            Ok(_) => Ok(false),
            Err(error) => Err(error),
        }
    }

    /// The commits in `(from, to]`, newest first.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] if git cannot walk the range.
    pub fn commits_in(&self, from: Option<&Commit>, to: &Commit) -> Result<Vec<Commit>, GitError> {
        let range = match from {
            Some(from) => format!("{}..{}", from.as_str(), to.as_str()),
            None => to.as_str().to_owned(),
        };
        let text = self.run(&["rev-list", &range])?;
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Commit::new(l.trim()).map_err(GitError::from))
            .collect()
    }

    /// Every `(commit, path)` touched in `(from, to]`, sorted.
    ///
    /// A rename contributes **both** of its paths: an observation watching the old
    /// location and one watching the new location have each had their subject moved out
    /// from under them, which is exactly the change staleness exists to notice.
    ///
    /// This is the single bulk query of `docs/10-freshness-and-git.md` §3 step 3. It is
    /// issued once for a whole build rather than once per record, which is what turns
    /// O(records × history) into O(history + records × globs).
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] if git cannot walk the range.
    pub fn touches_in(&self, from: Option<&Commit>, to: &Commit) -> Result<Vec<Touch>, GitError> {
        let range = match from {
            Some(from) => format!("{}..{}", from.as_str(), to.as_str()),
            None => to.as_str().to_owned(),
        };
        // `-m` so a merge reports the paths it changed rather than nothing at all.
        let text = self.run(&[
            "log",
            "--name-status",
            "-M",
            "-m",
            "--format=commit %H",
            &range,
        ])?;

        let mut out = BTreeSet::new();
        let mut current: Option<Commit> = None;
        for line in text.lines() {
            if let Some(hash) = line.strip_prefix("commit ") {
                current = Commit::new(hash.trim()).ok();
                continue;
            }
            let Some(commit) = &current else { continue };
            let mut fields = line.split('\t');
            let Some(status) = fields.next() else {
                continue;
            };
            if status.trim().is_empty() {
                continue;
            }
            for path in fields.filter(|p| !p.trim().is_empty()) {
                out.insert(Touch {
                    commit: commit.clone(),
                    path: path.trim().to_owned(),
                });
            }
        }
        Ok(out.into_iter().collect())
    }

    /// Paths with uncommitted changes in the working tree or the index.
    ///
    /// Freshness is computed from committed history only, so these are invisible to it.
    /// `AKR-G004` reports them so that nobody is misled by a clean queue on a dirty tree
    /// (`docs/10-freshness-and-git.md` §8).
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] if git cannot read the status.
    pub fn working_tree_changes(&self) -> Result<BTreeSet<String>, GitError> {
        let text = self.run(&["status", "--porcelain", "-z", "--untracked-files=all"])?;
        let mut out = BTreeSet::new();
        for entry in text.split('\0').filter(|e| e.len() > 3) {
            // `XY <path>`; rename entries carry the origin in the following NUL field,
            // which this loop sees as a short entry and skips.
            out.insert(entry[3..].trim().to_owned());
        }
        Ok(out)
    }

    /// The commits that touched one path, newest first.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] if git cannot walk the history.
    pub fn commits_touching(&self, path: &str) -> Result<Vec<Commit>, GitError> {
        let text = self.run(&["log", "--format=%H", "--follow", "--", path])?;
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Commit::new(l.trim()).map_err(GitError::from))
            .collect()
    }

    /// The contents of a path at a commit, or `None` when it did not exist there.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] only for a failure that is not "no such path".
    pub fn file_at(&self, commit: &Commit, path: &str) -> Result<Option<String>, GitError> {
        match self.run(&["show", &format!("{}:{path}", commit.as_str())]) {
            Ok(text) => Ok(Some(text)),
            Err(GitError::CommandFailed { .. }) => Ok(None),
            Err(other) => Err(other),
        }
    }

    /// Every tracked path at a commit, sorted.
    ///
    /// Used by V-102's "matches nothing" check, which needs the tree rather than the
    /// history.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] if git cannot read the tree.
    pub fn run_ls_tree(&self, commit: &Commit) -> Result<BTreeSet<String>, GitError> {
        let text = self.run(&["ls-tree", "-r", "--name-only", commit.as_str()])?;
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }

    // -- process plumbing -------------------------------------------------------------

    fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<String, GitError> {
        run_in(&self.root, args)
    }

    fn status<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<i32, GitError> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| GitError::CommandFailed {
                subcommand: describe(args),
                stderr: e.to_string(),
            })?;
        Ok(output.status.code().unwrap_or(-1))
    }
}

fn describe<S: AsRef<OsStr>>(args: &[S]) -> String {
    args.iter()
        .map(|a| a.as_ref().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

fn run_in<S: AsRef<OsStr>>(dir: &Path, args: &[S]) -> Result<String, GitError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|e| GitError::CommandFailed {
            subcommand: describe(args),
            stderr: e.to_string(),
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(GitError::CommandFailed {
            subcommand: describe(args),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

// -------------------------------------------------------------------------------------
// Ancestry for LedgerFacts
// -------------------------------------------------------------------------------------

/// Builds the [`Ancestry`](crate::model::Ancestry) that V-020 reads, over the commits a
/// ledger actually references.
///
/// The commits are sorted oldest-first by asking git, then chained parent-to-child, so
/// that `is_descendant` over any pair of them returns what git would have returned.
///
/// # Precision
///
/// [`Ancestry`](crate::model::Ancestry) holds one parent per commit, so it represents a
/// chain rather than a DAG. That is exact whenever the referenced commits are totally
/// ordered by ancestry — linear history, and any history where the commits AKR cites sit
/// on one line of development, which is the normal case for `observed_at` values. Where
/// two referenced commits are on genuinely divergent branches, the chain reports the pair
/// as unrelated, which is the conservative answer: an acceptance check reads as *not*
/// satisfied rather than falsely satisfied.
///
/// Prefer [`Repository::is_descendant`] wherever a repository is in hand; this exists to
/// fill the facts for rules that have only the ledger.
///
/// # Errors
/// Propagates any git failure.
pub fn ancestry_over(
    repository: &Repository,
    commits: impl IntoIterator<Item = Commit>,
) -> Result<crate::model::Ancestry, GitError> {
    let known: Vec<Commit> = commits
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|c| repository.contains(c))
        .collect();

    // Sort oldest-first: `a` before `b` when `b` descends from `a`.
    let mut ordered = known.clone();
    let mut failure = None;
    ordered.sort_by(|a, b| {
        if a == b {
            return std::cmp::Ordering::Equal;
        }
        match repository.is_descendant(b, a) {
            Ok(true) => std::cmp::Ordering::Less,
            Ok(false) => match repository.is_descendant(a, b) {
                Ok(true) => std::cmp::Ordering::Greater,
                Ok(false) => a.as_str().cmp(b.as_str()),
                Err(error) => {
                    failure = Some(error);
                    std::cmp::Ordering::Equal
                }
            },
            Err(error) => {
                failure = Some(error);
                std::cmp::Ordering::Equal
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }

    let pairs: Vec<(Commit, Commit)> = ordered
        .windows(2)
        .map(|pair| (pair[1].clone(), pair[0].clone()))
        .collect();
    Ok(crate::model::Ancestry::from_pairs(pairs))
}

/// The last commit at which a record's canonical text reached its current value.
///
/// This is the `last_change` of D-016 — the commit an acceptance check's evidence must
/// descend from. It is computed over the record, not over its file: adding an unrelated
/// record to the same file must not invalidate every check in it.
///
/// Walks the commits that touched the record's file, newest first, hashing the record at
/// each; the answer is the oldest commit whose hash still equals the current one. A
/// commit where the record did not yet exist ends the walk, so a record's introduction is
/// its own last change.
///
/// Returns `None` when the record is not in the file at `HEAD`, or when the file is not
/// tracked — an uncommitted record has no last change, and inventing one would make an
/// acceptance verdict out of nothing.
///
/// # Errors
/// Propagates any git failure.
pub fn last_change_of(
    repository: &Repository,
    path: &str,
    key: &crate::model::LogicalKey,
    revision: u32,
) -> Result<Option<Commit>, GitError> {
    let commits = repository.commits_touching(path)?;
    let Some(newest) = commits.first() else {
        return Ok(None);
    };
    let Some(current) = hash_at(repository, newest, path, key, revision)? else {
        return Ok(None);
    };

    let mut answer = newest.clone();
    for commit in commits.iter().skip(1) {
        match hash_at(repository, commit, path, key, revision)? {
            Some(hash) if hash == current => answer = commit.clone(),
            _ => break,
        }
    }
    Ok(Some(answer))
}

/// The *definitional* content hash of one record as it stood at one commit (D-029): the
/// canonical text with lifecycle and completion bookkeeping (`state`, `note`, a check's
/// `verified_by`) removed, so `last_change_of` reports the last redefinition, not the last
/// lifecycle transition.
fn hash_at(
    repository: &Repository,
    commit: &Commit,
    path: &str,
    key: &crate::model::LogicalKey,
    revision: u32,
) -> Result<Option<crate::model::ContentHash>, GitError> {
    let Some(text) = repository.file_at(commit, path)? else {
        return Ok(None);
    };
    let parsed = crate::syntax::parse(&text, crate::diagnostics::FileId(0));
    let Some(file) = parsed.file else {
        return Ok(None);
    };
    for (at, item) in file.items.iter().enumerate() {
        let crate::syntax::cst::Item::Record(record) = item else {
            continue;
        };
        if record.revision != revision || record.key != key.to_string() {
            continue;
        }
        // D-029: `last_change` tracks the last *definitional* change, so a completion or
        // abandonment (state, a check's `verified_by`, or a `note`) does not count as the
        // record's newest content change and strand the evidence that closes it. The D-015
        // seal still hashes the whole record via `canonical_record_text`.
        if let Some(definitional) = crate::resolve::definitional_record_text(&file, at) {
            return Ok(Some(crate::hash::content_hash(&definitional)));
        }
    }
    Ok(None)
}

/// Fills `last_change` for every revision of a ledger whose file is tracked.
///
/// # Errors
/// Propagates any git failure.
pub fn last_changes(
    repository: &Repository,
    ledger: &crate::model::Ledger,
) -> Result<BTreeMap<crate::model::RevisionId, Commit>, GitError> {
    let mut out = BTreeMap::new();
    for record in crate::graph::sorted_records(ledger) {
        let Some(path) = &record.file else { continue };
        if let Some(commit) = last_change_of(repository, path, &record.id.key, record.id.revision)?
        {
            out.insert(record.id.clone(), commit);
        }
    }
    Ok(out)
}
