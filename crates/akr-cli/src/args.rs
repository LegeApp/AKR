//! Argument parsing: global flags, subcommands, and the usage errors of
//! `docs/07-cli.md` §2.
//!
//! Hand-rolled, for the reason the JSON writer is: the surface is small, the exit-status
//! contract is specific (`AKR-C001`–`AKR-C005` all exit 2), and a parser that produces
//! its own error strings would have to be talked out of them.

use akr_core::model::{Commit, Date, Glob};
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
    /// Run stages A–F.
    Build,
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
    /// `akr search <query> [--kind ...] [--state ...] [--limit n]`.
    Search {
        /// The query, in the full-text engine's syntax.
        query: String,
        /// Restrict to these kinds. Applied before ranking.
        kinds: Vec<String>,
        /// Restrict to these states. Applied before ranking.
        states: Vec<String>,
        /// Maximum results.
        limit: Option<usize>,
    },
    /// Legacy import. Arrives with P8.
    Import {
        /// The source document.
        path: PathBuf,
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
    /// The name the JSON envelope reports.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Help => "help".to_owned(),
            Self::Version => "version".to_owned(),
            Self::Init { .. } => "init".to_owned(),
            Self::Fmt { .. } => "fmt".to_owned(),
            Self::Check { .. } => "check".to_owned(),
            Self::Build => "build".to_owned(),
            Self::View { .. } => "view".to_owned(),
            Self::Get { .. } => "get".to_owned(),
            Self::Explain { .. } => "explain".to_owned(),
            Self::WhyCurrent { .. } => "why-current".to_owned(),
            Self::Impact { .. } => "impact".to_owned(),
            Self::ReviewQueue { .. } => "review-queue".to_owned(),
            Self::Lock { .. } => "lock".to_owned(),
            Self::Context { .. } => "context".to_owned(),
            Self::Search { .. } => "search".to_owned(),
            Self::Import { .. } => "import".to_owned(),
            Self::Propose { .. } => "propose".to_owned(),
            Self::Revise { .. } => "revise".to_owned(),
            Self::Supersede { .. } => "supersede".to_owned(),
            Self::Complete { .. } => "complete".to_owned(),
            Self::Abandon { .. } => "abandon".to_owned(),
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

const COMMANDS: &[&str] = &[
    "init",
    "fmt",
    "check",
    "build",
    "view",
    "get",
    "search",
    "context",
    "impact",
    "why-current",
    "explain",
    "propose",
    "revise",
    "supersede",
    "complete",
    "abandon",
    "evidence",
    "review-queue",
    "import",
    "lock",
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
            "--help" | "-h" => return ok(global, Command::Help),
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
            known_flags(&[])?;
            Command::Build
        }
        "view" => {
            known_flags(&[])?;
            Command::View {
                name: need(0, "a view name")?,
            }
        }
        "get" => {
            known_flags(&["--history", "--relations", "--rev"])?;
            Command::Get {
                reference: need(0, "a reference")?,
                history: flag_set("--history"),
                relations: flag_set("--relations"),
            }
        }
        "explain" => {
            known_flags(&[])?;
            Command::Explain {
                subject: need(0, "a diagnostic code or rule identifier")?,
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
        "search" => {
            known_flags(&["--kind", "--state", "--limit"])?;
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
            }
        }
        "import" => {
            known_flags(&["--namespace", "--tracking", "--dry-run"])?;
            Command::Import {
                path: PathBuf::from(need(0, "a source document")?),
            }
        }
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

/// The value of `--flag value` or `--flag=value`.
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
        ("build", "run stages A-F: index, views, lock"),
        ("view", "render one view to stdout"),
        ("get", "retrieve one record"),
        ("search", "full-text search (P7)"),
        ("context", "assemble a context bundle"),
        (
            "impact",
            "what rests on a record, or what a range invalidates",
        ),
        (
            "why-current",
            "explain a head resolution and a freshness verdict",
        ),
        ("explain", "print a registry entry for a code or a rule"),
        ("review-queue", "list the stale and at-risk records"),
        ("import", "import a legacy document (P8)"),
        ("lock", "verify or rewrite akr.lock"),
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
            "evidence add",
            "record what was observed; --result, --method",
        ),
    ] {
        out.push_str(&format!("    {name:<16}{summary}\n"));
    }
    out.push_str("\nEXIT CODES\n    0 ok    1 diagnostics    2 usage    3 environment\n");
    out
}
