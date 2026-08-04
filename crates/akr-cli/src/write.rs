//! The write commands of `docs/07-cli.md` §6, over `akr_core::ops`.
//!
//! Every one of them delegates the whole of §4's pipeline — parse, apply, validate,
//! format, write — to the library, and does three things of its own: turn command-line
//! strings into the library's request types, render an [`Applied`] or a [`Refused`], and
//! choose an exit status.
//!
//! # Refusals are rendered from structure, never from prose
//!
//! [`Refused`] carries `unfinished_children` and `unsatisfied_checks` as vectors. The
//! §6 samples — the list of children a supersession must dispose of, the `--disposition`
//! lines to copy — are built from those vectors here. Nothing parses a message string,
//! so a refusal renders identically however the library words it, and the MCP surface
//! can render the same data differently without either drifting.
//!
//! # Nothing here writes
//!
//! Not one line of this module touches the filesystem. That is not a stylistic
//! preference: `docs/07` §4 guarantees that a refused write leaves the working tree
//! byte-identical, and the guarantee is only checkable if there is exactly one writer.
//! `tests/writes.rs` hashes every source file around each refusing path.

use crate::args::Command;
use crate::commands::Output;
use crate::session::{EnvError, Exit, Session};
use akr_core::json::Value;
use akr_core::model::{
    Commit, EvidenceResult, Kind, LogicalKey, Outcome as DispositionOutcome, Record, Reference,
    State,
};
use akr_core::ops::{
    Applied, Change, ChangeKind, DispositionRequest, Edits, Refused, ReviseMode, WriteContext,
    WriteResult,
};
use std::path::Path;

/// Runs a write command.
///
/// # Errors
/// [`EnvError`] when an argument cannot be turned into a request at all — a malformed
/// key, an unknown kind, an unreadable `--from` file. A refusal from the library is not
/// an error here: it is an [`Output`] with exit 1, because the tool did its job.
pub fn run(session: &Session, command: &Command) -> Result<Output, EnvError> {
    let context = context_of(session);
    match command {
        Command::Propose {
            key,
            kind,
            title,
            from,
            edit,
        } => {
            let key = parse_key(key)?;
            let kind = parse_kind(kind)?;
            let template = body_template(session, from.as_deref(), *edit, &key, kind)?;
            render(
                session,
                akr_core::ops::propose(
                    &context,
                    &key,
                    kind,
                    title.as_deref().unwrap_or_default(),
                    template,
                ),
            )
        }
        Command::Revise {
            key,
            from,
            edit,
            state,
            title,
            in_place,
            dispositions,
        } => {
            let key = parse_key(key)?;
            let mode = if *in_place {
                ReviseMode::InPlace
            } else {
                ReviseMode::Auto
            };
            let mut edits = Edits {
                title: title.clone(),
                state: state.as_deref().map(parse_state).transpose()?,
                replace_with: None,
            };
            if let Some(record) = body_template(session, from.as_deref(), *edit, &key, Kind::Work)?
            {
                edits.replace_with = Some(Box::new(record));
            }
            let _ = dispositions;
            render(session, akr_core::ops::revise(&context, &key, mode, &edits))
        }
        Command::Supersede {
            key,
            with,
            dispositions,
        } => {
            let key = parse_key(key)?;
            // `--with` names the superseding key. When it is the same key — the common
            // case, and the one §6's sample shows — it is redundant, and when it differs
            // the operation is a proposal plus a supersession, which P6c does not fuse.
            if let Some(with) = with
                && parse_key(with)? != key
            {
                return Err(EnvError::new(
                    "AKR-C004",
                    format!(
                        "--with {with}: superseding a key with a different key is not yet supported"
                    ),
                )
                .help("propose the new key first, then `akr supersede` the old one"));
            }
            let dispositions = parse_dispositions(dispositions)?;
            render(
                session,
                akr_core::ops::supersede(&context, &key, &dispositions),
            )
        }
        Command::Complete { key, checks } => {
            let key = parse_key(key)?;
            let checks = parse_checks(checks)?;
            render(session, akr_core::ops::complete(&context, &key, &checks))
        }
        Command::Abandon {
            key,
            reason,
            dispositions,
        } => {
            let key = parse_key(key)?;
            let dispositions = parse_dispositions(dispositions)?;
            render(
                session,
                akr_core::ops::abandon(
                    &context,
                    &key,
                    reason.as_deref().unwrap_or_default(),
                    &dispositions,
                ),
            )
        }
        Command::EvidenceAdd {
            key,
            result,
            method,
            command,
            artifact,
            summary,
            observed_at,
        } => evidence_add(
            session,
            &context,
            key,
            result.as_deref(),
            method.as_deref(),
            command.as_deref(),
            artifact.as_deref(),
            summary.as_deref(),
            observed_at.as_deref(),
        ),
        _ => unreachable!("run is only called for write commands"),
    }
}

