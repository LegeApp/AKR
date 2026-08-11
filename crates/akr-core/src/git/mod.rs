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

/// One entry of the git index: what is staged, at which mode, as which blob.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexEntry {
    /// The repository-relative path, `/`-separated on every platform.
    pub path: String,
    /// The file mode git recorded, e.g. `100644`.
    pub mode: String,
    /// The blob object id of the staged content.
    pub blob: String,
}

/// One commit as the session-head briefing needs to present it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitBrief {
    /// The commit object.
    pub commit: Commit,
    /// Its one-line subject.
    pub subject: String,
    /// `AKR-Work` trailer values, in authored order.
    pub work: Vec<String>,
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

    /// HEAD and the newest reachable commit carrying an `AKR-Work` trailer.
    ///
    /// These are deliberately separate answers: a maintenance commit remains the newest
    /// Git fact while the handoff focus stays on the last commit that named project work.
    /// Only history reachable from HEAD is considered; another branch must not become the
    /// current session merely because it was authored later.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when Git cannot read the history.
    pub fn session_head(&self) -> Result<(CommitBrief, Option<CommitBrief>), GitError> {
        let latest = self.commit_brief(&["show", "-s", "--format=%H%x1f%s%x1f%B", "HEAD"])?;
        let linked_text = self.run(&[
            "log",
            "HEAD",
            "-1",
            "--extended-regexp",
            "--grep=^AKR-Work:",
            "--format=%H%x1f%s%x1f%B",
        ])?;
        let linked = if linked_text.trim().is_empty() {
            None
        } else {
            Some(self.parse_commit_brief(&linked_text)?)
        };
        Ok((latest, linked))
    }

    fn commit_brief(&self, args: &[&str]) -> Result<CommitBrief, GitError> {
        let text = self.run(args)?;
        self.parse_commit_brief(&text)
    }

    fn parse_commit_brief(&self, text: &str) -> Result<CommitBrief, GitError> {
        let mut fields = text.splitn(3, '\u{1f}');
        let oid = fields.next().unwrap_or_default().trim();
        let subject = fields.next().unwrap_or_default().trim().to_owned();
        let body = fields.next().unwrap_or_default();
        let commit = Commit::new(oid).map_err(GitError::from)?;
        let work = body
            .lines()
            .filter_map(|line| line.trim().strip_prefix("AKR-Work:"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Ok(CommitBrief {
            commit,
            subject,
            work,
        })
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

    /// Reads one path at many commits through a single `cat-file --batch` process.
    fn file_versions(
        &self,
        commits: &[Commit],
        path: &str,
    ) -> Result<BTreeMap<Commit, Option<String>>, GitError> {
        let input = commits
            .iter()
            .map(|commit| format!("{}:{path}\n", commit.as_str()))
            .collect::<String>();
        let bytes = self.run_bytes_with_stdin(&["cat-file", "--batch"], &input)?;
        let mut cursor = 0usize;
        let mut out = BTreeMap::new();
        for commit in commits {
            let Some(relative_end) = bytes[cursor..].iter().position(|byte| *byte == b'\n') else {
                return Err(GitError::MalformedOutput(
                    "cat-file --batch response ended before its header".to_owned(),
                ));
            };
            let header_end = cursor + relative_end;
            let header = String::from_utf8_lossy(&bytes[cursor..header_end]);
            cursor = header_end + 1;
            if header.ends_with(" missing") {
                out.insert(commit.clone(), None);
                continue;
            }
            let size = header
                .split_whitespace()
                .nth(2)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| GitError::MalformedOutput(header.into_owned()))?;
            let content_end = cursor.saturating_add(size);
            if content_end > bytes.len() {
                return Err(GitError::MalformedOutput(
                    "cat-file --batch response ended inside an object".to_owned(),
                ));
            }
            let text = String::from_utf8_lossy(&bytes[cursor..content_end]).into_owned();
            cursor = content_end;
            if bytes.get(cursor) == Some(&b'\n') {
                cursor += 1;
            }
            out.insert(commit.clone(), Some(text));
        }
        Ok(out)
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

    /// Every entry of the git index, in path order.
    ///
    /// The *index*, deliberately, and not the working tree. `docs/16-change-protocol.md`
    /// §3: the staged tree is the synchronisation boundary, because a working tree with
    /// eighteen modified files does not say which of them belong to the change being made.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when git cannot read the index.
    pub fn staged_entries(&self) -> Result<Vec<IndexEntry>, GitError> {
        let text = self.run(&["ls-files", "--stage"])?;
        let mut out = Vec::new();
        for line in text.lines() {
            // `<mode> <object> <stage>\t<path>`
            let Some((meta, path)) = line.split_once('\t') else {
                continue;
            };
            let fields: Vec<&str> = meta.split_whitespace().collect();
            if fields.len() < 3 {
                continue;
            }
            out.push(IndexEntry {
                mode: fields[0].to_owned(),
                blob: fields[1].to_owned(),
                path: path.replace('\\', "/"),
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// The contents of a blob.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when the object is missing or unreadable.
    pub fn blob(&self, oid: &str) -> Result<String, GitError> {
        self.run(&["cat-file", "blob", oid])
    }

    /// Writes the current index out as a tree and returns its object id.
    ///
    /// Cheap, and it changes neither `HEAD` nor the index — `write-tree` only
    /// materialises what is already staged. It is how a prepared transaction notices that
    /// the staged tree moved underneath it.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when the index cannot be written as a tree.
    pub fn write_tree(&self) -> Result<String, GitError> {
        Ok(self.run(&["write-tree"])?.trim().to_owned())
    }

    /// A path inside the git directory of *this worktree*, resolved by git.
    ///
    /// `--git-path` rather than `.git/`: in a linked worktree those are different
    /// directories, and a change transaction is per worktree by construction.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when git cannot resolve the path.
    pub fn git_path(&self, relative: &str) -> Result<PathBuf, GitError> {
        let text = self.run(&["rev-parse", "--git-path", relative])?;
        let raw = PathBuf::from(text.trim());
        Ok(if raw.is_absolute() {
            raw
        } else {
            self.root.join(raw)
        })
    }

    /// Whether the index differs from `HEAD` at all.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when git cannot compare them.
    pub fn has_staged_changes(&self) -> Result<bool, GitError> {
        Ok(self.status(&["diff", "--cached", "--quiet"])? != 0)
    }

    /// Commits the index with `message`, returning the new commit.
    ///
    /// Git is asked to make the commit rather than reimplemented, which is the whole
    /// shape of the bridge (`docs/16-change-protocol.md` §1): AKR leads the intent, git
    /// seals the snapshot.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when the commit is refused — an empty index, a failing
    /// hook, or an unconfigured identity.
    pub fn commit(&self, message: &str) -> Result<Commit, GitError> {
        self.run(&["commit", "-m", message])?;
        self.head()
    }

    /// Commits reachable from `range` whose message carries `trailer`.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when the range does not resolve.
    pub fn log_grep(&self, range: &str, trailer: &str) -> Result<Vec<(Commit, String)>, GitError> {
        let text = self.run(&[
            "log",
            range,
            &format!("--grep={trailer}"),
            "--format=%H\t%s",
        ])?;
        let mut out = Vec::new();
        for line in text.lines() {
            let Some((oid, subject)) = line.split_once('\t') else {
                continue;
            };
            if let Ok(commit) = Commit::new(oid.trim()) {
                out.push((commit, subject.to_owned()));
            }
        }
        Ok(out)
    }

    /// Which of `commits` the repository actually has, in one call.
    ///
    /// One `cat-file --batch-check` instead of one `rev-parse` per commit. On a ledger
    /// with a few hundred evidence citations the difference is a few hundred process
    /// spawns, which is most of what made `akr check` take two minutes on a real
    /// repository (`saveyourskin.papercut.akr-check-akr-build-akr-lock-update-each-took`).
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when git cannot be started. A commit git does not
    /// recognise is absent from the result, not an error: that is the question being asked.
    pub fn contains_all(&self, commits: &[Commit]) -> Result<BTreeSet<Commit>, GitError> {
        if commits.is_empty() {
            return Ok(BTreeSet::new());
        }
        let mut input = String::new();
        for commit in commits {
            input.push_str(commit.as_str());
            input.push('\n');
        }
        let text = self.run_with_stdin(
            &["cat-file", "--batch-check=%(objectname) %(objecttype)"],
            &input,
        )?;
        let mut out = BTreeSet::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            let (Some(oid), Some(kind)) = (fields.next(), fields.next()) else {
                continue;
            };
            if kind == "commit"
                && let Ok(commit) = Commit::new(oid)
            {
                out.insert(commit);
            }
        }
        Ok(out)
    }

    /// `commits` in topological order, newest first, from one history walk.
    ///
    /// This replaces a comparison sort whose comparator spawned `git merge-base
    /// --is-ancestor`: O(n log n) processes to answer a question git will answer once.
    /// `--topo-order` guarantees that no commit is listed before one that descends from
    /// it, which is exactly the order the ancestry table needs.
    ///
    /// Commits git does not place are omitted; the caller falls back to a deterministic
    /// tiebreak for those rather than inventing an ancestry.
    ///
    /// # Errors
    /// [`GitError::CommandFailed`] when the walk fails.
    pub fn topological_order(&self, commits: &[Commit]) -> Result<Vec<Commit>, GitError> {
        if commits.is_empty() {
            return Ok(Vec::new());
        }
        let mut args: Vec<String> = vec!["rev-list".to_owned(), "--topo-order".to_owned()];
        args.extend(commits.iter().map(|c| c.as_str().to_owned()));
        let text = self.run(&args)?;
        let wanted: BTreeSet<&str> = commits.iter().map(Commit::as_str).collect();
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|line| wanted.contains(line))
            .filter_map(|line| Commit::new(line).ok())
            .collect())
    }

    // -- process plumbing -------------------------------------------------------------

    fn run<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<String, GitError> {
        run_in(&self.root, args)
    }

    fn run_with_stdin<S: AsRef<OsStr>>(&self, args: &[S], input: &str) -> Result<String, GitError> {
        use std::io::Write as _;
        use std::process::Stdio;
        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GitError::CommandFailed {
                subcommand: describe(args),
                stderr: e.to_string(),
            })?;
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(input.as_bytes());
        }
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .map_err(|e| GitError::CommandFailed {
                subcommand: describe(args),
                stderr: e.to_string(),
            })?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    fn run_bytes_with_stdin<S: AsRef<OsStr>>(
        &self,
        args: &[S],
        input: &str,
    ) -> Result<Vec<u8>, GitError> {
        use std::io::Write as _;
        use std::process::Stdio;
        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| GitError::CommandFailed {
                subcommand: describe(args),
                stderr: e.to_string(),
            })?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(input.as_bytes())
                .map_err(|e| GitError::CommandFailed {
                    subcommand: describe(args),
                    stderr: e.to_string(),
                })?;
        }
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .map_err(|e| GitError::CommandFailed {
                subcommand: describe(args),
                stderr: e.to_string(),
            })?;
        if !output.status.success() {
            return Err(GitError::CommandFailed {
                subcommand: describe(args),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(output.stdout)
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
    let distinct: Vec<Commit> = commits
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    // One `cat-file --batch-check` rather than one `rev-parse` each.
    let present = repository.contains_all(&distinct)?;
    let known: Vec<Commit> = distinct
        .into_iter()
        .filter(|commit| present.contains(commit))
        .collect();

    // One history walk rather than a comparison sort whose comparator forks git.
    //
    // The old shape was O(n log n) `merge-base --is-ancestor` processes, and on a ledger
    // with a few hundred evidence citations over a few hundred commits that dominated
    // `akr check`, `akr build` and `akr lock --update` alike — two minutes each, reported
    // from a real port session. `rev-list --topo-order` answers the whole question once.
    let mut ordered = repository.topological_order(&known)?;
    ordered.reverse(); // oldest first: `a` before `b` when `b` descends from `a`.

    // Anything git would not place — it should place everything, but a corrupt or grafted
    // history is not this function's problem to diagnose — keeps a deterministic position
    // rather than an invented ancestry.
    if ordered.len() != known.len() {
        let placed: BTreeSet<&Commit> = ordered.iter().collect();
        let mut missing: Vec<Commit> = known
            .iter()
            .filter(|commit| !placed.contains(commit))
            .cloned()
            .collect();
        missing.sort();
        missing.extend(ordered);
        ordered = missing;
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
    // Records are conventionally grouped into a handful of ledger files. Walking the
    // same file history once per record made session startup O(records × file history)
    // and spawned hundreds of git processes in medium-sized projects. Read and parse
    // each historical file version once, then advance every record in that file together.
    let mut by_file: BTreeMap<String, BTreeSet<crate::model::RevisionId>> = BTreeMap::new();
    for record in crate::graph::sorted_records(ledger) {
        let Some(path) = &record.file else { continue };
        by_file
            .entry(path.clone())
            .or_default()
            .insert(record.id.clone());
    }

    let mut out = BTreeMap::new();
    for (path, wanted) in by_file {
        let commits = repository.commits_touching(&path)?;
        let Some(newest) = commits.first() else {
            continue;
        };
        let versions = repository.file_versions(&commits, &path)?;
        let current = versions
            .get(newest)
            .and_then(Option::as_deref)
            .map_or_else(BTreeMap::new, |text| definitional_hashes(text, &wanted));
        let mut unresolved: BTreeSet<crate::model::RevisionId> = current.keys().cloned().collect();
        let mut answers: BTreeMap<crate::model::RevisionId, Commit> = unresolved
            .iter()
            .cloned()
            .map(|id| (id, newest.clone()))
            .collect();

        for commit in commits.iter().skip(1) {
            if unresolved.is_empty() {
                break;
            }
            let older = versions
                .get(commit)
                .and_then(Option::as_deref)
                .map_or_else(BTreeMap::new, |text| definitional_hashes(text, &unresolved));
            unresolved.retain(|id| {
                if older.get(id) == current.get(id) {
                    answers.insert(id.clone(), commit.clone());
                    true
                } else {
                    false
                }
            });
        }
        out.extend(answers);
    }
    Ok(out)
}

/// Definitional hashes for the requested revisions in one historical file version.
fn definitional_hashes(
    text: &str,
    wanted: &BTreeSet<crate::model::RevisionId>,
) -> BTreeMap<crate::model::RevisionId, crate::model::ContentHash> {
    let parsed = crate::syntax::parse(text, crate::diagnostics::FileId(0));
    let Some(file) = parsed.file else {
        return BTreeMap::new();
    };
    let mut out = BTreeMap::new();
    for (at, item) in file.items.iter().enumerate() {
        let crate::syntax::cst::Item::Record(record) = item else {
            continue;
        };
        let Ok(key) = crate::model::LogicalKey::parse(&record.key) else {
            continue;
        };
        let id = crate::model::RevisionId::new(key, record.revision);
        if !wanted.contains(&id) {
            continue;
        }
        if let Some(definitional) = crate::resolve::definitional_record_text(&file, at) {
            out.insert(id, crate::hash::content_hash(&definitional));
        }
    }
    out
}
