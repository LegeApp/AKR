//! Argument parsing: global flags, subcommands, and the usage errors of
//! `docs/07-cli.md` §2.
//!
//! Hand-rolled, for the reason the JSON writer is: the surface is small, the exit-status
//! contract is specific (`AKR-C001`–`AKR-C005` all exit 2), and a parser that produces
//! its own error strings would have to be talked out of them.

use akr_core::{
    ingest::TableMode,
    model::{Commit, Date, Glob},
};
use std::path::PathBuf;

/// Output form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// For humans; may be reworded between versions.
    #[default]
    Text,
    /// The stable envelope of `docs/07-cli.md` §5.
    Json,
}

/// The diagnostic profile (D-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    /// Warnings are errors. The default.
    #[default]
    Strict,
    /// Warnings stay warnings. For `akr import` on legacy material, and nothing else.
    Lenient,
}

/// How much of a record `akr get` and `knowledge.get` return.
///
/// The default is `body`, not `canonical`. Canonical AKR syntax is rarely what a reader
/// wants and is the largest part of the payload, so it is an explicit request: a tool
/// whose cheapest call returns the most bytes will be called that way every time
/// (`sources/context-reduction.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Detail {
    /// Identity, title, state, scope, relation counts, freshness, source locators.
    Summary,
    /// Summary plus content slots, claims, checks and full relations (the default).
    #[default]
    Body,
    /// Body plus the canonically formatted AKR source text.
    Canonical,
}

impl Detail {
    /// Parses the `--detail` argument.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "summary" => Some(Self::Summary),
            "body" => Some(Self::Body),
            "canonical" => Some(Self::Canonical),
            _ => None,
        }
    }

    /// The name used on the command line and over MCP.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Body => "body",
            Self::Canonical => "canonical",
        }
    }
}

/// Flags that apply to every command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    /// Where to start looking for the workspace.
    pub dir: PathBuf,
    /// Strict or lenient.
    pub profile: Profile,
    /// Text or JSON.
    pub format: Format,
    /// Resolve against this commit instead of HEAD.
    pub at: Option<Commit>,
    /// The date `review_after` is compared against. An input, never a clock reading.
    pub today: Option<Date>,
    /// Fail rather than rebuild the index.
    pub no_rebuild: bool,
    /// Suppress progress lines.
    pub quiet: bool,
}

impl Default for Global {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("."),
            profile: Profile::Strict,
            format: Format::Text,
            at: None,
            today: None,
            no_rebuild: false,
            quiet: false,
        }
    }
}

/// A parsed invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    /// The global flags.
    pub global: Global,
    /// The command.
    pub command: Command,
}

/// The commands of `docs/07-cli.md` §6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Print help and exit 0.
    Help,
    /// Print one command's help and exit 0: `akr <command> --help`.
    HelpFor {
        /// The command whose help was asked for.
        name: String,
    },
    /// Print the version and exit 0.
    Version,
    /// Scaffold a workspace.
    Init {
        /// The project name.
        project: Option<String>,
        /// Declared namespaces.
        namespaces: Vec<String>,
    },
    /// Canonically format, or check formatting.
    Fmt {
        /// Report differences without writing.
        check: bool,
        /// Paths to format; empty means the whole workspace.
        paths: Vec<PathBuf>,
    },
    /// Run stages A–D.
    Check {
        /// Also fail if the review queue is non-empty.
        review_clean: bool,
        /// Also compare the committed views against a fresh render.
        views_current: bool,
    },
    /// Run stages A–F; `--check` renders in-memory without writing.
    Build {
        /// Verify generated outputs and lock without writing.
        check: bool,
    },
    /// Render one view to stdout.
    View {
        /// The catalogue name.
        name: String,
    },
    /// Retrieve one record.
    Get {
        /// Any of the four reference forms.
        reference: String,
        /// List every revision.
        history: bool,
        /// Include inbound edges.
        relations: bool,
        /// How much of the record to return.
        detail: Detail,
    },
    /// Print a registry entry for a code or a rule.
    Explain {
        /// `AKR-R014` or `V-017`.
        subject: String,
    },
    /// Explain a head resolution and a freshness verdict.
    WhyCurrent {
        /// The reference.
        reference: String,
    },
    /// Report what depends on a record, or what a commit range invalidates.
    Impact {
        /// Record mode.
        reference: Option<String>,
        /// Git mode: `A..B`.
        git_diff: Option<String>,
    },
    /// List the stale and at-risk records.
    ReviewQueue {
        /// Only stale records.
        stale_only: bool,
        /// Only at-risk records.
        at_risk_only: bool,
        /// Restrict to these kinds.
        kinds: Vec<String>,
    },
    /// Verify or rewrite `akr.lock`.
    Lock {
        /// Verify without writing.
        check: bool,
    },
    /// Assemble a context bundle.
    Context {
        /// The anchor.
        goal: String,
        /// Path filters.
        paths: Vec<Glob>,
        /// Token ceiling.
        budget: Option<usize>,
    },
    /// Orient an ambiguous task to planning records before assembling context.
    Start {
        /// Plain-language task description.
        task: String,
        /// Path filters expected to be touched.
        paths: Vec<Glob>,
        /// Token ceiling carried into the recommended context call.
        budget: Option<usize>,
    },
    /// `akr search <query> [--kind ...] [--state ...] [--limit n]`.
    Search {
        /// What to look for. Ordinary words unless [`Self::Search::raw_fts`] is set.
        query: String,
        /// Treat the query as a raw FTS5 expression rather than as words.
        raw_fts: bool,
        /// Restrict to these kinds. Applied before ranking.
        kinds: Vec<String>,
        /// Restrict to these states. Applied before ranking.
        states: Vec<String>,
        /// Maximum results.
        limit: Option<usize>,
    },
    /// `akr import <path> [--namespace <ns>] [--tracking <key>] [--dry-run]`.
    Import {
        /// The source document.
        path: PathBuf,
        /// Namespace for proposed keys. Defaults to the document's first path segment.
        namespace: Option<String>,
        /// The tracking `work` record. Created if absent.
        tracking: Option<String>,
        /// Print what would be written and write nothing.
        dry_run: bool,
    },
    /// `akr ingest preview <path> [--source-kind <kind>] [--tables rows|support]`.
    IngestPreview {
        /// The source document.
        path: PathBuf,
        /// Source-kind provenance.
        source_kind: String,
        /// Table behavior.
        tables: TableMode,
    },
    /// `akr ingest start <path> [--source-kind <kind>] [--tables rows|support]`.
    IngestStart {
        /// The source document.
        path: PathBuf,
        /// Source-kind provenance.
        source_kind: String,
        /// Table behavior.
        tables: TableMode,
    },
    /// `akr ingest show <ingest-id> [--pending] [--limit n]`.
    IngestShow {
        /// The ingest identifier.
        ingest_id: String,
        /// Restrict to pending candidates.
        pending_only: bool,
        /// Maximum candidates.
        limit: Option<usize>,
    },
    /// `akr ingest mark <ingest-id> <candidate-id> <disposition> ...`.
    IngestMark {
        /// The ingest identifier.
        ingest_id: String,
        /// Candidate being reviewed.
        candidate_id: String,
        /// The disposition character.
        disposition: String,
        /// One or more basis references.
        basis: Vec<String>,
        /// Optional target for existing record mapping.
        target: Option<String>,
        /// Optional promotion kind.
        promote_kind: Option<String>,
        /// Target for revise or attach-source promotion.
        promote_target: Option<String>,
        /// Mark a promote plan as attach_source.
        promote_attach: bool,
        /// Relations staged against the candidate.
        relations: Vec<String>,
        /// Optional note.
        note: Option<String>,
        /// Previous manifest version.
        base_version: Option<usize>,
    },
    /// `akr ingest apply <ingest-id> [--base-version n]`.
    IngestApply {
        /// The ingest identifier.
        ingest_id: String,
        /// Previous manifest version.
        base_version: Option<usize>,
        /// Preview only if true.
        dry_run: bool,
    },
    /// `akr ingest close <ingest-id> [--base-version n]`.
    IngestClose {
        /// The ingest identifier.
        ingest_id: String,
        /// Previous manifest version.
        base_version: Option<usize>,
    },
    /// `akr source add <path> [--id <id>] [--title <t>] [--origin external|internal-reference] [--observed-at <commit>] [--scope <glob>]`.
    SourceAdd {
        /// The source file to register.
        path: PathBuf,
        /// Stable id for the source.
        id: Option<String>,
        /// Human title.
        title: Option<String>,
        /// Origin.
        origin: Option<String>,
        /// Observed commit/URL.
        observed_at: Option<String>,
        /// Scope glob.
        scope: Option<String>,
    },
    /// `akr source list [--all-versions]`.
    SourceList {
        /// Include superseded sources.
        all: bool,
    },
    /// `akr source get <id> [--whole|--lines a:b|--section "heading"]`.
    SourceGet {
        /// Source id.
        id: String,
        /// Whole file.
        whole: bool,
        /// Line range `a:b`.
        lines: Option<String>,
        /// Heading section.
        section: Option<String>,
    },
    /// `akr source get --chunk <chunk-id> [--neighbors n]`.
    SourceGetChunk {
        /// The derived chunk id, as printed by `akr source search`.
        chunk: String,
        /// How many chunks either side to include.
        neighbors: usize,
    },
    /// `akr source search <query> [--literal|--fts] [--document id] [--limit n]`.
    SourceSearch {
        /// The query.
        query: String,
        /// `--literal` verifies an exact substring; `--fts` passes an FTS5 expression
        /// through; the default escapes punctuation into ordinary terms.
        mode: String,
        /// Restrict to these documents.
        documents: Vec<String>,
        /// Include superseded documents.
        all_versions: bool,
        /// Maximum results.
        limit: Option<usize>,
    },
    /// `akr source verify`.
    SourceVerify,
    /// `akr source supersede <old-id> <new-path> [--id <new-id>]`.
    SourceSupersede {
        /// Old source id.
        old_id: String,
        /// New file path.
        new_path: PathBuf,
        /// New id.
        new_id: Option<String>,
    },
    /// `akr source status <id>`.
    SourceStatus {
        /// Source id.
        id: String,
    },
    /// `akr source dependents <id>`.
    SourceDependents {
        /// Source id.
        id: String,
    },
    /// `akr source finalize <id> [--retain cited|metadata] [--context exact|block] [--remove-file] [--dry-run]`.
    SourceFinalize {
        /// Source id.
        id: String,
        /// Retention mode.
        retain: String,
        /// Context policy.
        context: String,
        /// Remove the full source file after replacement is durable.
        remove_file: bool,
        /// Report the plan without changing files.
        dry_run: bool,
    },
    /// `akr diff --staged` — the semantic delta between HEAD and the git index.
    DiffStaged,
    /// `akr change begin ...` — open a change transaction in this worktree.
    ChangeBegin {
        /// fix, feat, perf, refactor, test, docs, build or chore.
        kind: String,
        /// The commit subject, imperative.
        summary: String,
        /// The commit scope.
        scope: Option<String>,
        /// The work record this commit mainly advances.
        primary: Option<String>,
        /// Other records the same change advances.
        related: Vec<String>,
        /// A note specific to this commit.
        note: Option<String>,
        /// Why a material change carries no work reference.
        untracked_reason: Option<String>,
    },
    /// `akr change show`.
    ChangeShow,
    /// `akr change abort`.
    ChangeAbort,
    /// `akr change prepare --staged` / `akr change verify --staged`.
    ChangePrepare {
        /// `prepare` writes the result; `verify` only reports it.
        write: bool,
    },
    /// `akr git message`.
    GitMessage,
    /// `akr git commit`.
    GitCommit,
    /// `akr git log <record>`.
    GitLog {
        /// The record whose commits to list.
        reference: String,
    },
    /// `akr git install-hooks`.
    GitInstallHooks,
    /// `akr git-hook <name>` — what an installed hook calls.
    GitHook {
        /// The hook name.
        name: String,
    },
    /// `akr propose <key> --kind <kind>`.
    Propose {
        /// The new key.
        key: String,
        /// The record kind.
        kind: String,
        /// The title, when one is given.
        title: Option<String>,
        /// A file holding the record body.
        from: Option<PathBuf>,
        /// Open `$EDITOR` on a template.
        edit: bool,
    },
    /// `akr revise <key>`.
    Revise {
        /// The key to revise.
        key: String,
        /// A file holding the replacement body.
        from: Option<PathBuf>,
        /// Open `$EDITOR` on the head.
        edit: bool,
        /// A new state for the revision.
        state: Option<String>,
        /// A new title.
        title: Option<String>,
        /// Edit a proposed head in place rather than creating revision n+1.
        in_place: bool,
        /// Dispositions, for a revise that retires a sealed planning head.
        dispositions: Vec<String>,
    },
    /// `akr supersede <key>`.
    Supersede {
        /// The key whose head is retired.
        key: String,
        /// The superseding key, when it differs.
        with: Option<String>,
        /// `child=outcome[:into]` pairs.
        dispositions: Vec<String>,
    },
    /// `akr complete <key>`.
    Complete {
        /// The planning key to complete.
        key: String,
        /// `check=@evidence` pairs.
        checks: Vec<String>,
    },
    /// `akr abandon <key> --reason <text>`.
    Abandon {
        /// The planning key to abandon.
        key: String,
        /// Why, which lands in the D-026 `note` slot.
        reason: Option<String>,
        /// `child=outcome[:into]` pairs.
        dispositions: Vec<String>,
    },
    /// `akr papercut -m <agent> "message"` (D-027).
    Papercut {
        /// What got in the way, in one or two sentences.
        message: String,
        /// Who hit it: a model or harness name. Lands in `author`.
        agent: Option<String>,
        /// The namespace for the key; needed only when the project declares several.
        namespace: Option<String>,
        /// What the friction was with, when that is not this project (D-033).
        about: Option<String>,
    },
    /// `akr papercut collate [--projects <dir>] [--about <subject>] [--namespace <ns>]`.
    PapercutCollate {
        /// A directory of sibling workspaces to scan; defaults to the siblings of the
        /// workspace root.
        projects: Option<PathBuf>,
        /// The namespace for the master record's key; needed only when the project
        /// declares several.
        namespace: Option<String>,
        /// Absorb only the sisters' papercuts whose `about` names this subject.
        about: Option<String>,
        /// Absorb every live sister papercut, whatever its subject.
        all: bool,
    },
    /// `akr evidence add <key>`.
    EvidenceAdd {
        /// The evidence key.
        key: String,
        /// `pass`, `fail` or `inconclusive`.
        result: Option<String>,
        /// `manual`, `command` or `observation`.
        method: Option<String>,
        /// The command that was run.
        command: Option<String>,
        /// A path to the artefact.
        artifact: Option<String>,
        /// A one-line summary.
        summary: Option<String>,
        /// The commit it was observed at. Defaults to HEAD.
        observed_at: Option<String>,
    },
}