/// `akr evidence add`, over the P5 evidence module.
///
/// Evidence is created through [`akr_core::evidence::AddEvidence`] rather than through
/// `propose`, because an evidence record has required slots a blank template cannot
/// invent — a result, a method, and the commit it was observed at. The record it builds
/// is then handed to `propose`, so the write goes through the one pipeline of §4 like
/// everything else.
#[allow(clippy::too_many_arguments)]
fn evidence_add(
    session: &Session,
    context: &WriteContext,
    key: &str,
    result: Option<&str>,
    method: Option<&str>,
    command: Option<&str>,
    artifact: Option<&str>,
    summary: Option<&str>,
    observed_at: Option<&str>,
) -> Result<Output, EnvError> {
    let key = parse_key(key)?;
    let result = match result {
        Some("pass") => EvidenceResult::Pass,
        Some("fail") => EvidenceResult::Fail,
        Some("inconclusive") => EvidenceResult::Inconclusive,
        Some(other) => {
            return Err(EnvError::new(
                "AKR-C004",
                format!("--result: {other:?} is not `pass`, `fail` or `inconclusive`"),
            ));
        }
        None => {
            return Err(EnvError::new(
                "AKR-C003",
                "evidence add requires --result pass|fail|inconclusive",
            ));
        }
    };
    let method = match method {
        Some("manual") => akr_core::model::CheckMethod::Manual,
        Some("command") => akr_core::model::CheckMethod::Command,
        Some("observation") => akr_core::model::CheckMethod::Observation,
        Some(other) => {
            return Err(EnvError::new(
                "AKR-C004",
                format!("--method: {other:?} is not `manual`, `command` or `observation`"),
            ));
        }
        None => {
            return Err(EnvError::new(
                "AKR-C003",
                "evidence add requires --method manual|command|observation",
            ));
        }
    };

    // `--observed-at` defaults to HEAD (§6). An evidence record with no commit cannot be
    // tested for descendancy, so D-016's acceptance rule would never be satisfiable.
    let commit = match observed_at {
        Some(text) => Commit::new(text).map_err(|_| {
            EnvError::new(
                "AKR-C004",
                format!("--observed-at: {text:?} is not 40 lowercase hex digits"),
            )
            .help("AKR takes full commit hashes, never abbreviations (D-008)")
        })?,
        None => session.commit.clone().ok_or_else(|| {
            EnvError::new(
                "AKR-G001",
                "no commit to record: not inside a git repository",
            )
            .help("pass --observed-at <commit> explicitly")
        })?,
    };
    if let Some(repository) = &session.repository
        && !repository.contains(&commit)
    {
        return Err(EnvError::new(
            "AKR-G011",
            format!("observed_at {commit} is not present in this repository"),
        ));
    }

    let title = summary
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| key.to_string());
    let mut request =
        akr_core::evidence::AddEvidence::new(key.clone(), &title, result, method, commit);
    if let Some(command) = command {
        request = request.command(command);
    }
    if let Some(artifact) = artifact {
        request = request.artifact(artifact);
    }
    if let Some(summary) = summary {
        request = request.summary(summary);
    }
    if let Some(author) = &context.author {
        request.author = Some(author.clone());
    }

    let record = request.to_record();
    render(
        session,
        akr_core::ops::propose(context, &key, Kind::Evidence, &title, Some(record)),
    )
}

