//! The nine tools, each over the function the command line calls.
//!
//! Every tool here does three things and no more: translate its JSON arguments into the
//! request type `akr-cli` already takes, call the same function `akr <command>` calls, and
//! shape the result as `docs/08-mcp.md` §3 or §4 declares. The ledger logic is entirely in
//! `akr-core`; the command logic is entirely in `akr-cli`.
//!
//! # Read and write are separated here, structurally
//!
//! §7: "Read tools never touch `.akr/records/`." [`is_write`] is the one place that
//! distinction is recorded, and [`call`] routes on it — a read tool cannot reach the write
//! module because the match arms do not lead there.

use akr_cli::args::{Command, Format, Global, Profile};
use akr_cli::commands::{self, Output};
use akr_cli::session::{EnvError, Exit, Session};
use akr_core::json::Value;
use akr_core::model::{Glob, LogicalKey};
use std::path::{Path, PathBuf};

use crate::errors::{Class, ToolError, class_of, first_error_code};
use crate::record;

/// Whether a tool writes to `.akr/records/` (§2).
#[must_use]
pub fn is_write(name: &str) -> bool {
    crate::schema::TOOLS
        .iter()
        .find(|tool| tool.name == name)
        .is_some_and(|tool| tool.writes)
}

/// Runs a tool.
///
/// # Errors
/// [`ToolError`] carrying §5's class, summary and diagnostic array.
pub fn call(root: &Path, name: &str, arguments: &Value) -> Result<Value, ToolError> {
    match name {
        "knowledge.search" => search(root, arguments),
        "knowledge.get" => get(root, arguments),
        "knowledge.context" => context(root, arguments),
        "knowledge.impact" => impact(root, arguments),
        "knowledge.validate" => validate(root, arguments),
        "knowledge.propose" => propose(root, arguments),
        "knowledge.revise" => revise(root, arguments),
        "knowledge.supersede" => supersede(root, arguments),
        "knowledge.complete" => complete(root, arguments),
        "knowledge.evidence_add" => evidence_add(root, arguments),
        "knowledge.papercut" => papercut(root, arguments),
        other => Err(ToolError::new(
            "AKR-X041",
            format!("unknown tool {other:?}; the catalogue is closed for 0.1"),
        )),
    }
}

// -------------------------------------------------------------------------------------
// read tools
// -------------------------------------------------------------------------------------

fn search(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let query = required_str(arguments, "query")?;
    run_read(
        root,
        &Command::Search {
            query: query.to_owned(),
            kinds: string_list(arguments, "kinds"),
            states: string_list(arguments, "states"),
            limit: arguments
                .get("limit")
                .and_then(Value::as_integer)
                .and_then(|n| usize::try_from(n).ok()),
        },
    )
}