impl Command {
    /// Whether the command needs Git history before dispatch.
    ///
    /// Search can use a current disposable index without Git. Task orientation includes
    /// the Git-backed session head, so only raw search keeps the ledger-only fast path.
    #[must_use]
    pub const fn needs_git_facts(&self) -> bool {
        !matches!(self, Self::Search { .. })
    }

    /// The name the JSON envelope reports.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Help => "help".to_owned(),
            Self::HelpFor { .. } => "help".to_owned(),
            Self::Version => "version".to_owned(),
            Self::Init { .. } => "init".to_owned(),
            Self::Fmt { .. } => "fmt".to_owned(),
            Self::Check { .. } => "check".to_owned(),
            Self::Build { .. } => "build".to_owned(),
            Self::View { .. } => "view".to_owned(),
            Self::Get { .. } => "get".to_owned(),
            Self::Explain { .. } => "explain".to_owned(),
            Self::WhyCurrent { .. } => "why-current".to_owned(),
            Self::Impact { .. } => "impact".to_owned(),
            Self::ReviewQueue { .. } => "review-queue".to_owned(),
            Self::Lock { .. } => "lock".to_owned(),
            Self::Context { .. } => "context".to_owned(),
            Self::Start { .. } => "start".to_owned(),
            Self::Search { .. } => "search".to_owned(),
            Self::Import { .. } => "import".to_owned(),
            Self::IngestPreview { .. }
            | Self::IngestStart { .. }
            | Self::IngestShow { .. }
            | Self::IngestMark { .. }
            | Self::IngestApply { .. }
            | Self::IngestClose { .. } => "ingest".to_owned(),
            Self::DiffStaged => "diff".to_owned(),
            Self::ChangeBegin { .. }
            | Self::ChangeShow
            | Self::ChangeAbort
            | Self::ChangePrepare { .. } => "change".to_owned(),
            Self::GitMessage | Self::GitCommit | Self::GitLog { .. } | Self::GitInstallHooks => {
                "git".to_owned()
            }
            Self::GitHook { .. } => "git-hook".to_owned(),
            Self::SourceAdd { .. }
            | Self::SourceList { .. }
            | Self::SourceGet { .. }
            | Self::SourceGetChunk { .. }
            | Self::SourceSearch { .. }
            | Self::SourceVerify
            | Self::SourceSupersede { .. }
            | Self::SourceStatus { .. }
            | Self::SourceDependents { .. }
            | Self::SourceFinalize { .. } => "source".to_owned(),
            Self::Propose { .. } => "propose".to_owned(),
            Self::Revise { .. } => "revise".to_owned(),
            Self::Supersede { .. } => "supersede".to_owned(),
            Self::Complete { .. } => "complete".to_owned(),
            Self::Abandon { .. } => "abandon".to_owned(),
            Self::Papercut { .. } => "papercut".to_owned(),
            Self::PapercutCollate { .. } => "papercut collate".to_owned(),
            Self::EvidenceAdd { .. } => "evidence add".to_owned(),
        }
    }

    /// Whether the command produces data a JSON envelope can carry.
    ///
    /// `fmt` and `init` do not: their output is a file-system effect rather than data, and
    /// asking for JSON is `AKR-C041` (`docs/07-cli.md` §5).
    #[must_use]
    pub const fn supports_json(&self) -> bool {
        !matches!(self, Self::Fmt { .. } | Self::Init { .. })
    }
}