// -------------------------------------------------------------------------------------
// argument conversion
// -------------------------------------------------------------------------------------

pub(crate) fn context_of(session: &Session) -> WriteContext {
    let mut context = WriteContext::new(&session.akr_dir);
    context.strict = session.global.profile == crate::args::Profile::Strict;
    // The author is whatever git thinks, because AKR is not an identity system (D-005)
    // and the one honest source of a name in a repository is the repository's own config.
    if let Some(author) = git_author(session) {
        context = context.with_author(author);
    }
    context
}

/// The author to record, from `git config user.name`.
///
/// AKR is not an identity system (D-005): `author` is free text, and the one honest source
/// of a name inside a repository is the repository's own configuration. Absent is fine.
fn git_author(session: &Session) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "user.name"])
        .current_dir(&session.root)
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!name.is_empty()).then_some(name)
}

pub(crate) fn parse_key(text: &str) -> Result<LogicalKey, EnvError> {
    let trimmed = text.strip_prefix('@').unwrap_or(text);
    LogicalKey::parse(trimmed)
        .map_err(|e| EnvError::new("AKR-C004", format!("{text:?} is not a key: {e}")))
}

fn parse_kind(text: &str) -> Result<Kind, EnvError> {
    Kind::from_name(text).ok_or_else(|| {
        EnvError::new("AKR-C004", format!("--kind: {text:?} is not a record kind")).help(
            "one of term, requirement, constraint, policy, decision, observation, evidence, \
             assessment, milestone, work, track, question",
        )
    })
}

fn parse_state(text: &str) -> Result<State, EnvError> {
    State::from_name(text)
        .ok_or_else(|| EnvError::new("AKR-C004", format!("--state: {text:?} is not a state")))
}

/// `child=outcome[:into][,note]` — the form §6 shows.
fn parse_dispositions(raw: &[String]) -> Result<Vec<DispositionRequest>, EnvError> {
    raw.iter().map(|text| parse_disposition(text)).collect()
}