/// A JSON array of strings, or nothing.
fn string_list(arguments: &Value, field: &str) -> Vec<String> {
    arguments
        .get(field)
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

fn get(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let reference = required_str(arguments, "ref")?;
    run_read(
        root,
        &Command::Get {
            reference: reference.to_owned(),
            history: flag(arguments, "history"),
            // §3's sample shows relations, so they are on by default here where the CLI
            // makes them opt-in: an agent pays for a second call, a human pays for a
            // wider terminal.
            relations: arguments
                .get("relations")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        },
    )
}

fn context(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let goal = required_str(arguments, "goal")?;
    let paths: Vec<Glob> = arguments
        .get("paths")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(Glob::new)
        .collect();
    let budget = arguments
        .get("budget_tokens")
        .and_then(Value::as_integer)
        .and_then(|n| usize::try_from(n).ok());
    // The same `Command::Context` the binary builds from `--goal`, `--paths` and
    // `--budget`, so §1's invariant holds by construction rather than by agreement.
    run_read(
        root,
        &Command::Context {
            goal: goal.to_owned(),
            paths,
            budget,
        },
    )
}

fn impact(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let reference = arguments.get("ref").and_then(Value::as_str);
    let git_diff = arguments.get("git_diff").and_then(Value::as_str);
    match (reference, git_diff) {
        (None, None) => Err(ToolError::new(
            "AKR-C003",
            "knowledge.impact requires exactly one of `ref` or `git_diff`",
        )),
        (Some(_), Some(_)) => Err(ToolError::new(
            "AKR-C005",
            "`ref` and `git_diff` are mutually exclusive",
        )),
        _ => run_read(
            root,
            &Command::Impact {
                reference: reference.map(ToOwned::to_owned),
                git_diff: git_diff.map(ToOwned::to_owned),
            },
        ),
    }
}

/// `knowledge.validate` — §3's `{ok, diagnostics, counts}`.
///
/// The one read tool whose payload is not the command's `result` verbatim: `akr check`
/// reports the pipeline's stage counts, and an agent wants a verdict. Both are computed
/// from the same [`Output`], so they cannot disagree.
fn validate(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let mut session = open(root, false)?;
    let command = Command::Check {
        review_clean: flag(arguments, "review_clean"),
        views_current: false,
    };
    let output = commands::run(&mut session, &command).map_err(environment)?;
    let diagnostics = commands::diagnostics_json(&output.diagnostics, &session.sources);
    let counts = Value::object(vec![
        ("records", field(&output.result, "records")),
        ("revisions", field(&output.result, "revisions")),
        ("stale", field(&output.result, "stale")),
        ("at_risk", field(&output.result, "at_risk")),
    ]);
    // Never an error: `knowledge.validate` reports a verdict, and a ledger with
    // diagnostics is a fact about the ledger rather than a failure of the call.
    Ok(Value::object(vec![
        ("ok", Value::bool(output.exit == Exit::Ok)),
        ("diagnostics", Value::array(diagnostics)),
        ("counts", counts),
    ]))
}

// -------------------------------------------------------------------------------------
// write tools
// -------------------------------------------------------------------------------------

fn propose(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let session = open(root, true)?;
    let key = required_str(arguments, "key")?;
    let kind_name = required_str(arguments, "kind")?;
    let title = required_str(arguments, "title")?;
    let kind = akr_core::model::Kind::from_name(kind_name)
        .ok_or_else(|| ToolError::new("AKR-C004", format!("`{kind_name}` is not a record kind")))?;
    let parsed = key_of(key)?;

    let source = record::to_source(
        &session.ledger.project.name,
        &parsed,
        1,
        kind,
        title,
        arguments.get("state").and_then(Value::as_str),
        arguments,
    )?;
    let template = record::parse(&source, &parsed)?;
    let context = write_context(&session);
    write_result(
        &session,
        &parsed,
        akr_core::ops::propose(&context, &parsed, kind, title, Some(template)),
    )
}

fn revise(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let session = open(root, true)?;
    let key = required_str(arguments, "key")?;
    let parsed = key_of(key)?;
    let head = session
        .ledger
        .head(&parsed)
        .ok()
        .ok_or_else(|| ToolError::new("AKR-L001", format!("{key} does not resolve")))?;

    // §3: `base_rev` must equal the current head revision. This is the only concurrency
    // control the surface has, and it is enough because the store is a git working tree a
    // human is also watching.
    let base_rev = arguments
        .get("base_rev")
        .and_then(Value::as_integer)
        .ok_or_else(|| ToolError::new("AKR-C003", "knowledge.revise requires `base_rev`"))?;
    if base_rev != i64::from(head.id.revision) {
        return Err(ToolError::new(
            "AKR-C033",
            format!(
                "base_rev {base_rev} is not the head of {key}, which is at revision {}",
                head.id.revision
            ),
        ));
    }

    let title = arguments
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or(&head.title);
    let source = record::to_source(
        &session.ledger.project.name,
        &parsed,
        head.id.revision + 1,
        head.kind,
        title,
        arguments.get("state").and_then(Value::as_str),
        &merged(arguments, head),
    )?;
    let replacement = record::parse(&source, &parsed)?;

    let edits = akr_core::ops::Edits {
        title: Some(title.to_owned()),
        state: None,
        replace_with: Some(Box::new(replacement)),
    };
    let context = write_context(&session);
    write_result(
        &session,
        &parsed,
        akr_core::ops::revise(&context, &parsed, akr_core::ops::ReviseMode::Auto, &edits),
    )
}

fn supersede(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let session = open(root, true)?;
    let key = required_str(arguments, "old_key")?;
    let parsed = key_of(key)?;
    if let Some(new_key) = arguments.get("new_key").and_then(Value::as_str)
        && key_of(new_key)? != parsed
    {
        return Err(ToolError::new(
            "AKR-C004",
            "superseding a key with a different key is not supported in 0.1",
        ));
    }
    let dispositions = dispositions(arguments)?;
    let context = write_context(&session);
    write_result(
        &session,
        &parsed,
        akr_core::ops::supersede(&context, &parsed, &dispositions),
    )
}

fn complete(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let session = open(root, true)?;
    let key = required_str(arguments, "key")?;
    let parsed = key_of(key)?;
    let mut checks = Vec::new();
    if let Some(Value::Object(pairs)) = arguments.get("checks") {
        for (id, reference) in pairs {
            let text = reference.as_str().ok_or_else(|| {
                ToolError::new("AKR-C004", format!("check `{id}` needs a reference"))
            })?;
            let reference = akr_core::model::Reference::parse(text).map_err(|e| {
                ToolError::new(
                    "AKR-C004",
                    format!("check `{id}`: {text:?} is not a reference: {e}"),
                )
            })?;
            checks.push((id.clone(), reference));
        }
    }
    let context = write_context(&session);
    write_result(
        &session,
        &parsed,
        akr_core::ops::complete(&context, &parsed, &checks),
    )
}

/// `knowledge.evidence_add`, over the same [`akr_core::evidence::AddEvidence`] request
/// `akr evidence add` builds.
///
/// The schema deliberately has no field for what the evidence verifies (D-016): the link
/// is authored on the check (`verified_by`) or supplied to `knowledge.complete`.
fn evidence_add(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let session = open(root, true)?;
    let key = required_str(arguments, "key")?;
    let parsed = key_of(key)?;

    let result = match required_str(arguments, "result")? {
        "pass" => akr_core::model::EvidenceResult::Pass,
        "fail" => akr_core::model::EvidenceResult::Fail,
        "inconclusive" => akr_core::model::EvidenceResult::Inconclusive,
        other => {
            return Err(ToolError::new(
                "AKR-C004",
                format!("`result`: {other:?} is not `pass`, `fail` or `inconclusive`"),
            ));
        }
    };
    let method = match required_str(arguments, "method")? {
        "manual" => akr_core::model::CheckMethod::Manual,
        "command" => akr_core::model::CheckMethod::Command,
        "observation" => akr_core::model::CheckMethod::Observation,
        other => {
            return Err(ToolError::new(
                "AKR-C004",
                format!("`method`: {other:?} is not `manual`, `command` or `observation`"),
            ));
        }
    };

    // `observed_at` defaults to HEAD, as on the command line. An evidence record with no
    // commit could never satisfy D-016's descendancy rule.
    let commit = match arguments.get("observed_at").and_then(Value::as_str) {
        Some(text) => {
            let bare = text.strip_prefix("git:").unwrap_or(text);
            akr_core::model::Commit::new(bare).map_err(|_| {
                ToolError::new(
                    "AKR-C004",
                    format!(
                        "`observed_at`: {text:?} is not 40 lowercase hex digits; AKR takes \
                         full commit hashes, never abbreviations (D-008)"
                    ),
                )
            })?
        }
        None => session.commit.clone().ok_or_else(|| {
            ToolError::new(
                "AKR-G001",
                "no commit to record: not inside a git repository; pass `observed_at`",
            )
        })?,
    };
    if let Some(repository) = &session.repository
        && !repository.contains(&commit)
    {
        return Err(ToolError::new(
            "AKR-G011",
            format!("observed_at {commit} is not present in this repository"),
        ));
    }

    let summary = arguments.get("summary").and_then(Value::as_str);
    let title = arguments
        .get("title")
        .and_then(Value::as_str)
        .or(summary)
        .map_or_else(|| parsed.to_string(), ToOwned::to_owned);
    let mut request =
        akr_core::evidence::AddEvidence::new(parsed.clone(), &title, result, method, commit);
    if let Some(command) = arguments.get("command").and_then(Value::as_str) {
        request = request.command(command);
    }
    if let Some(artifact) = arguments.get("artifact").and_then(Value::as_str) {
        request = request.artifact(artifact);
    }
    if let Some(summary) = summary {
        request = request.summary(summary);
    }

    let record = request.to_record();
    let context = write_context(&session);
    write_result(
        &session,
        &parsed,
        akr_core::ops::propose(
            &context,
            &parsed,
            akr_core::model::Kind::Evidence,
            &title,
            Some(record),
        ),
    )
}

/// `knowledge.papercut`, over the same [`akr_core::papercut`] request `akr papercut`
/// builds (D-027). The message is the whole ceremony.
fn papercut(root: &Path, arguments: &Value) -> Result<Value, ToolError> {
    let session = open(root, true)?;
    let message = required_str(arguments, "message")?;
    let agent = required_str(arguments, "agent")?;
    let namespace = arguments.get("namespace").and_then(Value::as_str);

    let commit = session.commit.clone().ok_or_else(|| {
        ToolError::new(
            "AKR-G001",
            "no commit to record: not inside a git repository",
        )
    })?;
    let key = akr_core::papercut::allocate_key(&session.ledger, namespace, message)
        .map_err(|e| ToolError::new("AKR-C004", e.to_string()))?;
    let request = akr_core::papercut::LogPapercut {
        message: message.to_owned(),
        agent: agent.to_owned(),
        observed_at: commit,
        created_at: Some(session.today),
    };
    let record = request.to_record(key.clone());
    let title = record.title.clone();
    let context = write_context(&session);
    write_result(
        &session,
        &key,
        akr_core::ops::propose(
            &context,
            &key,
            akr_core::model::Kind::Papercut,
            &title,
            Some(record),
        ),
    )
}

// -------------------------------------------------------------------------------------
// shared plumbing
// -------------------------------------------------------------------------------------

/// Opens the workspace, exactly as the command line opens it.
fn open(root: &Path, _writing: bool) -> Result<Session, ToolError> {
    let global = Global {
        dir: PathBuf::from(root),
        profile: Profile::Strict,
        format: Format::Json,
        ..Global::default()
    };
    Session::open(&global).map_err(environment)
}

/// Runs a read command and returns its `result` object verbatim.
///
/// Verbatim is the point. §1's invariant is that a tool and its command produce the same
/// answer, and the cheapest way to keep that true is for the tool to add nothing.
fn run_read(root: &Path, command: &Command) -> Result<Value, ToolError> {
    let mut session = open(root, false)?;
    let output = commands::run(&mut session, command).map_err(environment)?;
    finish(&session, output)
}

fn finish(session: &Session, output: Output) -> Result<Value, ToolError> {
    if output.exit == Exit::Ok {
        return Ok(output.result);
    }
    let diagnostics = commands::diagnostics_json(&output.diagnostics, &session.sources);
    let code = first_error_code(&diagnostics)
        .unwrap_or("AKR-R001")
        .to_owned();
    let summary = output
        .diagnostics
        .first()
        .map_or_else(|| "the command failed".to_owned(), |d| d.message.clone());
    Err(ToolError {
        class: class_of(&code),
        summary,
        diagnostics,
        wrote: false,
    })
}

fn write_context(session: &Session) -> akr_core::ops::WriteContext {
    akr_core::ops::WriteContext::new(&session.akr_dir)
}

/// Renders an `ops` outcome as §4's payload, or as §5's refusal.
///
/// `target` is the key the tool was asked about. A revision or a supersession touches two
/// revisions of it — the retired head and its successor — and `Applied::changes` is in key
/// order, so the successor is the *last* of them, not the first. The payload names the
/// revision the write produced, because that is the one the agent's next `base_rev` has to
/// be.
fn write_result(
    session: &Session,
    target: &LogicalKey,
    result: akr_core::ops::WriteResult,
) -> Result<Value, ToolError> {
    match result {
        Ok(applied) => {
            let first = applied
                .changes
                .iter()
                .filter(|change| &change.id.key == target)
                .max_by_key(|change| change.id.revision)
                .or_else(|| applied.changes.first());
            let path = applied
                .files
                .first()
                .map(|file| session.akr_dir.join(file))
                .and_then(|full| {
                    full.strip_prefix(&session.root)
                        .ok()
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                })
                .unwrap_or_default();
            let mut fields = vec![
                (
                    "key",
                    Value::string(first.map(|c| c.id.key.to_string()).unwrap_or_default()),
                ),
                (
                    "rev",
                    Value::integer(first.map_or(0, |c| i64::from(c.id.revision))),
                ),
                ("path", Value::string(path)),
                ("written", Value::bool(true)),
                ("lock_stale", Value::bool(applied.lock_stale)),
            ];
            // §4's `state` and `content_hash` describe the revision that just landed, so
            // they have to come from a workspace read *after* the write. `session` is the
            // snapshot the write was planned against: it does not know the new revision at
            // all, and for a state move it still holds the state the record left.
            if let Some(change) = first
                && let Ok(written) = Session::open(&session.global)
            {
                if let Some(record) = written.ledger.get(&change.id) {
                    fields.insert(2, ("state", Value::string(record.state.name())));
                }
                if let Some(text) = written.inputs.canonical_text.get(&change.id) {
                    let hash = akr_core::hash::content_hash(text);
                    fields.push(("content_hash", Value::string(hash.to_string())));
                }
            }
            Ok(Value::object(fields))
        }
        Err(refused) => {
            let mut diagnostics = vec![diagnostic_json(&refused.diagnostic())];
            diagnostics.extend(refused.diagnostics.iter().map(diagnostic_json));
            let mut error = ToolError {
                class: class_of(refused.code.as_str()),
                summary: refused.message.clone(),
                diagnostics,
                wrote: false,
            };
            // §4: the children are listed in the error payload, so the agent's next
            // message can name them. That is the moment the design cares most about.
            if !refused.unfinished_children.is_empty() {
                error.diagnostics.push(Value::object(vec![
                    ("code", Value::string(refused.code.as_str())),
                    ("severity", Value::string("error")),
                    (
                        "message",
                        Value::string("unfinished children need a disposition"),
                    ),
                    (
                        "unfinished_children",
                        Value::array(
                            refused
                                .unfinished_children
                                .iter()
                                .map(|child| {
                                    Value::object(vec![
                                        ("child", Value::string(child.key.to_string())),
                                        ("state", Value::string(child.state.name())),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]));
            }
            if !refused.unsatisfied_checks.is_empty() {
                error.diagnostics.push(Value::object(vec![
                    ("code", Value::string(refused.code.as_str())),
                    ("severity", Value::string("error")),
                    (
                        "message",
                        Value::string("acceptance checks are not satisfied"),
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
                ]));
            }
            Err(error)
        }
    }
}

fn diagnostic_json(diagnostic: &akr_core::diagnostics::Diagnostic) -> Value {
    let mut fields = vec![
        ("code", Value::string(diagnostic.code.as_str())),
        ("severity", Value::string("error")),
        ("message", Value::string(diagnostic.message.clone())),
    ];
    if let Some(rule) = diagnostic.rule {
        fields.push(("rule", Value::string(rule.to_string())));
    }
    if let Some(help) = &diagnostic.help {
        fields.push(("help", Value::string(help.clone())));
    }
    Value::object(fields)
}

fn environment(error: EnvError) -> ToolError {
    let mut mapped = ToolError::new(error.code, error.message.clone());
    // `AKR-I022` and `AKR-M002` are the deferred surfaces — search and import — and they
    // are environment failures in the sense that matters: not the agent's fault, and not
    // fixable by trying again.
    if mapped.class == Class::Invariant {
        mapped.class = Class::Environment;
    }
    mapped
}

fn dispositions(arguments: &Value) -> Result<Vec<akr_core::ops::DispositionRequest>, ToolError> {
    let mut out = Vec::new();
    for item in arguments
        .get("dispositions")
        .and_then(Value::as_array)
        .unwrap_or_default()
    {
        let child = item
            .get("child")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::new("AKR-C003", "each disposition needs a `child`"))?;
        let outcome_name = item
            .get("outcome")
            .and_then(Value::as_str)
            .ok_or_else(|| ToolError::new("AKR-C003", "each disposition needs an `outcome`"))?;
        let outcome = akr_core::model::Outcome::ALL
            .iter()
            .copied()
            .find(|candidate| candidate.name() == outcome_name)
            .ok_or_else(|| {
                ToolError::new(
                    "AKR-C004",
                    format!("`{outcome_name}` is not a disposition outcome"),
                )
            })?;
        out.push(akr_core::ops::DispositionRequest {
            child: key_of(child)?,
            outcome,
            into: item
                .get("into")
                .and_then(Value::as_str)
                .map(key_of)
                .transpose()?,
            note: item
                .get("note")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        });
    }
    Ok(out)
}

/// A revision's payload merged onto its head, so an edit naming one slot keeps the rest.
///
/// `knowledge.revise` takes the slots that change, not the whole record — an agent that
/// had to resend every slot would eventually drop one, and dropping a slot is how a record
/// silently loses a claim.
fn merged(arguments: &Value, head: &akr_core::model::Record) -> Value {
    let mut slots: Vec<(String, Value)> = Vec::new();
    for (slot, value) in &head.content {
        let rendered = match value {
            akr_core::model::ContentValue::Prose(text)
            | akr_core::model::ContentValue::Text(text) => Value::string(text.clone()),
            akr_core::model::ContentValue::Date(date) => Value::string(date.to_string()),
            akr_core::model::ContentValue::Commit(commit) => Value::string(commit.as_str()),
            akr_core::model::ContentValue::Enum(word) => Value::string(word.to_string()),
            akr_core::model::ContentValue::Strings(items) => {
                Value::array(items.iter().map(|s| Value::string(s.clone())).collect())
            }
            akr_core::model::ContentValue::Globs(items) => {
                Value::array(items.iter().map(|g| Value::string(g.as_str())).collect())
            }
            akr_core::model::ContentValue::Refs(items) => {
                Value::array(items.iter().map(|r| Value::string(r.to_string())).collect())
            }
        };
        slots.push((slot.name().to_owned(), rendered));
    }
    if let Some(Value::Object(given)) = arguments.get("slots") {
        for (name, value) in given {
            slots.retain(|(existing, _)| existing != name);
            slots.push((name.clone(), value.clone()));
        }
    }

    let mut relations: Vec<(String, Value)> = Vec::new();
    for (relation, targets) in &head.relations {
        relations.push((
            relation.name().to_owned(),
            Value::array(
                targets
                    .iter()
                    .map(|r| Value::string(r.to_string()))
                    .collect(),
            ),
        ));
    }
    if let Some(Value::Object(given)) = arguments.get("relations") {
        for (name, value) in given {
            relations.retain(|(existing, _)| existing != name);
            relations.push((name.clone(), value.clone()));
        }
    }

    let mut fields = vec![
        ("slots".to_owned(), Value::Object(slots)),
        ("relations".to_owned(), Value::Object(relations)),
    ];
    if let Some(scope) = arguments.get("scope") {
        fields.push(("scope".to_owned(), scope.clone()));
    } else if !head.scope.is_empty() {
        fields.push((
            "scope".to_owned(),
            Value::array(
                head.scope
                    .iter()
                    .map(|term| match term {
                        akr_core::model::ScopeTerm::All => Value::string("all"),
                        akr_core::model::ScopeTerm::Path(glob) => Value::string(glob.as_str()),
                        akr_core::model::ScopeTerm::Ref(reference) => {
                            Value::string(reference.to_string())
                        }
                    })
                    .collect(),
            ),
        ));
    }
    for name in ["claims", "retired_claims", "author", "created_at", "acceptance"] {
        if let Some(value) = arguments.get(name) {
            fields.push((name.to_owned(), value.clone()));
        }
    }
    if let Some(acceptance) = &head.acceptance
        && arguments.get("acceptance").is_none()
        && !acceptance.checks.is_empty()
    {
        fields.push((
            "acceptance".to_owned(),
            Value::array(
                acceptance
                    .checks
                    .iter()
                    .map(|check| {
                        let mut check_fields = vec![
                            ("id", Value::string(check.id.to_string())),
                            ("statement", Value::string(check.statement.clone())),
                            ("method", Value::string(check.method.name())),
                        ];
                        if let Some(command) = &check.command {
                            check_fields.push(("command", Value::string(command.clone())));
                        }
                        if !check.verified_by.is_empty() {
                            check_fields.push((
                                "verified_by",
                                Value::array(
                                    check
                                        .verified_by
                                        .iter()
                                        .map(|r| Value::string(r.to_string()))
                                        .collect(),
                                ),
                            ));
                        }
                        Value::object(check_fields)
                    })
                    .collect(),
            ),
        ));
    }
    if arguments.get("claims").is_none() && !head.claims.is_empty() {
        fields.push((
            "claims".to_owned(),
            Value::array(
                head.claims
                    .iter()
                    .map(|claim| {
                        Value::object(vec![
                            ("anchor", Value::string(claim.anchor.to_string())),
                            ("text", Value::string(claim.text.clone())),
                        ])
                    })
                    .collect(),
            ),
        ));
    }
    Value::Object(fields)
}

fn key_of(text: &str) -> Result<akr_core::model::LogicalKey, ToolError> {
    let trimmed = text.strip_prefix('@').unwrap_or(text);
    let trimmed = trimmed.split_once('/').map_or(trimmed, |(key, _)| key);
    akr_core::model::LogicalKey::parse(trimmed).map_err(|e| {
        ToolError::new(
            "AKR-C004",
            format!(
                "{text:?} is not a key: {e}; keys are dot-delimited — \
                 namespace.topic.slug — and the first segment must be a namespace \
                 declared in .akr/project.akr"
            ),
        )
    })
}

fn required_str<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::new("AKR-C003", format!("`{name}` is required")))
}

fn flag(arguments: &Value, name: &str) -> bool {
    arguments
        .get(name)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn field(result: &Value, name: &str) -> Value {
    result.get(name).cloned().unwrap_or(Value::Integer(0))
}