/// A usage error. Every one exits 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError {
    /// The `AKR-C0nn` code.
    pub code: &'static str,
    /// The message.
    pub message: String,
    /// The `help:` line.
    pub help: Option<String>,
}

impl UsageError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            help: None,
        }
    }

    fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// Every command name, in `--help` order, including the ones the banner does not list.
///
/// This is what `nearest` suggests from and what the help-coverage test walks, so a
/// command missing here is a command no typo ever finds and whose `--help` nobody
/// notices is absent — which is how `git-hook` came to answer "unknown command".
pub const COMMANDS: &[&str] = &[
    "init",
    "fmt",
    "check",
    "build",
    "view",
    "get",
    "search",
    "start",
    "context",
    "impact",
    "why-current",
    "explain",
    "review-queue",
    "import",
    "ingest",
    "lock",
    "source",
    "diff",
    "change",
    "git",
    "git-hook",
    "propose",
    "revise",
    "supersede",
    "complete",
    "abandon",
    "papercut",
    "evidence",
];

/// Parses an argument list, excluding the program name.
///
/// # Errors
/// [`UsageError`] for an unknown command (`AKR-C001`), an unknown flag (`AKR-C002`), a
/// missing argument (`AKR-C003`), a bad flag value (`AKR-C004`), or mutually exclusive
/// flags (`AKR-C005`).
pub fn parse(argv: &[String]) -> Result<Invocation, UsageError> {
    let mut global = Global::default();
    let mut rest: Vec<String> = Vec::new();
    let mut strict_seen = false;
    let mut lenient_seen = false;
    let mut at_seen = false;

    let mut args = argv.iter().peekable();
    let mut command_seen = false;
    while let Some(arg) = args.next() {
        // Global flags are accepted after the command as well as before it. §1's grammar
        // writes them first, but `akr check --format json` is what everybody types, no
        // command flag shares a name with a global one, and a parser that refused it would
        // be enforcing punctuation rather than meaning.
        if command_seen && !arg.starts_with('-') {
            rest.push(arg.clone());
            continue;
        }
        match arg.as_str() {
            "--help" | "-h" => {
                // `akr propose --help` answers for `propose`, not with the top-level
                // list — every flag requirement should be one round trip away.
                if let Some(name) = rest.first() {
                    if help_for(name).is_none() {
                        let mut error =
                            UsageError::new("AKR-C001", format!("unknown command {name:?}"));
                        if let Some(nearest) = nearest(name) {
                            error = error.with_help(format!("did you mean `akr {nearest}`?"));
                        }
                        return Err(error);
                    }
                    return ok(global, Command::HelpFor { name: name.clone() });
                }
                return ok(global, Command::Help);
            }
            "--version" | "-V" => return ok(global, Command::Version),
            "--dir" => global.dir = PathBuf::from(value(&mut args, "--dir")?),
            "--strict" => {
                strict_seen = true;
                global.profile = Profile::Strict;
            }
            "--lenient" => {
                lenient_seen = true;
                global.profile = Profile::Lenient;
            }
            "--format" => {
                let text = value(&mut args, "--format")?;
                global.format = match text.as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    other => {
                        return Err(UsageError::new(
                            "AKR-C004",
                            format!("--format: {other:?} is not `text` or `json`"),
                        ));
                    }
                };
            }
            "--at" => {
                at_seen = true;
                let text = value(&mut args, "--at")?;
                global.at = Some(Commit::new(&text).map_err(|_| {
                    UsageError::new(
                        "AKR-C004",
                        format!("--at: {text:?} is not 40 lowercase hex digits"),
                    )
                    .with_help("AKR takes full commit hashes, never abbreviations (D-008)")
                })?);
            }
            "--today" => {
                let text = value(&mut args, "--today")?;
                global.today = Some(Date::parse(&text).map_err(|_| {
                    UsageError::new("AKR-C004", format!("--today: {text:?} is not YYYY-MM-DD"))
                })?);
            }
            "--no-rebuild" => global.no_rebuild = true,
            "--quiet" | "-q" => global.quiet = true,
            "--no-color" => {}
            other if other.starts_with('-') && command_seen => rest.push(other.to_owned()),
            other if other.starts_with('-') => {
                return Err(UsageError::new(
                    "AKR-C002",
                    format!("unknown flag {other:?}"),
                ));
            }
            other => {
                command_seen = true;
                rest.push(other.to_owned());
            }
        }
    }

    if strict_seen && lenient_seen {
        return Err(UsageError::new(
            "AKR-C005",
            "--strict cannot be combined with --lenient",
        ));
    }

    let Some((name, tail)) = rest.split_first() else {
        return ok(global, Command::Help);
    };
    let command = parse_command(name, tail, at_seen)?;

    if global.format == Format::Json && !command.supports_json() {
        return Err(UsageError::new(
            "AKR-C041",
            format!("{} does not support --format json", command.name()),
        )
        .with_help("its output is a file-system effect rather than data"));
    }

    ok(global, command)
}

fn ok(global: Global, command: Command) -> Result<Invocation, UsageError> {
    Ok(Invocation { global, command })
}

fn value<'a>(
    args: &mut std::iter::Peekable<impl Iterator<Item = &'a String>>,
    flag: &str,
) -> Result<String, UsageError> {
    args.next()
        .map(ToOwned::to_owned)
        .ok_or_else(|| UsageError::new("AKR-C003", format!("{flag} requires a value")))
}