fn parse_disposition(text: &str) -> Result<DispositionRequest, EnvError> {
    let (child, rest) = text.split_once('=').ok_or_else(|| {
        EnvError::new(
            "AKR-C004",
            format!("--disposition {text:?}: expected <child>=<outcome>[:<into>]"),
        )
    })?;
    let (outcome, into) = match rest.split_once(':') {
        Some((outcome, into)) => (outcome, Some(into)),
        None => (rest, None),
    };
    let outcome = DispositionOutcome::ALL
        .iter()
        .copied()
        .find(|candidate| candidate.name() == outcome)
        .ok_or_else(|| {
            EnvError::new(
                "AKR-C004",
                format!("--disposition: {outcome:?} is not a disposition outcome"),
            )
            .help(format!(
                "one of {}",
                DispositionOutcome::ALL
                    .iter()
                    .map(|o| o.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
    Ok(DispositionRequest {
        child: parse_key(child)?,
        outcome,
        into: into.map(parse_key).transpose()?,
        note: None,
    })
}

/// `check=@evidence` — the form `akr complete --check` takes.
fn parse_checks(raw: &[String]) -> Result<Vec<(String, Reference)>, EnvError> {
    raw.iter()
        .map(|text| {
            let (id, reference) = text.split_once('=').ok_or_else(|| {
                EnvError::new(
                    "AKR-C004",
                    format!("--check {text:?}: expected <check-id>=<evidence-reference>"),
                )
            })?;
            let reference = Reference::parse(reference).map_err(|e| {
                EnvError::new(
                    "AKR-C004",
                    format!("--check: {reference:?} is not a reference: {e}"),
                )
            })?;
            Ok((id.to_owned(), reference))
        })
        .collect()
}

/// The record body a `--from` file holds, if one was given.
///
/// `--edit` is refused rather than half-implemented: opening an editor is a terminal
/// interaction the MCP surface cannot have, and a flag that works from a shell and fails
/// from an agent is worse than one that is honestly absent.
fn body_template(
    _session: &Session,
    from: Option<&Path>,
    edit: bool,
    key: &LogicalKey,
    _kind: Kind,
) -> Result<Option<Record>, EnvError> {
    if edit {
        return Err(
            EnvError::new("AKR-C004", "--edit is not available in this build")
                .help("write the record to a file and pass --from <file>"),
        );
    }
    let Some(path) = from else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path)
        .map_err(|e| EnvError::new("AKR-C042", format!("cannot read {}: {e}", path.display())))?;

    let mut sources = akr_core::diagnostics::SourceMap::new();
    let display_path = path.to_string_lossy().into_owned();
    let file = sources.add(&display_path, &text);
    let parsed = akr_core::syntax::parse(&text, file);
    let refuse = |message: String| {
        EnvError::new(
            "AKR-C031",
            format!("{} does not parse as a record: {message}", path.display()),
        )
    };
    let Some(tree) = parsed.file else {
        let first = parsed
            .diagnostics
            .first()
            .map_or_else(|| "no records".to_owned(), |d| d.message.clone());
        return Err(refuse(first));
    };
    let (ledger, lowered) = akr_core::syntax::lower::lower_all(&[(display_path, tree)]);
    if let Some(fatal) = parsed
        .diagnostics
        .iter()
        .chain(lowered.iter())
        .find(|d| d.severity == akr_core::diagnostics::Severity::Error)
    {
        return Err(refuse(fatal.message.clone()));
    }
    let record = ledger
        .records()
        .iter()
        .find(|r| r.id.key == *key)
        .or_else(|| ledger.records().first())
        .cloned()
        .ok_or_else(|| EnvError::new("AKR-C031", format!("{} holds no record", path.display())))?;
    Ok(Some(record))
}

// -------------------------------------------------------------------------------------
// rendering
// -------------------------------------------------------------------------------------

pub(crate) fn render(session: &Session, result: WriteResult) -> Result<Output, EnvError> {
    match result {
        Ok(applied) => Ok(render_applied(session, &applied)),
        Err(refused) => Ok(render_refused(session, &refused)),
    }
}

fn render_applied(session: &Session, applied: &Applied) -> Output {
    let mut text = String::new();
    for change in &applied.changes {
        text.push_str(&change_line(change));
    }
    for file in &applied.files {
        text.push_str(&format!("wrote {}\n", display(session, file)));
    }
    if applied.lock_stale {
        // Every write invalidates the lock's seals and its source-graph hash, and no
        // write operation may invent a build (D-014). Saying so is the difference between
        // an expected `AKR-R052` on the next check and a confusing one.
        text.push_str("akr.lock is now stale; run `akr build` to refresh it\n");
    }
    for diagnostic in &applied.diagnostics {
        text.push_str(&akr_core::diagnostics::render(diagnostic, &session.sources));
    }

    Output {
        text,
        result: Value::object(vec![
            ("operation", Value::string(applied.operation.name())),
            (
                "changes",
                Value::array(applied.changes.iter().map(change_json).collect()),
            ),
            (
                "files",
                Value::array(
                    applied
                        .files
                        .iter()
                        .map(|f| Value::string(display(session, f)))
                        .collect(),
                ),
            ),
            ("lock_stale", Value::bool(applied.lock_stale)),
        ]),
        diagnostics: applied.diagnostics.clone(),
        exit: Exit::Ok,
        commit: session.commit.as_ref().map(|c| c.as_str().to_owned()),
        source_graph: session.source_graph(),
    }
}

fn change_line(change: &Change) -> String {
    match &change.kind {
        ChangeKind::Created => format!("created {}\n", change.id),
        ChangeKind::Edited => format!("edited {}\n", change.id),
        ChangeKind::StateChanged { from, to } => {
            format!("{} {from} -> {to}\n", change.id)
        }
    }
}

fn change_json(change: &Change) -> Value {
    let (kind, from, to) = match &change.kind {
        ChangeKind::Created => ("created", None, None),
        ChangeKind::Edited => ("edited", None, None),
        ChangeKind::StateChanged { from, to } => {
            ("state_changed", Some(from.name()), Some(to.name()))
        }
    };
    let mut fields = vec![
        ("key", Value::string(change.id.key.to_string())),
        ("rev", Value::integer(i64::from(change.id.revision))),
        ("change", Value::string(kind)),
        ("file", Value::string(change.file.to_string_lossy())),
    ];
    if let (Some(from), Some(to)) = (from, to) {
        fields.push(("from", Value::string(from)));
        fields.push(("to", Value::string(to)));
    }
    Value::object(fields)
}

/// A refusal, rendered from [`Refused`]'s structured fields.
fn render_refused(session: &Session, refused: &Refused) -> Output {
    // The refusal line first, then the structure, then the help — §6's sample order. The
    // help line names the fix, and a fix reads better after the thing it fixes than before
    // it, so the diagnostic is rendered without its help and the help is added at the end.
    let mut headline = refused.diagnostic();
    headline.help = None;
    let mut text = akr_core::diagnostics::render(&headline, &session.sources);

    if !refused.unfinished_children.is_empty() {
        let width = refused
            .unfinished_children
            .iter()
            .map(|child| child.key.to_string().len() + 1)
            .max()
            .map_or(0, |longest| longest + 3);
        text.push_str(&format!(
            "  {} unfinished {}:\n",
            refused.unfinished_children.len(),
            if refused.unfinished_children.len() == 1 {
                "child"
            } else {
                "children"
            }
        ));
        for child in &refused.unfinished_children {
            text.push_str(&format!(
                "    {:<width$}{}\n",
                format!("@{}", child.key),
                child.state.name()
            ));
        }
    }
    if !refused.unsatisfied_checks.is_empty() {
        let width = refused
            .unsatisfied_checks
            .iter()
            .map(|check| check.id.len())
            .max()
            .map_or(0, |longest| longest + 3);
        text.push_str(&format!(
            "  {} unsatisfied {}:\n",
            refused.unsatisfied_checks.len(),
            if refused.unsatisfied_checks.len() == 1 {
                "check"
            } else {
                "checks"
            }
        ));
        for check in &refused.unsatisfied_checks {
            text.push_str(&format!("    {:<width$}{}\n", check.id, check.reason));
        }
    }
    // The validation diagnostics are the information only when there is no structure: a
    // refusal that already listed its unfinished children would otherwise say the same
    // thing twice, in two voices.
    if refused.unfinished_children.is_empty() && refused.unsatisfied_checks.is_empty() {
        for diagnostic in &refused.diagnostics {
            text.push_str(&akr_core::diagnostics::render(diagnostic, &session.sources));
        }
    }
    if let Some(help) = &refused.help {
        text.push_str(&format!("help: {help}\n"));
    }
    if !refused.unfinished_children.is_empty() {
        for child in &refused.unfinished_children {
            text.push_str(&format!(
                "  --disposition {}=intentionally_dropped\n",
                child.key
            ));
        }
    }
    text.push_str("nothing written\n");

    let mut diagnostics = vec![refused.diagnostic()];
    diagnostics.extend(refused.diagnostics.clone());
    Output {
        text,
        result: Value::object(vec![
            ("operation", Value::string(refused.operation.name())),
            ("refused", Value::bool(true)),
            ("code", Value::string(refused.code.as_str())),
            (
                "unfinished_children",
                Value::array(
                    refused
                        .unfinished_children
                        .iter()
                        .map(|child| {
                            Value::object(vec![
                                ("key", Value::string(child.key.to_string())),
                                ("state", Value::string(child.state.name())),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "unsatisfied_checks",
                Value::array(
                    refused
                        .unsatisfied_checks
                        .iter()
                        .map(|check| {
                            Value::object(vec![
                                ("id", Value::string(check.id.clone())),
                                ("reason", Value::string(check.reason.clone())),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]),
        diagnostics,
        exit: Exit::Diagnostics,
        commit: session.commit.as_ref().map(|c| c.as_str().to_owned()),
        source_graph: session.source_graph(),
    }
}

fn display(session: &Session, path: &Path) -> String {
    let full = session.akr_dir.join(path);
    full.strip_prefix(&session.root)
        .unwrap_or(&full)
        .to_string_lossy()
        .replace('\\', "/")
}