fn parse_command(name: &str, tail: &[String], at_seen: bool) -> Result<Command, UsageError> {
    let flag_set = |flag: &str| tail.iter().any(|a| a == flag);
    let positional: Vec<&String> = tail.iter().filter(|a| !a.starts_with('-')).collect();

    let known_flags = |allowed: &[&str]| -> Result<(), UsageError> {
        for arg in tail.iter().filter(|a| a.starts_with("--")) {
            let base = arg.split('=').next().unwrap_or(arg);
            if !allowed.contains(&base) {
                return Err(UsageError::new(
                    "AKR-C002",
                    format!("unknown flag {base:?} for command {name:?}"),
                ));
            }
        }
        Ok(())
    };

    let need = |n: usize, what: &str| -> Result<String, UsageError> {
        positional
            .get(n)
            .map(|s| (*s).clone())
            .ok_or_else(|| UsageError::new("AKR-C003", format!("{name} requires {what}")))
    };

    Ok(match name {
        "init" => {
            known_flags(&["--project", "--namespace"])?;
            Command::Init {
                project: option_value(tail, "--project"),
                namespaces: repeated(tail, "--namespace"),
            }
        }
        "fmt" => {
            known_flags(&["--check"])?;
            Command::Fmt {
                check: flag_set("--check"),
                paths: positional.iter().map(|p| PathBuf::from(*p)).collect(),
            }
        }
        "check" => {
            known_flags(&["--review-clean", "--views-current"])?;
            Command::Check {
                review_clean: flag_set("--review-clean"),
                views_current: flag_set("--views-current"),
            }
        }
        "build" => {
            known_flags(&["--check"])?;
            Command::Build {
                check: flag_set("--check"),
            }
        }
        "view" => {
            known_flags(&[])?;
            Command::View {
                name: need(0, "a view name")?,
            }
        }
        "get" => {
            known_flags(&["--history", "--relations", "--rev", "--detail"])?;
            Command::Get {
                reference: need(0, "a reference")?,
                history: flag_set("--history"),
                relations: flag_set("--relations"),
                detail: match option_value(tail, "--detail") {
                    Some(name) => Detail::from_name(&name).ok_or_else(|| {
                        UsageError::new("AKR-C004", format!("`{name}` is not a detail level"))
                            .with_help("summary, body or canonical")
                    })?,
                    None => Detail::default(),
                },
            }
        }
        "explain" => {
            known_flags(&[])?;
            Command::Explain {
                subject: need(0, "a diagnostic code, rule identifier or record kind")?,
            }
        }
        "why-current" => {
            known_flags(&[])?;
            Command::WhyCurrent {
                reference: need(0, "a reference")?,
            }
        }
        "impact" => {
            known_flags(&["--git-diff", "--depth"])?;
            let git_diff = option_value(tail, "--git-diff");
            if git_diff.is_some() && at_seen {
                return Err(UsageError::new(
                    "AKR-C005",
                    "--at cannot be combined with --git-diff",
                ));
            }
            let reference = positional.first().map(|s| (*s).clone());
            if reference.is_none() && git_diff.is_none() {
                return Err(UsageError::new(
                    "AKR-C003",
                    "impact requires a reference or --git-diff <A>..<B>",
                ));
            }
            Command::Impact {
                reference,
                git_diff,
            }
        }
        "review-queue" => {
            known_flags(&["--stale-only", "--at-risk-only", "--kind"])?;
            let stale_only = flag_set("--stale-only");
            let at_risk_only = flag_set("--at-risk-only");
            if stale_only && at_risk_only {
                return Err(UsageError::new(
                    "AKR-C005",
                    "--stale-only cannot be combined with --at-risk-only",
                ));
            }
            Command::ReviewQueue {
                stale_only,
                at_risk_only,
                kinds: repeated(tail, "--kind"),
            }
        }
        "lock" => {
            known_flags(&["--check", "--update"])?;
            Command::Lock {
                check: flag_set("--check"),
            }
        }
        "context" => {
            known_flags(&["--goal", "--paths", "--budget"])?;
            let goal = option_value(tail, "--goal")
                .ok_or_else(|| UsageError::new("AKR-C003", "context requires --goal <key>"))?;
            let budget = match option_value(tail, "--budget") {
                Some(text) => Some(text.parse().map_err(|_| {
                    UsageError::new(
                        "AKR-C004",
                        format!("--budget: {text:?} is not a positive integer"),
                    )
                })?),
                None => None,
            };
            Command::Context {
                goal,
                paths: repeated(tail, "--paths")
                    .iter()
                    .map(|p| Glob::new(p))
                    .collect(),
                budget,
            }
        }
        "start" => {
            known_flags(&["--paths", "--budget"])?;
            let budget = match option_value(tail, "--budget") {
                Some(text) => Some(text.parse().map_err(|_| {
                    UsageError::new(
                        "AKR-C004",
                        format!("--budget: {text:?} is not a positive integer"),
                    )
                })?),
                None => None,
            };
            Command::Start {
                task: need(0, "a task description")?,
                paths: repeated(tail, "--paths")
                    .iter()
                    .map(|path| Glob::new(path))
                    .collect(),
                budget,
            }
        }
        "search" => {
            known_flags(&["--kind", "--state", "--limit", "--fts"])?;
            let limit = match option_value(tail, "--limit") {
                Some(text) => Some(text.parse().map_err(|_| {
                    UsageError::new(
                        "AKR-C004",
                        format!("--limit: {text:?} is not a positive integer"),
                    )
                })?),
                None => None,
            };
            Command::Search {
                query: need(0, "a query")?,
                kinds: repeated(tail, "--kind"),
                states: repeated(tail, "--state"),
                limit,
                raw_fts: tail.iter().any(|a| a == "--fts"),
            }
        }
        "import" => {
            known_flags(&["--namespace", "--tracking", "--dry-run"])?;
            Command::Import {
                path: PathBuf::from(need(0, "a source document")?),
                namespace: option_value(tail, "--namespace"),
                tracking: option_value(tail, "--tracking"),
                dry_run: flag_set("--dry-run"),
            }
        }
        "ingest" => parse_ingest(name, &positional, tail)?,
        "source" => parse_source(name, &positional, tail)?,
        "diff" => {
            // One mode for now, and it is named rather than defaulted: `akr diff` with no
            // flag would eventually mean something else, and a command whose meaning
            // changes under people is worse than one that asks.
            if !tail.iter().any(|a| a == "--staged") {
                return Err(UsageError::new(
                    "AKR-C003",
                    "akr diff requires --staged; the staged tree is the synchronisation \
                     boundary (docs/16-change-protocol.md §3)",
                ));
            }
            Command::DiffStaged
        }
        "change" => parse_change(name, &positional, tail)?,
        "git" => parse_git(name, &positional, tail)?,
        "git-hook" => Command::GitHook {
            name: positional
                .first()
                .map(|s| (*s).clone())
                .ok_or_else(|| UsageError::new("AKR-C003", "git-hook requires a hook name"))?,
        },
        "propose" => {
            known_flags(&["--kind", "--title", "--from", "--edit"])?;
            Command::Propose {
                key: need(0, "a key")?,
                kind: option_value(tail, "--kind")
                    .ok_or_else(|| UsageError::new("AKR-C003", "propose requires --kind <kind>"))?,
                title: option_value(tail, "--title"),
                from: option_value(tail, "--from").map(PathBuf::from),
                edit: flag_set("--edit"),
            }
        }
        "revise" => {
            known_flags(&[
                "--from",
                "--edit",
                "--state",
                "--title",
                "--in-place",
                "--disposition",
            ])?;
            Command::Revise {
                key: need(0, "a key")?,
                from: option_value(tail, "--from").map(PathBuf::from),
                edit: flag_set("--edit"),
                state: option_value(tail, "--state"),
                title: option_value(tail, "--title"),
                in_place: flag_set("--in-place"),
                dispositions: repeated(tail, "--disposition"),
            }
        }
        "supersede" => {
            known_flags(&["--with", "--disposition"])?;
            Command::Supersede {
                key: need(0, "a key")?,
                with: option_value(tail, "--with"),
                dispositions: repeated(tail, "--disposition"),
            }
        }
        "complete" => {
            known_flags(&["--check"])?;
            Command::Complete {
                key: need(0, "a key")?,
                checks: repeated(tail, "--check"),
            }
        }
        "abandon" => {
            known_flags(&["--reason", "--disposition"])?;
            Command::Abandon {
                key: need(0, "a key")?,
                reason: option_value(tail, "--reason"),
                dispositions: repeated(tail, "--disposition"),
            }
        }
        "papercut" => {
            // `collate` as the sole positional is the collation subcommand (D-030). A
            // message logged with `-m` may legitimately be the word "collate", so a flag
            // that supplies the agent keeps this on the logging path.
            let collate_subcommand = tail
                .iter()
                .find(|a| !a.starts_with('-'))
                .map(String::as_str)
                == Some("collate")
                && !tail.iter().any(|a| a == "-m" || a == "--agent");
            if collate_subcommand {
                known_flags(&["--projects", "--namespace", "--about", "--all"])?;
                let about = option_value(tail, "--about");
                let all = tail.iter().any(|a| a == "--all");
                if about.is_some() && all {
                    return Err(UsageError::new(
                        "AKR-C005",
                        "--about and --all are mutually exclusive",
                    ));
                }
                return Ok(Command::PapercutCollate {
                    projects: option_value(tail, "--projects").map(PathBuf::from),
                    namespace: option_value(tail, "--namespace"),
                    about,
                    all,
                });
            }
            // Parsed by hand: `-m` takes a value, and the generic positional filter
            // would otherwise mistake that value for the message.
            let mut message: Option<String> = None;
            let mut agent: Option<String> = None;
            let mut namespace: Option<String> = None;
            let mut about: Option<String> = None;
            let mut args = tail.iter();
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "-m" | "--agent" => {
                        agent = Some(args.next().cloned().ok_or_else(|| {
                            UsageError::new("AKR-C003", "-m requires a value: who hit it")
                        })?);
                    }
                    "--namespace" => {
                        namespace = Some(args.next().cloned().ok_or_else(|| {
                            UsageError::new("AKR-C003", "--namespace requires a value")
                        })?);
                    }
                    "--about" => {
                        about = Some(args.next().cloned().ok_or_else(|| {
                            UsageError::new(
                                "AKR-C003",
                                "--about requires a value: what the friction was with",
                            )
                        })?);
                    }
                    other if other.starts_with('-') => {
                        return Err(UsageError::new(
                            "AKR-C002",
                            format!("unknown flag {other:?} for command \"papercut\""),
                        ));
                    }
                    other => {
                        if message.is_some() {
                            return Err(UsageError::new(
                                "AKR-C004",
                                format!("papercut takes one message; {other:?} is a second"),
                            )
                            .with_help("quote the whole message"));
                        }
                        message = Some(other.to_owned());
                    }
                }
            }
            Command::Papercut {
                message: message
                    .ok_or_else(|| UsageError::new("AKR-C003", "papercut requires a message"))?,
                agent,
                namespace,
                about,
            }
        }
        "evidence" => {
            known_flags(&[
                "--result",
                "--method",
                "--command",
                "--artifact",
                "--summary",
                "--observed-at",
            ])?;
            // `evidence` has one subcommand and always will: evidence is created, never
            // edited, because a revision of an observation is a new observation (D-015).
            let sub = need(0, "the `add` subcommand")?;
            if sub != "add" {
                return Err(UsageError::new(
                    "AKR-C001",
                    format!("unknown subcommand {sub:?} for command \"evidence\""),
                )
                .with_help("the only subcommand is `akr evidence add`"));
            }
            Command::EvidenceAdd {
                key: need(1, "a key")?,
                result: option_value(tail, "--result"),
                method: option_value(tail, "--method"),
                command: option_value(tail, "--command"),
                artifact: option_value(tail, "--artifact"),
                summary: option_value(tail, "--summary"),
                observed_at: option_value(tail, "--observed-at"),
            }
        }
        other => {
            let mut error = UsageError::new("AKR-C001", format!("unknown command {other:?}"));
            if let Some(nearest) = nearest(other) {
                error = error.with_help(format!("did you mean `akr {nearest}`?"));
            }
            return Err(error);
        }
    })
}

fn parse_ingest(
    name: &str,
    positional: &[&String],
    tail: &[String],
) -> Result<Command, UsageError> {
    let known_flags = |allowed: &[&str], name: &str| -> Result<(), UsageError> {
        for arg in tail.iter().filter(|a| a.starts_with("--")) {
            let base = arg.split('=').next().unwrap_or(arg);
            if !allowed.contains(&base) {
                return Err(UsageError::new(
                    "AKR-C002",
                    format!("unknown flag {base:?} for command {name:?}"),
                ));
            }
        }
        Ok(())
    };

    let flag_set = |flag: &str| tail.iter().any(|a| a == flag);

    let need = |index: usize, what: &str| -> Result<String, UsageError> {
        positional
            .get(index)
            .map(|s| (*s).clone())
            .ok_or_else(|| UsageError::new("AKR-C003", format!("ingest {name} requires {what}")))
    };

    let source_kind = option_value(tail, "--source-kind")
        .unwrap_or_else(|| "internal".to_owned())
        .to_lowercase();
    if !matches!(source_kind.as_str(), "internal" | "external") {
        return Err(UsageError::new(
            "AKR-C004",
            format!("--source-kind {source_kind:?} is not internal|external"),
        ));
    }

    let tables = parse_table_mode(option_value(tail, "--tables"), name)?;

    let sub = need(0, "a subcommand")?;
    let sub = sub.as_str();
    Ok(match sub {
        "preview" => {
            known_flags(&["--source-kind", "--tables"], "ingest preview")?;
            Command::IngestPreview {
                path: PathBuf::from(need(1, "a source document")?),
                source_kind,
                tables,
            }
        }
        "start" => {
            known_flags(&["--source-kind", "--tables"], "ingest start")?;
            Command::IngestStart {
                path: PathBuf::from(need(1, "a source document")?),
                source_kind,
                tables,
            }
        }
        "show" => {
            known_flags(&["--pending", "--limit"], "ingest show")?;
            let limit = match option_value(tail, "--limit") {
                Some(text) => Some(text.parse().map_err(|_| {
                    UsageError::new(
                        "AKR-C004",
                        format!("--limit: {text:?} is not a positive integer"),
                    )
                })?),
                None => None,
            };
            Command::IngestShow {
                ingest_id: need(1, "an ingest id")?,
                pending_only: flag_set("--pending"),
                limit,
            }
        }
        "mark" => {
            known_flags(
                &[
                    "--basis",
                    "--target",
                    "--promote-kind",
                    "--promote-target",
                    "--promote-attach-source",
                    "--relation",
                    "--note",
                    "--base-version",
                ],
                "ingest mark",
            )?;
            Command::IngestMark {
                ingest_id: need(1, "an ingest id")?,
                candidate_id: need(2, "a candidate id")?,
                disposition: need(3, "a disposition")?,
                basis: repeated(tail, "--basis"),
                target: option_value(tail, "--target"),
                promote_kind: option_value(tail, "--promote-kind"),
                promote_target: option_value(tail, "--promote-target"),
                promote_attach: flag_set("--promote-attach-source"),
                relations: repeated(tail, "--relation"),
                note: option_value(tail, "--note"),
                base_version: option_value(tail, "--base-version")
                    .and_then(|text| text.parse::<usize>().ok()),
            }
        }
        "apply" => {
            known_flags(&["--base-version", "--dry-run"], "ingest apply")?;
            let base_version = option_value(tail, "--base-version")
                .map(|text| {
                    text.parse::<usize>().map_err(|_| {
                        UsageError::new(
                            "AKR-C004",
                            format!("--base-version: {text:?} is not a positive integer"),
                        )
                    })
                })
                .transpose()?;
            Command::IngestApply {
                ingest_id: need(1, "an ingest id")?,
                base_version,
                dry_run: flag_set("--dry-run"),
            }
        }
        "close" => {
            known_flags(&["--base-version"], "ingest close")?;
            Command::IngestClose {
                ingest_id: need(1, "an ingest id")?,
                base_version: option_value(tail, "--base-version")
                    .map(|text| {
                        text.parse().map_err(|_| {
                            UsageError::new(
                                "AKR-C004",
                                format!("--base-version: {text:?} is not a positive integer"),
                            )
                        })
                    })
                    .transpose()?,
            }
        }
        _ => {
            return Err(UsageError::new(
                "AKR-C001",
                format!("unknown subcommand {sub:?} for command \"ingest\""),
            )
            .with_help("supported subcommands: preview, start, show, mark, apply, close"));
        }
    })
}

fn parse_source(
    name: &str,
    positional: &[&String],
    tail: &[String],
) -> Result<Command, UsageError> {
    let need = |index: usize, what: &str| -> Result<String, UsageError> {
        positional
            .get(index)
            .map(|s| (*s).clone())
            .ok_or_else(|| UsageError::new("AKR-C003", format!("source {name} requires {what}")))
    };
    let sub = need(0, "a subcommand (add|list|get|search|verify|supersede)")?;
    let sub = sub.as_str();
    Ok(match sub {
        "search" => {
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                let base = arg.split('=').next().unwrap_or(arg);
                if ![
                    "--literal",
                    "--fts",
                    "--document",
                    "--all-versions",
                    "--limit",
                ]
                .contains(&base)
                {
                    return Err(UsageError::new(
                        "AKR-C002",
                        format!("unknown flag {base:?} for command \"source search\""),
                    ));
                }
            }
            let literal = tail.iter().any(|a| a == "--literal");
            let fts = tail.iter().any(|a| a == "--fts");
            if literal && fts {
                return Err(UsageError::new(
                    "AKR-C005",
                    "--literal and --fts are mutually exclusive",
                ));
            }
            Command::SourceSearch {
                query: need(1, "a query")?,
                mode: if literal {
                    "literal".to_owned()
                } else if fts {
                    "fts".to_owned()
                } else {
                    "text".to_owned()
                },
                documents: repeated(tail, "--document"),
                all_versions: tail.iter().any(|a| a == "--all-versions"),
                limit: option_value(tail, "--limit").and_then(|v| v.parse().ok()),
            }
        }
        "add" => {
            let allowed = vec!["--id", "--title", "--origin", "--observed-at", "--scope"];
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                let base = arg.split('=').next().unwrap_or(arg);
                if !allowed.contains(&base) {
                    return Err(UsageError::new(
                        "AKR-C002",
                        format!("unknown flag {base:?} for command \"source add\""),
                    ));
                }
            }
            Command::SourceAdd {
                path: PathBuf::from(need(1, "a source document")?),
                id: option_value(tail, "--id"),
                title: option_value(tail, "--title"),
                origin: option_value(tail, "--origin"),
                observed_at: option_value(tail, "--observed-at"),
                scope: option_value(tail, "--scope"),
            }
        }
        "list" => {
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                let base = arg.split('=').next().unwrap_or(arg);
                if base != "--all-versions" {
                    return Err(UsageError::new(
                        "AKR-C002",
                        format!("unknown flag {base:?} for command \"source list\""),
                    ));
                }
            }
            Command::SourceList {
                all: tail.iter().any(|a| a == "--all-versions"),
            }
        }
        "get" => {
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                let base = arg.split('=').next().unwrap_or(arg);
                if !["--whole", "--lines", "--section", "--chunk", "--neighbors"].contains(&base) {
                    return Err(UsageError::new(
                        "AKR-C002",
                        format!("unknown flag {base:?} for command \"source get\""),
                    ));
                }
            }
            // `--chunk` names a unit of the derived index rather than a document, so it
            // takes no source id and pairs only with `--neighbors`.
            if let Some(chunk) = option_value(tail, "--chunk") {
                if ["--whole", "--lines", "--section"]
                    .iter()
                    .any(|flag| tail.iter().any(|a| a.split('=').next() == Some(flag)))
                {
                    return Err(UsageError::new(
                        "AKR-C005",
                        "--chunk cannot be combined with --whole, --lines or --section",
                    ));
                }
                return Ok(Command::SourceGetChunk {
                    chunk,
                    neighbors: option_value(tail, "--neighbors")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0),
                });
            }
            let has_lines = option_value(tail, "--lines").is_some();
            let has_section = option_value(tail, "--section").is_some();
            let has_whole = tail.iter().any(|a| a == "--whole");
            if (has_lines as u8 + has_section as u8 + has_whole as u8) > 1 {
                return Err(UsageError::new(
                    "AKR-C005",
                    "--whole, --lines and --section are mutually exclusive",
                ));
            }
            Command::SourceGet {
                id: need(1, "a source id")?,
                whole: has_whole,
                lines: option_value(tail, "--lines"),
                section: option_value(tail, "--section"),
            }
        }
        "verify" => {
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                return Err(UsageError::new(
                    "AKR-C002",
                    format!("unknown flag {arg:?} for command \"source verify\""),
                ));
            }
            Command::SourceVerify
        }
        "supersede" => {
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                let base = arg.split('=').next().unwrap_or(arg);
                if !["--id"].contains(&base) {
                    return Err(UsageError::new(
                        "AKR-C002",
                        format!("unknown flag {base:?} for command \"source supersede\""),
                    ));
                }
            }
            Command::SourceSupersede {
                old_id: need(1, "an existing source id")?,
                new_path: PathBuf::from(need(2, "a new source document")?),
                new_id: option_value(tail, "--id"),
            }
        }
        "status" => {
            if tail.iter().any(|arg| arg.starts_with("--")) {
                return Err(UsageError::new(
                    "AKR-C002",
                    "source status does not accept flags",
                ));
            }
            Command::SourceStatus {
                id: need(1, "a source id")?,
            }
        }
        "dependents" => {
            if tail.iter().any(|arg| arg.starts_with("--")) {
                return Err(UsageError::new(
                    "AKR-C002",
                    "source dependents does not accept flags",
                ));
            }
            Command::SourceDependents {
                id: need(1, "a source id")?,
            }
        }
        "finalize" => {
            for arg in tail.iter().filter(|a| a.starts_with("--")) {
                let base = arg.split('=').next().unwrap_or(arg);
                if !["--retain", "--context", "--remove-file", "--dry-run"].contains(&base) {
                    return Err(UsageError::new(
                        "AKR-C002",
                        format!("unknown flag {base:?} for command \"source finalize\""),
                    ));
                }
            }
            let retain = option_value(tail, "--retain").unwrap_or_else(|| "cited".into());
            if !["cited", "metadata"].contains(&retain.as_str()) {
                return Err(UsageError::new(
                    "AKR-C004",
                    "source finalize --retain must be cited or metadata",
                ));
            }
            let context = option_value(tail, "--context").unwrap_or_else(|| "block".into());
            if !["exact", "block"].contains(&context.as_str()) {
                return Err(UsageError::new(
                    "AKR-C004",
                    "source finalize --context must be exact or block",
                ));
            }
            Command::SourceFinalize {
                id: need(1, "a source id")?,
                retain,
                context,
                remove_file: tail.iter().any(|arg| arg == "--remove-file"),
                dry_run: tail.iter().any(|arg| arg == "--dry-run"),
            }
        }
        _ => {
            return Err(UsageError::new(
                "AKR-C001",
                format!("unknown subcommand {sub:?} for command \"source\""),
            )
            .with_help("supported subcommands: add, list, get, verify, supersede, status, dependents, finalize"));
        }
    })
}

fn parse_table_mode(value: Option<String>, name: &str) -> Result<TableMode, UsageError> {
    match value.as_deref() {
        None | Some("rows") => Ok(TableMode::Rows),
        Some("support") => Ok(TableMode::Support),
        Some(other) => Err(UsageError::new(
            "AKR-C004",
            format!("{name} --tables: {other:?} is not rows|support"),
        )),
    }
}

/// The value of `--flag value` or `--flag=value`.
/// `akr change <subcommand>`.
fn parse_change(
    name: &str,
    positional: &[&String],
    tail: &[String],
) -> Result<Command, UsageError> {
    let sub = positional.first().map(|s| s.as_str()).ok_or_else(|| {
        UsageError::new(
            "AKR-C003",
            format!("{name} requires a subcommand (begin|show|prepare|verify|abort)"),
        )
    })?;
    Ok(match sub {
        "begin" => {
            let summary = option_value(tail, "--summary")
                .ok_or_else(|| UsageError::new("AKR-C003", "change begin requires --summary"))?;
            Command::ChangeBegin {
                kind: option_value(tail, "--kind").unwrap_or_else(|| "chore".to_owned()),
                summary,
                scope: option_value(tail, "--scope"),
                primary: option_value(tail, "--primary"),
                related: repeated(tail, "--related"),
                note: option_value(tail, "--note"),
                untracked_reason: option_value(tail, "--untracked-reason"),
            }
        }
        "show" => Command::ChangeShow,
        "abort" => Command::ChangeAbort,
        "prepare" | "verify" => {
            if !tail.iter().any(|a| a == "--staged") {
                return Err(UsageError::new(
                    "AKR-C003",
                    format!("change {sub} requires --staged"),
                ));
            }
            Command::ChangePrepare {
                write: sub == "prepare",
            }
        }
        other => {
            return Err(UsageError::new(
                "AKR-C001",
                format!("unknown subcommand {other:?} for command \"change\""),
            )
            .with_help("supported subcommands: begin, show, prepare, verify, abort"));
        }
    })
}

/// `akr git <subcommand>`.
fn parse_git(name: &str, positional: &[&String], _tail: &[String]) -> Result<Command, UsageError> {
    let sub = positional.first().map(|s| s.as_str()).ok_or_else(|| {
        UsageError::new(
            "AKR-C003",
            format!("{name} requires a subcommand (message|commit|log|install-hooks)"),
        )
    })?;
    Ok(match sub {
        "message" => Command::GitMessage,
        "commit" => Command::GitCommit,
        "install-hooks" => Command::GitInstallHooks,
        "log" => Command::GitLog {
            reference: positional
                .get(1)
                .map(|s| (*s).clone())
                .ok_or_else(|| UsageError::new("AKR-C003", "git log requires a record"))?,
        },
        other => {
            return Err(UsageError::new(
                "AKR-C001",
                format!("unknown subcommand {other:?} for command \"git\""),
            )
            .with_help("supported subcommands: message, commit, log, install-hooks"));
        }
    })
}

fn option_value(tail: &[String], flag: &str) -> Option<String> {
    let mut args = tail.iter();
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next().cloned();
        }
        if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
            return Some(value.to_owned());
        }
    }
    None
}

/// Every value of a repeatable flag, in order.
fn repeated(tail: &[String], flag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut args = tail.iter();
    while let Some(arg) = args.next() {
        if arg == flag {
            if let Some(value) = args.next() {
                out.push(value.clone());
            }
        } else if let Some(value) = arg.strip_prefix(&format!("{flag}=")) {
            out.push(value.to_owned());
        }
    }
    out
}

/// The nearest known command by edit distance, for the `help:` line of `AKR-C001`.
fn nearest(name: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .map(|candidate| (distance(name, candidate), *candidate))
        .filter(|(d, _)| *d <= 3)
        .min()
        .map(|(_, candidate)| candidate)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, x) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let cost = usize::from(x != y);
            current[j + 1] = (previous[j] + cost)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        previous.clone_from(&current);
    }
    previous[b.len()]
}

/// One command's `--help` text, or `None` for a name that is not a command.
///
/// Wording is drawn from `docs/07-cli.md` §6, compressed to what an author mid-command
/// needs: the usage line, what the command does, every flag, and the mistakes the
/// diagnostics most often see.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn help_for(name: &str) -> Option<String> {
    let text = match name {
        "init" => {
            "akr init [--project <name>] [--namespace <name> ...]\n\
             \n\
             Scaffolds a workspace: .akr/project.akr, .akr/records/, .akr/archive/, an\n\
             AGENTS.md protocol section, and .gitignore entries. Never overwrites an\n\
             existing .akr/ (AKR-C013).\n\
             \n\
             FLAGS\n\
             \x20   --project <name>      the project name, in key-segment form\n\
             \x20   --namespace <name>    a declared namespace; repeatable\n"
        }
        "fmt" => {
            "akr fmt [--check] [<path> ...]\n\
             \n\
             Parses each .akr file and re-emits it canonically (D-012). With no paths,\n\
             the whole workspace including akr.lock.\n\
             \n\
             FLAGS\n\
             \x20   --check    write nothing; report differences as AKR-F diagnostics, exit 1\n"
        }
        "check" => {
            "akr check [--review-clean] [--views-current]\n\
             \n\
             Runs stages A-D and reports every diagnostic. The command CI runs.\n\
             Staleness never changes the exit code (D-024).\n\
             \n\
             FLAGS\n\
             \x20   --review-clean     also fail when the review queue is non-empty (AKR-G041)\n\
             \x20   --views-current    also render stage F in memory and diff the committed\n\
             \x20                      views (AKR-E011..E014) — the D-025 gate\n"
        }
        "build" => {
            "akr build [--check]\n\
             \n\
             Runs stages A-F: everything `akr check` does, then the index cache, the\n\
             generated views, and akr.lock. Only files whose bytes change are rewritten.\n\
             In --check mode, nothing is written and the command fails on any diagnostic,\n\
             including generated-view mismatches.\n"
        }
        "view" => {
            "akr view <name>\n\
             \n\
             Renders one view to stdout without writing it. Names, case-insensitive,\n\
             with or without .md: roadmap, current-state, active-work, review-required,\n\
             open-questions, decision-history.\n"
        }
        "get" => {
            "akr get <ref> [--history] [--relations]\n\
             \n\
             Retrieves one record. <ref> is any of the four D-009 forms: @key (head),\n\
             @key/2 (pinned), @key#anchor, @key/2#anchor. The @ is optional here.\n\
             \n\
             FLAGS\n\
             \x20   --history      list every revision with state and supersession edges\n\
             \x20   --relations    include inbound edges, invisible in the source text\n"
        }
        "search" => {
            "akr search <query> [--kind <kind> ...] [--state <state> ...] [--limit <n>]\n\
             \n\
             Full-text search over live revisions, BM25-ranked. Filters apply before\n\
             ranking. Search ranks; it never authorises.\n\
             \n\
             FLAGS\n\
             \x20   --kind <kind>      restrict to a kind; repeatable\n\
             \x20   --state <state>    restrict to a state; repeatable\n\
             \x20   --limit <n>        maximum results\n"
        }
        "context" => {
            "akr context --goal <key> [--paths <glob> ...] [--budget <tokens>]\n\
             \n\
             Assembles the deterministic context bundle an agent reads before working.\n\
             \n\
             FLAGS\n\
             \x20   --goal <key>        required; a live milestone, work or track record\n\
             \x20   --paths <glob>      path globs the work will touch; repeatable\n\
             \x20   --budget <tokens>   approximate token ceiling for the bundle\n"
        }
        "start" => {
            "akr start <task> [--paths <glob> ...] [--budget <tokens>]\n\
             \n\
             Prints a validated project handoff (latest Git/AKR work, outstanding branches,\n\
             review attention and any dirty ledger overlay), then searches live planning\n\
             records and returns a ready-made context request when one result is clear.\n\
             \n\
             FLAGS\n\
             \x20   --paths <glob>      path globs the work will touch; repeatable\n\
             \x20   --budget <tokens>   approximate token ceiling for the combined response\n"
        }
        "impact" => {
            "akr impact <ref> | --git-diff <A>..<B> [--depth <n>]\n\
             \n\
             Record mode reports what rests on a record (reverse closure along\n\
             supported_by, depends_on, derived_from). Git mode reports what a commit\n\
             range makes stale. The tool to call before proposing a supersession.\n\
             \n\
             FLAGS\n\
             \x20   --git-diff <A>..<B>   full 40-hex commits on both ends\n\
             \x20   --depth <n>           maximum propagation depth\n"
        }
        "why-current" => {
            "akr why-current <ref>\n\
             \n\
             Explains a head resolution and a freshness verdict: the supersession chain,\n\
             the lock entry, and the propagation path when the record is at risk.\n"
        }
        "explain" => {
            "akr explain <code> | <rule> | <kind>\n\
             \n\
             Prints the registry entry for a diagnostic code (akr explain AKR-R014), the\n\
             catalogue entry for a validation rule (akr explain V-017), or the schema of\n\
             a record kind (akr explain milestone): class, lifecycle, required and\n\
             optional slots. Needs no workspace.\n"
        }
        "review-queue" => {
            "akr review-queue [--stale-only] [--at-risk-only] [--kind <kind> ...]\n\
             \n\
             Lists stale records with their cause, then at-risk records by propagation\n\
             depth. Exit 0 regardless of queue length; a non-empty queue is healthy.\n\
             \n\
             FLAGS\n\
             \x20   --stale-only      only stale records\n\
             \x20   --at-risk-only    only at-risk records\n\
             \x20   --kind <kind>     restrict to a kind; repeatable\n"
        }
        "lock" => {
            "akr lock [--check] [--update]\n\
             \n\
             --check reports drift between akr.lock and the current build without\n\
             writing (AKR-R051, AKR-R052). --update rewrites the lock, which is also\n\
             what `akr build` does at its end.\n"
        }
        "import" => {
            "akr import <path> [--namespace <ns>] [--tracking <key>] [--dry-run]\n\
             \n\
             Reads a legacy Markdown document, proposes one record per durable claim\n\
             with a verbatim source excerpt, and maintains a tracking work record whose\n\
             checks enumerate the claims (D-022). Combine with --lenient for legacy\n\
             material that raises warnings.\n\
             \n\
             FLAGS\n\
             \x20   --namespace <ns>    namespace for proposed keys; defaults to the\n\
             \x20                       document's first path segment\n\
             \x20   --tracking <key>    the tracking work record; created if absent\n\
             \x20   --dry-run           print what would be written, write nothing\n"
        }
        "source" => {
            "akr source add <path> [--id <id>] [--title <title>] [--origin external|internal-reference] [--observed-at <commit>] [--scope <glob>]\n\
             akr source list [--all-versions]\n\
             akr source get <id> [--whole|--lines <a:b>|--section <heading>]\n\
             akr source get --chunk <chunk-id> [--neighbors <n>]\n\
             akr source search <query> [--literal|--fts] [--document <id>] [--all-versions] [--limit <n>]\n\
            akr source verify\n\
            akr source supersede <old-id> <new-path> [--id <new-id>]\n\
             akr source status <id>\n\
             akr source dependents <id>\n\
             akr source finalize <id> [--retain cited|metadata] [--context exact|block] [--remove-file] [--dry-run]\n\
             \n\
             Immutable source library in sources/. `add` copies the file to\n\
             sources/external/<id>--<hash>.md, records its sha256 in\n\
             sources/catalog.json, and creates no ledger records. `verify`\n\
             recomputes hashes and reports AKR-S021 on mismatch; `check` runs\n\
             the same verification. `supersede` adds a new version and preserves\n\
             the old file; the catalog's `supersedes` field links them.\n\
             \n\
             `search` ranks chunks of the registered documents and labels every\n\
             result non-authoritative: a hit says where a passage is, never that\n\
             the project adopted it. Punctuation is escaped into ordinary terms by\n\
             default, so DecodeRequest::default() is a query rather than a parse\n\
             error; --literal verifies an exact substring against the stored bytes\n\
             and --fts passes a raw FTS5 expression through.\n\
             \n\
             FLAGS\n\
             \x20   add --id, --title, --origin, --observed-at, --scope\n\
             \x20   get --whole, --lines, --section, --chunk, --neighbors\n\
             \x20   search --literal, --fts, --document, --all-versions, --limit\n\
             \x20   list --all-versions\n"
        }
        "diff" => {
            "akr diff --staged\n\
             \n\
             The semantic delta between the HEAD ledger and the ledger staged in the\n\
             git index: records added, revisions added, state transitions, evidence\n\
             added, and the implementation files staged beside them.\n\
             \n\
             It parses both trees rather than reading `git diff` text. A reformat, a\n\
             reordering, or a record moved between files is not a semantic change,\n\
             and a textual diff cannot say so (docs/16-change-protocol.md, 4).\n"
        }
        "change" => {
            "akr change begin --summary <text> [--kind <kind>] [--scope <scope>]\n\
             \x20                 [--primary <key>] [--related <key>]... [--note <text>]\n\
             \x20                 [--untracked-reason <text>]\n\
             akr change show\n\
             akr change prepare --staged\n\
             akr change verify --staged\n\
             akr change abort\n\
             \n\
             A change transaction associates the commit you are about to make with the\n\
             AKR work it advances. It lives in this worktree's git directory, is never\n\
             committed, and is invisible to search and context: only the generated\n\
             commit message and its trailers are durable (D-032).\n\
             \n\
             `prepare` refuses a material code change that names neither a work record\n\
             nor --untracked-reason, refuses when several work records moved and none\n\
             was named primary, and records the staged tree so that a tree which moves\n\
             afterwards invalidates the preparation. `verify` runs the same checks and\n\
             writes nothing.\n\
             \n\
             KINDS\n\
             \x20   fix, feat, perf, refactor, test, docs, build, chore\n"
        }
        "git" => {
            "akr git message\n\
             akr git commit\n\
             akr git log <record>\n\
             akr git install-hooks\n\
             \n\
             `message` prints the message generated from the prepared transaction and\n\
             the staged semantic delta; `commit` generates it and hands the index to\n\
             git. The durable AKR-to-git link is the commit trailers -- AKR-Change,\n\
             AKR-Work, AKR-Evidence, AKR-Graph, AKR-Tree -- which survive rebases and\n\
             cherry-picks and which `git log` finds.\n\
             \n\
             `install-hooks` writes two-line wrappers around `akr git-hook`, so the\n\
             checks stay in the binary rather than becoming a second implementation.\n\
             Hooks live in the git directory, which is never cloned: run it once per\n\
             worktree, or the hooks are simply absent rather than failing.\n"
        }
        "git-hook" => {
            "akr git-hook <pre-commit|commit-msg>\n\
             \n\
             What an installed hook calls; not normally run by hand. `pre-commit` runs\n\
             the same verification as `akr change verify --staged` and refuses rather\n\
             than repairing, so a commit with no prepared change transaction is\n\
             rejected (AKR-C031). Hooks are a guardrail and are bypassable; CI remains\n\
             the final authority.\n\
             \n\
             Install them with `akr git install-hooks`.\n"
        }
        "ingest" => {
            "akr ingest preview <path> [--source-kind internal|external] [--tables rows|support]\n\
             \n\
             Candidate-oriented import of a Markdown source for manual review. Use `preview`
             first, `start` to persist, `mark` to add dispositions, and `apply` to
             execute staged operations.\n\
             \n\
             SUBCOMMANDS\n\
             \x20   preview --path <path>   extract candidates (no manifest written)\n\
             \x20   start <path>           create a new manifest and persist the source\n\
             \x20   show <ingest-id>       review candidates and summary\n\
             \x20   mark <ingest-id> <candidate-id> <disposition> ...\n\
             \x20   apply <ingest-id>      apply ready promotion actions\n\
             \x20   close <ingest-id>      finalize and stop accepting edits\n"
        }
        "propose" => {
            "akr propose <key> --kind <kind> [--title <text>] [--from <file>]\n\
             \n\
             Creates revision 1 of a new key in its class's initial state. An existing\n\
             key is an error: use `akr revise`.\n\
             \n\
             <key> is dot-delimited: namespace.topic.slug — the first segment must be a\n\
             namespace declared in .akr/project.akr.\n\
             \n\
             A body source is mandatory in this build: every kind requires its prose slot\n\
             (definition, statement, intent, rule ...), so a propose without --from is\n\
             refused before anything reaches the disk. Run\n\
             `akr explain <kind>` for the kind's required slots.\n\
             \n\
             FLAGS\n\
             \x20   --kind <kind>     required; one of the twelve kinds\n\
             \x20   --title <text>    the one-line label\n\
             \x20   --from <file>     a file holding the record body\n"
        }
        "revise" => {
            "akr revise <key> [--from <file>] [--state <state>] [--title <text>]\n\
             \x20          [--in-place] [--disposition <child>=<outcome>[:<into>] ...]\n\
             \n\
             Creates revision n+1 by copying the head and applying edits; a sealed old\n\
             head is retired in the same atomic write. A proposed head is edited in\n\
             place instead. Revising a sealed planning head demands a disposition for\n\
             every unfinished part_of child, exactly as supersede does (D-017).\n\
             \n\
             FLAGS\n\
             \x20   --from <file>     a file holding the replacement body\n\
             \x20   --state <state>   move along the class's lifecycle\n\
             \x20   --title <text>    replace the title\n\
             \x20   --in-place        force the in-place path; AKR-C032 on a sealed head\n\
             \x20   --disposition     <child>=<outcome>[:<into>]; repeatable\n"
        }
        "supersede" => {
            "akr supersede <old-key> --with <new-key>\n\
             \x20             [--disposition <child>=<outcome>[:<into>] ...]\n\
             \n\
             Creates or updates the superseding record, moves the old head to\n\
             `superseded`, and requires a disposition for every unfinished part_of\n\
             child of a planning record (D-017). The error lists the children it\n\
             needs; answer each one.\n\
             \n\
             Outcomes: completed, carried_forward (with :<into>), intentionally_dropped,\n\
             obsolete.\n"
        }
        "complete" => {
            "akr complete <key> [--check <id>=<evidence-ref> ...]\n\
             \n\
             Moves a milestone or work record to `completed`. Every acceptance check\n\
             must be satisfied by evidence with result pass observed after the record\n\
             last changed (D-016). Evidence references are D-009 forms:\n\
             --check no-placeholder-assets=@sys.evidence.asset-audit/1\n\
             \n\
             Completing a milestone requires its plan of record to be retired first\n\
             (V-019): complete or abandon the plan, then complete the milestone.\n"
        }
        "abandon" => {
            "akr abandon <key> --reason <text>\n\
             \x20           [--disposition <child>=<outcome>[:<into>] ...]\n\
             \n\
             Moves a planning record to `abandoned`. Requires --reason, which lands in\n\
             the durable note slot and is rendered by the views, and a disposition for\n\
             every unfinished child, like supersede.\n"
        }
        "papercut" => {
            "akr papercut -m <agent> \"message\" [--about <subject>] [--namespace <ns>]\n\
             akr papercut collate [--projects <dir>] [--about <subject>|--all] [--namespace <ns>]\n\
             \n\
             Logs a small friction hit while working — a tool call that missed and had\n\
             to be retried, a confusing setup step, a flaky command, a stale cache, a\n\
             misleading error, a non-obvious gotcha. One or two sentences: what you\n\
             were doing, what got in the way (a guess at the cause/fix is a bonus).\n\
             Log it proactively, in the moment; together these show where the project\n\
             needs sanding down (D-027).\n\
             \n\
             The message is the whole ceremony: the key, the commit, the author and\n\
             the date are filled in automatically, and the aggregate is rendered to\n\
             docs/generated/PAPERCUTS.md by `akr build`.\n\
             \n\
             --about says what the friction was *with*, when that is not this project:\n\
             `--about akr` on a papercut about AKR's own behaviour, hit while working\n\
             somewhere else. Absent means this project's own code or setup (D-033).\n\
             \n\
             `collate` reads the live papercut heads of every workspace under a scan\n\
             directory (default: the siblings of the workspace root) and gathers those\n\
             not already absorbed into one master papercut record, whose collated slot\n\
             is the dedup set for the next run (D-030). Sister projects are read, never\n\
             written. It absorbs the ones aimed at this project by default; --about\n\
             narrows to one subject and --all takes everything. Whatever is left behind\n\
             is counted in the output, so nothing goes missing silently.\n\
             \n\
             FLAGS\n\
             \x20   -m, --agent <name>    required to log; who hit it (a model or harness name)\n\
             \x20   --about <subject>     what the friction was with, if not this project\n\
             \x20   --projects <dir>      collate: a directory of sibling workspaces to scan\n\
             \x20   --all                 collate: absorb every subject, not just this project's\n\
             \x20   --namespace <ns>      only needed when the project declares several\n"
        }
        "evidence" | "evidence add" => {
            "akr evidence add <key> --result pass|fail|inconclusive\n\
             \x20                   --method manual|command|observation\n\
             \x20                   [--command <text>] [--artifact <path>]\n\
             \x20                   [--summary <text>] [--observed-at <commit>]\n\
             \n\
             Creates an evidence record. --observed-at defaults to HEAD and must be a\n\
             full 40-hex commit in the repository.\n\
             \n\
             There is deliberately no flag for what the evidence verifies (D-016): the\n\
             link is authored on the check (verified_by [ @key/n ]) or supplied to\n\
             `akr complete --check <id>=@key/n`.\n"
        }
        _ => return None,
    };
    Some(format!(
        "{text}\nGLOBAL FLAGS are listed by `akr --help`; they are accepted before or \
         after the command.\n"
    ))
}

/// The `--help` text.
#[must_use]
pub fn help() -> String {
    let mut out = String::from(
        "akr — a versioned project-knowledge ledger\n\
         \n\
         USAGE\n    \
             akr [GLOBAL FLAGS] <command> [COMMAND FLAGS] [ARGUMENTS]\n\
         \n\
         GLOBAL FLAGS\n\
         \x20   --dir <path>          where to look for the workspace (default: .)\n\
         \x20   --strict | --lenient  warnings are errors (default: --strict)\n\
         \x20   --format text|json    output form (default: text)\n\
         \x20   --at <commit>         resolve against this commit instead of HEAD\n\
         \x20   --today <date>        the date review_after is compared against\n\
         \x20   --no-rebuild          fail rather than rebuild the index\n\
         \x20   --quiet, -q           suppress progress lines\n\
         \x20   --version, -V         print the version\n\
         \x20   --help, -h            print this\n\
         \n\
         COMMANDS\n",
    );
    for (name, summary) in [
        ("init", "scaffold a workspace"),
        ("fmt", "canonically format, or --check"),
        ("check", "run stages A-D; --review-clean, --views-current"),
        ("build", "run stages A-F: index, views, lock; --check"),
        ("view", "render one view to stdout"),
        ("get", "retrieve one record"),
        ("search", "full-text search (P7)"),
        ("start", "orient an ambiguous task to planning records"),
        ("context", "assemble a context bundle"),
        (
            "impact",
            "what rests on a record, or what a range invalidates",
        ),
        (
            "why-current",
            "explain a head resolution and a freshness verdict",
        ),
        ("explain", "print a code, rule, or record-kind schema"),
        ("review-queue", "list the stale and at-risk records"),
        ("import", "draft proposed records from a legacy document"),
        (
            "ingest",
            "extract candidates from markdown and review dispositions",
        ),
        ("lock", "verify or rewrite akr.lock"),
        (
            "source",
            "immutable source library in sources/; add, search, get",
        ),
        ("diff", "the semantic delta of the staged ledger; --staged"),
        (
            "change",
            "the change transaction; begin, prepare, show, abort",
        ),
        ("git", "generate and make the commit; message, commit, log"),
        ("propose", "create a record; --kind, --title, --from"),
        (
            "revise",
            "create the next revision; --from, --state, --title",
        ),
        (
            "supersede",
            "replace a record; --disposition <child>=<outcome>[:<into>]",
        ),
        (
            "complete",
            "finish a milestone; --check <id>=<evidence-ref>",
        ),
        ("abandon", "abandon a planning record; --reason is required"),
        (
            "papercut",
            "log a small friction, in the moment; -m <agent>",
        ),
        (
            "evidence add",
            "record what was observed; --result, --method",
        ),
    ] {
        out.push_str(&format!("    {name:<16}{summary}\n"));
    }
    out.push_str("\nEXIT CODES\n    0 ok    1 diagnostics    2 usage    3 environment\n");
    out
}
