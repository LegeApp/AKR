//! The read and build commands of `docs/07-cli.md` §6.
//!
//! Each returns its own text, its own JSON `result` object, and an [`Exit`]. Printing and
//! the envelope belong to `main`, so a command never has to know which form was asked for
//! until the last moment.

use crate::args::Command;
use crate::session::{
    EnvError, Exit, GRAMMAR_VERSION, Session, TOOL_VERSION, VOCABULARY_VERSION, diagnostic_json,
    is_fatal, report,
};
use akr_core::context::{Request, assemble, render_json, render_text};
use akr_core::diagnostics::Diagnostic;
use akr_core::json::Value;
use akr_core::model::{Date, Kind, Reference, Relation, RevisionId};
use akr_core::render::{View, check_views_current, render, write_views};
use std::collections::BTreeSet;

/// What a command produced.
pub struct Output {
    /// The human-readable form.
    pub text: String,
    /// The `result` object of the JSON envelope.
    pub result: Value,
    /// Diagnostics, already span-attached.
    pub diagnostics: Vec<Diagnostic>,
    /// The process exit status.
    pub exit: Exit,
    /// The commit the build resolved against, for the envelope.
    pub commit: Option<String>,
    /// The source-graph hash of the inputs, for the envelope.
    pub source_graph: String,
}

impl Output {
    fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            result: Value::Object(Vec::new()),
            diagnostics: Vec::new(),
            exit: Exit::Ok,
            commit: None,
            source_graph: String::new(),
        }
    }

    fn with_result(mut self, result: Value) -> Self {
        self.result = result;
        self
    }

    fn with_diagnostics(mut self, diagnostics: Vec<Diagnostic>, exit: Exit) -> Self {
        self.diagnostics = diagnostics;
        self.exit = exit;
        self
    }
}

/// Runs a command that needs no workspace.
///
/// # Errors
/// Never; the signature matches [`run`] so `main` can treat them alike.
pub fn run_standalone(command: &Command) -> Option<Result<Output, EnvError>> {
    match command {
        Command::Help => Some(Ok(Output::text(crate::args::help()))),
        Command::Version => Some(Ok(Output::text(format!("akr {TOOL_VERSION}\n")))),
        Command::Explain { subject } => Some(Ok(explain(subject))),
        Command::Init {
            project,
            namespaces,
        } => Some(crate::init::run(project.as_deref(), namespaces)),
        _ => None,
    }
}

/// Runs a command against a loaded workspace.
///
/// # Errors
/// [`EnvError`] when the environment is unusable.
pub fn run(session: &mut Session, command: &Command) -> Result<Output, EnvError> {
    // The envelope's `commit` and `source_graph_hash` are properties of the build, not of
    // any one command, so they are filled in once here rather than by each command that
    // happens to remember.
    let mut output = dispatch(session, command)?;
    output.commit = session.commit.as_ref().map(|c| c.as_str().to_owned());
    output.source_graph = session.source_graph();
    Ok(output)
}

fn dispatch(session: &mut Session, command: &Command) -> Result<Output, EnvError> {
    match command {
        Command::Check {
            review_clean,
            views_current,
        } => check(session, *review_clean, *views_current),
        Command::Build => build(session),
        Command::Fmt { check, paths } => crate::fmt::run(session, *check, paths),
        Command::View { name } => view(session, name),
        Command::Get {
            reference,
            history,
            relations,
        } => get(session, reference, *history, *relations),
        Command::WhyCurrent { reference } => why_current(session, reference),
        Command::Impact {
            reference,
            git_diff,
        } => impact(session, reference.as_deref(), git_diff.as_deref()),
        Command::ReviewQueue {
            stale_only,
            at_risk_only,
            kinds,
        } => review_queue(session, *stale_only, *at_risk_only, kinds),
        Command::Lock { check } => lock(session, *check),
        Command::Context {
            goal,
            paths,
            budget,
        } => context(session, goal, paths, *budget),
        Command::Search { .. } => Err(EnvError::new(
            "AKR-I022",
            "search requires the full-text index, which arrives with phase P7",
        )
        .help("use `akr get` for a known key, or `akr context` for a whole bundle")),
        Command::Import { .. } => Err(EnvError::new("AKR-M002", "import arrives with phase P8")
            .help("see docs/12-migration.md for the workflow it will implement")),
        Command::Write { name } => Err(EnvError::new(
            "AKR-C001",
            format!("`akr {name}` arrives with phase P6c"),
        )
        .help("the write operations are landing in akr-core now; see docs/07-cli.md §4")),
        Command::Help | Command::Version | Command::Explain { .. } | Command::Init { .. } => {
            unreachable!("handled by run_standalone")
        }
    }
}

// -------------------------------------------------------------------------------------
// check
// -------------------------------------------------------------------------------------

fn check(
    session: &mut Session,
    review_clean: bool,
    views_current: bool,
) -> Result<Output, EnvError> {
    session.attach_lock();
    let model = session.resolve();
    let mut diagnostics = session.diagnostics(&model);
    let queue = session.review_queue();
    diagnostics.extend(queue.diagnostics.clone());

    let mut text = String::new();
    text.push_str(&format!("akr check — {}\n", session.ledger.project.name));
    text.push_str(&summary_lines(session, &model));
    text.push('\n');

    if views_current {
        text.push_str("  stages A-D                                 ok\n");
        let freshness = session.freshness(&queue);
        let context = akr_core::render::RenderContext::new(&model, &freshness);
        let view_diagnostics = check_views_current(&session.view_dir(), context).map_err(|e| {
            EnvError::new("AKR-E001", format!("cannot read the view directory: {e}"))
        })?;
        text.push_str(&format!(
            "  stage F  emit (in memory)   {} views        {}\n",
            View::ALL.len(),
            if view_diagnostics.is_empty() {
                "ok"
            } else {
                "FAILED"
            }
        ));
        for &view in View::ALL {
            let state = if view_diagnostics
                .iter()
                .any(|d| d.message.contains(view.file_name()))
            {
                "DIFFERS"
            } else {
                "current"
            };
            text.push_str(&format!("    {:<24} {state}\n", view.file_name()));
        }
        diagnostics.extend(view_diagnostics);
    } else {
        text.push_str(&stage_lines(session, &model));
        text.push('\n');
        text.push_str(&resolve_detail(session, &model));
        text.push('\n');
        text.push_str("  build facts (not diagnostics):\n");
        text.push_str(&format!(
            "    {} records stale, {} at risk — see `akr review-queue`\n",
            queue.stale.len(),
            queue.at_risk.len()
        ));
    }

    if review_clean && let Some(diagnostic) = queue.review_clean_diagnostic() {
        let mut diagnostic = diagnostic;
        // Point at the first stale record, so the caret lands on something actionable.
        if let Some(first) = queue.stale.first() {
            diagnostic.primary.subject = akr_core::diagnostics::Subject::Revision(first.id.clone());
            diagnostic.primary.message = Some(format!(
                "stale: {}",
                cause_text(&first.cause, session.today)
            ));
            session.spans.attach(&mut diagnostic);
        }
        diagnostics.push(diagnostic);
    }

    diagnostics.sort_by_key(Diagnostic::sort_key);
    let fatal = diagnostics
        .iter()
        .filter(|d| is_fatal(d, session.global.profile))
        .count();
    let exit = if fatal > 0 {
        Exit::Diagnostics
    } else {
        Exit::Ok
    };

    text.push('\n');
    if diagnostics.is_empty() {
        text.push_str("no diagnostics\n");
    } else {
        let (rendered, _) = report(&diagnostics, &session.sources, session.global.profile);
        text.push_str(&rendered);
    }

    let counts = counts_of(session, &model);
    let result = Value::object(vec![
        ("records", Value::integer(counts.records as i64)),
        ("revisions", Value::integer(counts.revisions as i64)),
        ("files", Value::integer(counts.files as i64)),
        ("references", Value::integer(counts.references as i64)),
        ("heads", Value::integer(counts.heads as i64)),
        (
            "checks",
            Value::object(vec![
                ("total", Value::integer(counts.checks as i64)),
                ("satisfied", Value::integer(counts.satisfied as i64)),
            ]),
        ),
        ("stale", Value::integer(queue.stale.len() as i64)),
        ("at_risk", Value::integer(queue.at_risk.len() as i64)),
    ]);

    Ok(Output::text(text)
        .with_result(result)
        .with_diagnostics(diagnostics, exit))
}

struct Counts {
    records: usize,
    revisions: usize,
    files: usize,
    references: usize,
    heads: usize,
    checks: usize,
    satisfied: usize,
}

fn counts_of(session: &Session, model: &akr_core::resolve::ResolvedModel<'_>) -> Counts {
    Counts {
        records: session.ledger.keys().len(),
        revisions: session.ledger.records().len(),
        // Source files only. The lock is build output, not input, and the lock's own
        // `source` list is the count this has to agree with (`spec/akr-lock.md` §2).
        files: session.inputs.sources.len(),
        references: model.edges.len(),
        heads: model.heads.len(),
        checks: model.acceptance.len(),
        satisfied: model
            .acceptance
            .iter()
            .filter(|v| v.verdict.is_satisfied())
            .count(),
    }
}

fn summary_lines(session: &Session, model: &akr_core::resolve::ResolvedModel<'_>) -> String {
    let counts = counts_of(session, model);
    let commit = session
        .commit
        .as_ref()
        .map_or_else(|| "(none)".to_owned(), |c| c.as_str()[..8].to_owned());
    format!(
        "  {} records, {} revisions, {} files\n  commit {commit}, grammar {GRAMMAR_VERSION}, \
         vocabulary {VOCABULARY_VERSION}, today {}\n",
        counts.records, counts.revisions, counts.files, session.today
    )
}

fn stage_lines(session: &Session, model: &akr_core::resolve::ResolvedModel<'_>) -> String {
    let counts = counts_of(session, model);
    let mark = |bad: bool| if bad { "FAILED" } else { "ok" };
    let parse_failed = !session.parse_diagnostics.is_empty();
    let stage_failed = |stage: akr_core::diagnostics::Stage| {
        model
            .diagnostics
            .iter()
            .any(|d| d.code.stage() == Some(stage))
    };
    format!(
        "  stage A  parse          {:>2} revisions       {}\n\
         \x20 stage B  type-check     {:>2} revisions       {}\n\
         \x20 stage C  link          {:>3} references      {}\n\
         \x20 stage D  resolve        {:>2} heads           {}\n",
        counts.revisions,
        mark(parse_failed),
        counts.revisions,
        mark(stage_failed(akr_core::diagnostics::Stage::Type)),
        counts.references,
        mark(stage_failed(akr_core::diagnostics::Stage::Link)),
        counts.heads,
        mark(stage_failed(akr_core::diagnostics::Stage::Resolve)),
    )
}

fn resolve_detail(session: &Session, model: &akr_core::resolve::ResolvedModel<'_>) -> String {
    let ledger = &session.ledger;
    let live_heads = model
        .heads
        .values()
        .filter(|id| ledger.get(id).is_some_and(akr_core::model::Record::is_live))
        .count();
    let chain_heads = model.heads.len() - live_heads;

    let chains: Vec<String> = model
        .supersession
        .iter()
        .filter(|(_, chain)| chain.len() > 1)
        .map(|(key, _)| key.to_string())
        .collect();

    let unfinished: usize = ledger.records().iter().map(|r| r.dispositions.len()).sum();

    let topics: BTreeSet<String> = ledger
        .records()
        .iter()
        .filter(|r| r.is_live())
        .filter_map(|r| r.topic.as_ref().map(ToString::to_string))
        .collect();

    let plans_live = ledger
        .records()
        .iter()
        .filter(|r| r.is_live() && !r.targets(Relation::PlanOfRecord).is_empty())
        .count();
    let plans_total = ledger
        .records()
        .iter()
        .filter(|r| !r.targets(Relation::PlanOfRecord).is_empty())
        .count();

    let sealed = ledger.records().iter().filter(|r| r.is_sealed()).count();
    let matched = ledger
        .facts
        .seals
        .values()
        .filter(|fact| match (&fact.recorded, &fact.computed) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        })
        .count();

    let contradictions: usize = ledger
        .records()
        .iter()
        .map(|r| r.targets(Relation::Contradicts).len())
        .sum();
    let acknowledged: usize = ledger
        .records()
        .iter()
        .filter(|r| r.acknowledged && !r.targets(Relation::Contradicts).is_empty())
        .count();

    let counts = counts_of(session, model);
    let mut out = String::from("  resolve detail\n");
    out.push_str(&format!(
        "    heads                        {live_heads} live, {chain_heads} resolved by supersession chain\n"
    ));
    out.push_str(&format!(
        "    supersession chains          {:>2} ({})\n",
        chains.len(),
        wrap_list(&chains, 37)
    ));
    out.push_str(&format!(
        "    dispositions checked          {unfinished} unfinished children, {unfinished} dispositioned\n"
    ));
    out.push_str(&format!(
        "    topics                        {} on live records, no overlapping pair\n",
        topics.len()
    ));
    out.push_str(&format!(
        "    plan_of_record                {plans_live} live, {} superseded\n",
        plans_total - plans_live
    ));
    out.push_str(&format!(
        "    acceptance                    {} checks, {} satisfied\n",
        counts.checks, counts.satisfied
    ));
    out.push_str(&format!(
        "    sealed revisions             {sealed} hashed, {matched} match akr.lock\n"
    ));
    out.push_str(&format!(
        "    contradictions                {contradictions} declared, {acknowledged} acknowledged\n"
    ));
    out
}

/// Wraps a comma-separated list, continuing under the opening parenthesis.
fn wrap_list(items: &[String], indent: usize) -> String {
    let joined = items.join(", ");
    if joined.len() <= 40 {
        return joined;
    }
    let pad = " ".repeat(indent);
    items.join(&format!(",\n{pad}"))
}

fn cause_text(cause: &akr_core::freshness::StaleCause, today: Date) -> String {
    match cause {
        akr_core::freshness::StaleCause::Watch { glob, commit, .. } => format!(
            "watches {:?} matched by {}",
            glob.as_str(),
            &commit.as_str()[..8]
        ),
        akr_core::freshness::StaleCause::ReviewAfter { date } => {
            let days = days_between(*date, today);
            format!("review_after {date} passed {days} days ago")
        }
    }
}

/// Whole days from `from` to `to`, proleptic Gregorian.
///
/// Hand-rolled because the tool has no dependencies (`docs/13-implementation-roadmap.md`
/// §4) and because a day count is the whole of the calendar arithmetic AKR needs.
fn days_between(from: Date, to: Date) -> i64 {
    days_from_civil(to) - days_from_civil(from)
}

/// Howard Hinnant's `days_from_civil`, the inverse of the one in `session.rs`.
fn days_from_civil(date: Date) -> i64 {
    let y = i64::from(date.year) - i64::from(date.month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(date.month);
    let d = i64::from(date.day);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

// -------------------------------------------------------------------------------------
// build
// -------------------------------------------------------------------------------------

fn build(session: &mut Session) -> Result<Output, EnvError> {
    session.attach_lock();
    let model = session.resolve();
    let diagnostics = session.diagnostics(&model);
    let fatal = diagnostics
        .iter()
        .filter(|d| is_fatal(d, session.global.profile))
        .count();
    if fatal > 0 {
        // The pipeline halts at the failing stage boundary and produces no output.
        let (rendered, _) = report(&diagnostics, &session.sources, session.global.profile);
        return Ok(Output::text(rendered).with_diagnostics(diagnostics, Exit::Diagnostics));
    }

    let queue = session.review_queue();
    let freshness = session.freshness(&queue);
    let context = akr_core::render::RenderContext::new(&model, &freshness);
    let written = write_views(&session.view_dir(), context)
        .map_err(|e| EnvError::new("AKR-E001", format!("cannot write views: {e}")))?;

    let lock = model.to_lock();
    let lock_path = session.akr_dir.join("akr.lock");
    let rendered = lock.render();
    let lock_changed = std::fs::read_to_string(&lock_path).ok().as_deref() != Some(&rendered);
    if lock_changed {
        std::fs::write(&lock_path, &rendered)
            .map_err(|e| EnvError::new("AKR-C042", format!("cannot write akr.lock: {e}")))?;
    }

    let counts = counts_of(session, &model);
    let mut text = String::new();
    text.push_str(&format!(
        "parsed {} revisions in {} files\n",
        counts.revisions, counts.files
    ));
    text.push_str(&format!(
        "resolved {} heads, {} superseded revisions\n",
        counts.heads,
        counts.revisions - counts.heads
    ));
    text.push_str(&format!(
        "{} stale records, {} at risk (see akr review-queue)\n",
        queue.stale.len(),
        queue.at_risk.len()
    ));
    text.push_str(&format!(
        "wrote docs/generated/ ({} views, {} changed)\n",
        View::ALL.len(),
        written.len()
    ));
    text.push_str(if lock_changed {
        "wrote akr.lock\n"
    } else {
        "akr.lock unchanged\n"
    });

    let result = Value::object(vec![
        ("views_written", Value::integer(written.len() as i64)),
        ("lock_changed", Value::bool(lock_changed)),
        ("stale", Value::integer(queue.stale.len() as i64)),
        ("at_risk", Value::integer(queue.at_risk.len() as i64)),
    ]);
    Ok(Output::text(text).with_result(result))
}

// -------------------------------------------------------------------------------------
// view, get, why-current
// -------------------------------------------------------------------------------------

fn view(session: &Session, name: &str) -> Result<Output, EnvError> {
    let Some(view) = View::from_name(name) else {
        return Err(EnvError::new(
            "AKR-E003",
            format!(
                "unknown view {name:?}; known views are {}",
                View::ALL
                    .iter()
                    .map(|v| v.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    };
    let model = session.resolve();
    let queue = session.review_queue();
    let freshness = session.freshness(&queue);
    let context = akr_core::render::RenderContext::new(&model, &freshness);
    let Some(text) = render(view, context) else {
        return Err(EnvError::new(
            "AKR-E003",
            format!("the {} renderer arrives with a later phase", view.name()),
        )
        .help("`akr view roadmap` is implemented; the other five follow"));
    };
    Ok(Output::text(text.clone()).with_result(Value::object(vec![
        ("view", Value::string(view.name())),
        ("text", Value::string(text)),
    ])))
}

fn get(
    session: &Session,
    reference: &str,
    history: bool,
    relations: bool,
) -> Result<Output, EnvError> {
    let model = session.resolve();
    let ledger = &session.ledger;
    let parsed = Reference::parse(reference)
        .map_err(|e| EnvError::new("AKR-L001", format!("{reference}: {e}")))?;
    let Some(record) = ledger.resolve(&parsed).ok().flatten() else {
        return Err(EnvError::new(
            "AKR-L001",
            format!("{reference} does not resolve to a record"),
        ));
    };

    let queue = session.review_queue();
    let freshness = session.freshness(&queue);
    let mut text = format!(
        "{} : {}    state {}    {}\n",
        record.id,
        record.kind.name(),
        record.state.name(),
        if model.is_head(&record.id) {
            "head"
        } else {
            "not head"
        }
    );
    text.push_str(&format!("  title    {}\n", record.title));
    if !record.scope.is_empty() {
        text.push_str(&format!(
            "  scope    {}\n",
            record
                .scope
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if let Some(topic) = &record.topic {
        text.push_str(&format!("  topic    {topic}\n"));
    }
    if freshness.is_stale(&record.id) {
        text.push_str("  freshness  stale\n");
    } else if let Some(flag) = freshness.at_risk(&record.id) {
        text.push_str(&format!(
            "  freshness  at_risk (depth {}, via {} -> {})\n",
            flag.depth,
            flag.via,
            flag.path
                .iter()
                .map(|id| format!("@{id}"))
                .collect::<Vec<_>>()
                .join(" -> ")
        ));
    }
    if let Some(body) = akr_core::context::body_of(record) {
        text.push('\n');
        for line in body.lines() {
            text.push_str(&format!("  {line}\n"));
        }
    }
    if !record.claims.is_empty() {
        text.push_str("\n  claims\n");
        for claim in &record.claims {
            let first = claim.text.lines().next().unwrap_or_default();
            text.push_str(&format!("    #{:<14} {first}\n", claim.anchor.as_str()));
        }
    }
    if relations {
        text.push_str("\n  relations (outbound)\n");
        for (relation, references) in &record.relations {
            for target in references {
                text.push_str(&format!("    {:<14} -> {target}\n", relation.name()));
            }
        }
        text.push_str("  relations (inbound)\n");
        for other in ledger.records() {
            for (relation, references) in &other.relations {
                for reference in references {
                    if ledger
                        .resolve(reference)
                        .ok()
                        .flatten()
                        .is_some_and(|t| t.id == record.id)
                    {
                        text.push_str(&format!("    {:<14} <- @{}\n", relation.name(), other.id));
                    }
                }
            }
        }
    }
    if history {
        text.push_str("\n  history\n");
        for revision in ledger.revisions_of(&record.id.key) {
            text.push_str(&format!(
                "    /{}  {:<12} {}\n",
                revision.id.revision,
                revision.state.name(),
                revision.title
            ));
        }
    }

    let result = Value::object(vec![
        ("key", Value::string(record.id.key.to_string())),
        ("rev", Value::integer(i64::from(record.id.revision))),
        ("kind", Value::string(record.kind.name())),
        ("state", Value::string(record.state.name())),
        ("title", Value::string(record.title.clone())),
        ("is_head", Value::bool(model.is_head(&record.id))),
    ]);
    Ok(Output::text(text).with_result(result))
}

fn why_current(session: &Session, reference: &str) -> Result<Output, EnvError> {
    let model = session.resolve();
    let ledger = &session.ledger;
    let parsed = Reference::parse(reference)
        .map_err(|e| EnvError::new("AKR-L001", format!("{reference}: {e}")))?;
    let Some(record) = ledger.resolve(&parsed).ok().flatten() else {
        return Err(EnvError::new(
            "AKR-L001",
            format!("{reference} does not resolve to a record"),
        ));
    };
    let key = &record.id.key;

    let mut text = format!("{key} — head is revision {}\n", record.id.revision);
    text.push_str("\n  head resolution\n");
    for revision in ledger.revisions_of(key) {
        let marker = if model.is_head(&revision.id) {
            "  -> head"
        } else {
            ""
        };
        text.push_str(&format!(
            "    revision {}  {:<12} {}{marker}\n",
            revision.id.revision,
            revision.state.name(),
            if revision.is_live() { "LIVE" } else { "    " }
        ));
    }
    if let Some(hash) = model.content_hash(&record.id) {
        text.push_str(&format!(
            "    lock: resolved to /{}, hash {}\n",
            record.id.revision,
            &hash.0[..20]
        ));
    }

    let queue = session.review_queue();
    let freshness = session.freshness(&queue);
    text.push_str("\n  freshness\n");
    if let Some(stale) = queue.stale.iter().find(|s| s.id == record.id) {
        text.push_str(&format!(
            "    STALE: {}\n",
            cause_text(&stale.cause, session.today)
        ));
    } else if let Some(flag) = freshness.at_risk(&record.id) {
        text.push_str(&format!("    AT RISK (depth {})\n", flag.depth));
        for hop in &flag.path {
            text.push_str(&format!("      via {} -> @{hop}\n", flag.via));
        }
    } else {
        text.push_str("    not stale, not at risk\n");
    }

    Ok(Output::text(text).with_result(Value::object(vec![
        ("key", Value::string(key.to_string())),
        ("head", Value::integer(i64::from(record.id.revision))),
    ])))
}

// -------------------------------------------------------------------------------------
// impact, review-queue
// -------------------------------------------------------------------------------------

fn impact(
    session: &Session,
    reference: Option<&str>,
    git_diff: Option<&str>,
) -> Result<Output, EnvError> {
    let ledger = &session.ledger;
    if let Some(range) = git_diff {
        return impact_range(session, range);
    }

    let reference = reference.expect("argument parsing requires one of the two");
    let parsed = Reference::parse(reference)
        .map_err(|e| EnvError::new("AKR-L001", format!("{reference}: {e}")))?;
    let Some(record) = ledger.resolve(&parsed).ok().flatten() else {
        return Err(EnvError::new(
            "AKR-L001",
            format!("{reference} does not resolve to a record"),
        ));
    };
    let queue = session.review_queue();
    let stale: BTreeSet<RevisionId> = [record.id.clone()].into();
    let dependents = akr_core::graph::propagate_staleness(ledger, &stale);

    let flag = if queue.stale_set().contains(&record.id) {
        "  [STALE]"
    } else {
        ""
    };
    let mut text = format!(
        "@{}  {}  {}{flag}\n\ndependents along supported_by, depends_on, derived_from\n\n",
        record.id,
        record.kind.name(),
        record.state.name()
    );
    let id_width = width(dependents.iter().map(|d| d.id.to_string().len() + 1), 3);
    let kind_width = width(
        dependents
            .iter()
            .filter_map(|d| ledger.get(&d.id))
            .map(|r| r.kind.name().len()),
        2,
    );
    for entry in &dependents {
        let dependent = ledger.get(&entry.id);
        text.push_str(&format!(
            "  depth {}  {:<id_width$}{:<kind_width$}{}\n",
            entry.depth,
            format!("@{}", entry.id),
            dependent.map_or("", |r| r.kind.name()),
            dependent.map_or("", |r| r.state.name()),
        ));
        text.push_str(&via_lines(&entry.via, &entry.path, 13));
    }
    if dependents.is_empty() {
        text.push_str("  (none)\n");
    }
    text.push_str(&format!(
        "\n{} dependent{}, maximum depth {}\n",
        dependents.len(),
        if dependents.len() == 1 { "" } else { "s" },
        dependents.iter().map(|d| d.depth).max().unwrap_or(0)
    ));

    let result = Value::object(vec![
        ("mode", Value::string("ref")),
        ("key", Value::string(record.id.key.to_string())),
        ("rev", Value::integer(i64::from(record.id.revision))),
        (
            "dependents",
            Value::array(
                dependents
                    .iter()
                    .map(|d| {
                        Value::object(vec![
                            ("key", Value::string(d.id.key.to_string())),
                            ("rev", Value::integer(i64::from(d.id.revision))),
                            ("depth", Value::integer(d.depth as i64)),
                            ("via", Value::string(d.via.name())),
                            (
                                "path",
                                Value::array(
                                    d.path
                                        .iter()
                                        .map(|id| Value::string(id.key.to_string()))
                                        .collect(),
                                ),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    Ok(Output::text(text).with_result(result))
}

/// `akr impact --git-diff A..B`: what a range would make questionable
/// (`docs/10-freshness-and-git.md` §6).
fn impact_range(session: &Session, range: &str) -> Result<Output, EnvError> {
    let ledger = &session.ledger;
    let Some(repository) = &session.repository else {
        return Err(EnvError::new("AKR-G001", "not inside a git repository"));
    };
    // An unresolvable end is a diagnostic, not an environment failure: the checkout is
    // fine, the argument is not. `docs/07-cli.md` §3 puts `AKR-G013` outside both the
    // usage list and the environment list, which leaves exit 1.
    let Some((from, to)) = range.split_once("..") else {
        return Ok(revision_error(range, "expected A..B"));
    };
    // `git rev-parse` resolves a branch or `HEAD`; a 40-hex commit that is not in the
    // repository resolves too, so both ends are checked for membership as well.
    let resolve = |text: &str| -> Option<akr_core::model::Commit> {
        let commit = repository.rev_parse(text).ok()?;
        repository.contains(&commit).then_some(commit)
    };
    let (Some(from), Some(to)) = (resolve(from), resolve(to)) else {
        let bad = if resolve(from).is_none() { from } else { to };
        return Ok(revision_error(
            bad,
            &format!("{bad:?} is not a commit in this repository"),
        ));
    };

    // The baseline is A, not HEAD. Impact asks what the range *would* make questionable,
    // so a record that this very range invalidated must not be filtered out for being
    // stale at HEAD — which it is precisely because the range happened.
    let already = akr_core::freshness::derive(ledger, repository, &from, session.today)
        .map(|queue| queue.stale_set())
        .unwrap_or_default();
    let impact = akr_core::freshness::impact_of_range(ledger, repository, &from, &to, &already)
        .map_err(|e| EnvError::new("AKR-G002", e.to_string()))?;

    let mut text = format!(
        "range {}..{}, {} commit{}\n\n",
        &from.as_str()[..8],
        &to.as_str()[..8],
        impact.commits,
        if impact.commits == 1 { "" } else { "s" }
    );
    text.push_str("touched paths\n");
    for path in &impact.touched {
        text.push_str(&format!("  {path}\n"));
    }

    if impact.newly_stale.is_empty() {
        text.push_str("\nnewly stale:   none\n");
    } else {
        text.push_str(&format!("\nnewly stale ({})\n", impact.newly_stale.len()));
        let id_width = width(
            impact
                .newly_stale
                .iter()
                .map(|s| s.id.to_string().len() + 1),
            3,
        );
        let kind_width = width(
            impact
                .newly_stale
                .iter()
                .filter_map(|s| ledger.get(&s.id))
                .map(|r| r.kind.name().len()),
            2,
        );
        for entry in &impact.newly_stale {
            let record = ledger.get(&entry.id);
            text.push_str(&format!(
                "  {:<id_width$}{:<kind_width$}{}\n",
                format!("@{}", entry.id),
                record.map_or("", |r| r.kind.name()),
                record.map_or("", |r| r.state.name()),
            ));
            text.push_str(&format!(
                "      {}\n",
                cause_text(&entry.cause, session.today)
            ));
            if let (Some(observed), akr_core::freshness::StaleCause::Watch { commit, .. }) =
                (&entry.observed_at, &entry.cause)
            {
                text.push_str(&format!(
                    "      observed_at {}, which {} descends from\n",
                    &observed.as_str()[..8],
                    &commit.as_str()[..8]
                ));
            }
        }
    }

    if impact.newly_at_risk.is_empty() {
        text.push_str("newly at risk: none\n");
    } else {
        text.push_str(&format!(
            "\nnewly at risk ({})\n",
            impact.newly_at_risk.len()
        ));
        let id_width = width(
            impact
                .newly_at_risk
                .iter()
                .map(|r| r.id.to_string().len() + 1),
            3,
        );
        for entry in &impact.newly_at_risk {
            text.push_str(&format!(
                "  {:<id_width$}depth {}\n",
                format!("@{}", entry.id),
                entry.depth
            ));
            text.push_str(&via_lines_bare(&entry.via, &entry.path, 6));
        }
    }

    let result = Value::object(vec![
        ("mode", Value::string("git_diff")),
        (
            "range",
            Value::object(vec![
                ("from", Value::string(from.as_str())),
                ("to", Value::string(to.as_str())),
                ("commits", Value::integer(impact.commits as i64)),
            ]),
        ),
        (
            "touched_paths",
            Value::array(impact.touched.iter().map(Value::string).collect()),
        ),
        (
            "newly_stale",
            Value::array(
                impact
                    .newly_stale
                    .iter()
                    .map(|s| stale_json(session, s))
                    .collect(),
            ),
        ),
        (
            "newly_at_risk",
            Value::array(
                impact
                    .newly_at_risk
                    .iter()
                    .map(|r| at_risk_json(session, r))
                    .collect(),
            ),
        ),
    ]);
    Ok(Output::text(text).with_result(result))
}

/// The `AKR-G013` diagnostic a bad `--git-diff` end raises.
fn revision_error(argument: &str, message: &str) -> Output {
    let diagnostic = Diagnostic {
        code: akr_core::git::codes::G013,
        severity: akr_core::diagnostics::Severity::Error,
        rule: None,
        message: format!("--git-diff: {message}"),
        primary: akr_core::diagnostics::Label::new(akr_core::diagnostics::Subject::File(
            argument.to_owned(),
        )),
        notes: Vec::new(),
        help: Some("AKR takes full 40-hex commits, never abbreviations (D-008)".to_owned()),
    };
    let text = format!(
        "{}\n1 error\n",
        akr_core::diagnostics::render(&diagnostic, &akr_core::diagnostics::SourceMap::new())
    );
    Output {
        text,
        result: Value::Object(Vec::new()),
        diagnostics: vec![diagnostic],
        exit: Exit::Diagnostics,
        commit: None,
        source_graph: String::new(),
    }
}

/// A column width: the widest entry plus a gap, or nothing when there are no entries.
fn width(lengths: impl Iterator<Item = usize>, gap: usize) -> usize {
    lengths.max().map_or(0, |longest| longest + gap)
}

/// `via <relation> -> @a` and, for a deeper path, the continuation lines under it.
fn via_lines(via: &akr_core::model::Relation, path: &[RevisionId], indent: usize) -> String {
    let pad = " ".repeat(indent);
    let relation = format!("{:<12}", via.to_string());
    let hang = " ".repeat(indent + 4 + 12 + 1);
    let mut out = String::new();
    for (index, id) in path.iter().enumerate() {
        if index == 0 {
            out.push_str(&format!("{pad}via {relation} -> @{id}\n"));
        } else {
            out.push_str(&format!("{hang}-> @{id}\n"));
        }
    }
    out
}

/// The same, without the leading `via` keyword.
fn via_lines_bare(via: &akr_core::model::Relation, path: &[RevisionId], indent: usize) -> String {
    let pad = " ".repeat(indent);
    let hang = " ".repeat(indent + 12 + 1);
    let mut out = String::new();
    for (index, id) in path.iter().enumerate() {
        if index == 0 {
            out.push_str(&format!("{pad}{:<12} -> @{id}\n", via.to_string()));
        } else {
            out.push_str(&format!("{hang}-> @{id}\n"));
        }
    }
    out
}

fn review_queue(
    session: &Session,
    stale_only: bool,
    at_risk_only: bool,
    kinds: &[String],
) -> Result<Output, EnvError> {
    let ledger = &session.ledger;
    let queue = session.review_queue();
    let wanted: Vec<Kind> = kinds.iter().filter_map(|k| Kind::from_name(k)).collect();
    let keep = |id: &RevisionId| {
        wanted.is_empty()
            || ledger
                .get(id)
                .is_some_and(|record| wanted.contains(&record.kind))
    };

    let stale: Vec<_> = queue
        .stale
        .iter()
        .filter(|s| !at_risk_only && keep(&s.id))
        .collect();
    let at_risk: Vec<_> = queue
        .at_risk
        .iter()
        .filter(|r| !stale_only && keep(&r.id))
        .collect();

    let mut text = format!(
        "review queue at {}, today {}\n",
        session
            .commit
            .as_ref()
            .map_or_else(|| "(no commit)".to_owned(), |c| c.as_str()[..8].to_owned()),
        session.today
    );

    if !at_risk_only {
        text.push_str(&format!("\nSTALE ({})\n", stale.len()));
        let id_width = width(stale.iter().map(|s| s.id.to_string().len()), 3);
        let kind_width = width(
            stale
                .iter()
                .filter_map(|s| ledger.get(&s.id))
                .map(|r| r.kind.name().len()),
            2,
        );
        for entry in &stale {
            let Some(record) = ledger.get(&entry.id) else {
                continue;
            };
            text.push_str(&format!(
                "\n  {:<id_width$}{:<kind_width$}{}\n",
                entry.id.to_string(),
                record.kind.name(),
                record.state.name()
            ));
            text.push_str(&format!(
                "      cause      {}\n",
                cause_text(&entry.cause, session.today)
            ));
            if let Some(observed) = &entry.observed_at {
                text.push_str(&format!(
                    "      observed   {}{}\n",
                    &observed.as_str()[..8],
                    distance_note(session, observed, &entry.cause)
                ));
            }
            // Every watch glob, not only the one that matched: a reader deciding whether
            // to re-observe needs to know what else the record is watching.
            for glob in akr_core::freshness::watches(record) {
                let matched = matches!(
                    &entry.cause,
                    akr_core::freshness::StaleCause::Watch { glob: g, .. } if *g == glob
                );
                text.push_str(&format!(
                    "      watches    {:?}{}\n",
                    glob.as_str(),
                    if matched {
                        String::new()
                    } else {
                        " — not matched since observed_at".to_owned()
                    }
                ));
            }
            if let Some(date) = akr_core::freshness::review_after(record)
                && !matches!(
                    entry.cause,
                    akr_core::freshness::StaleCause::ReviewAfter { .. }
                )
            {
                text.push_str(&format!("      review     {date} (not yet due)\n"));
            }
            let contradicts: Vec<String> = record
                .targets(akr_core::model::Relation::Contradicts)
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            if !contradicts.is_empty() {
                text.push_str(&format!(
                    "      note       contradicts {}\n                 ({})\n",
                    contradicts.join(", "),
                    if record.acknowledged {
                        "acknowledged"
                    } else {
                        "not acknowledged"
                    }
                ));
            }
        }
    }
    if !stale_only {
        text.push_str(&format!("\nAT RISK ({})\n", at_risk.len()));
        let id_width = width(at_risk.iter().map(|r| r.id.to_string().len()), 3);
        let kind_width = width(
            at_risk
                .iter()
                .filter_map(|r| ledger.get(&r.id))
                .map(|r| r.kind.name().len()),
            2,
        );
        for entry in &at_risk {
            let record = ledger.get(&entry.id);
            text.push_str(&format!(
                "\n  depth {}  {:<id_width$}{:<kind_width$}{}\n",
                entry.depth,
                entry.id.to_string(),
                record.map_or("", |r| r.kind.name()),
                record.map_or("", |r| r.state.name())
            ));
            text.push_str(&via_lines(&entry.via, &entry.path, 13));
        }
    }
    let mut summary = Vec::new();
    if !at_risk_only {
        summary.push(format!("{} stale", stale.len()));
    }
    if !stale_only {
        summary.push(format!("{} at risk", at_risk.len()));
        if let Some(depth) = at_risk.iter().map(|r| r.depth).max() {
            summary.push(format!("maximum propagation depth {depth}"));
        }
    }
    text.push_str(&format!("\n{}\n", summary.join(", ")));

    let result = Value::object(vec![
        ("today", Value::string(session.today.to_string())),
        (
            "stale",
            Value::array(stale.iter().map(|s| stale_json(session, s)).collect()),
        ),
        (
            "at_risk",
            Value::array(at_risk.iter().map(|r| at_risk_json(session, r)).collect()),
        ),
        (
            "counts",
            Value::object(vec![
                ("stale", Value::integer(stale.len() as i64)),
                ("at_risk", Value::integer(at_risk.len() as i64)),
                (
                    "max_depth",
                    Value::integer(at_risk.iter().map(|r| r.depth).max().unwrap_or(0) as i64),
                ),
            ]),
        ),
    ]);
    // Exit 0 regardless of queue length: a non-empty queue is normal and healthy.
    Ok(Output::text(text).with_result(result))
}

/// How far the observation sits behind the commit that invalidated it.
///
/// Purely informative, and silently absent when git cannot answer — a missing count is
/// better than an impact report that fails because a commit was garbage-collected.
fn distance_note(
    session: &Session,
    observed: &akr_core::model::Commit,
    cause: &akr_core::freshness::StaleCause,
) -> String {
    let akr_core::freshness::StaleCause::Watch { commit, .. } = cause else {
        return String::new();
    };
    let Some(repository) = &session.repository else {
        return String::new();
    };
    match repository.commits_in(Some(observed), commit) {
        Ok(commits) if !commits.is_empty() => format!(
            " ({} commit{} behind the match)",
            commits.len(),
            if commits.len() == 1 { "" } else { "s" }
        ),
        _ => String::new(),
    }
}

/// The JSON form of a stale entry (`docs/07-cli.md` §5).
fn stale_json(session: &Session, entry: &akr_core::freshness::Stale) -> Value {
    let record = session.ledger.get(&entry.id);
    let mut fields = vec![
        ("key", Value::string(entry.id.key.to_string())),
        ("rev", Value::integer(i64::from(entry.id.revision))),
        ("kind", Value::string(record.map_or("", |r| r.kind.name()))),
        (
            "state",
            Value::string(record.map_or("", |r| r.state.name())),
        ),
        ("cause", Value::string(entry.cause.name())),
    ];
    match &entry.cause {
        akr_core::freshness::StaleCause::Watch { glob, commit, path } => {
            fields.push(("glob", Value::string(glob.as_str())));
            fields.push(("matched_by", Value::string(commit.as_str())));
            fields.push(("matched_path", Value::string(path.clone())));
        }
        akr_core::freshness::StaleCause::ReviewAfter { date } => {
            fields.push(("review_after", Value::string(date.to_string())));
            fields.push((
                "days_overdue",
                Value::integer(days_between(*date, session.today)),
            ));
        }
    }
    if let Some(observed) = &entry.observed_at {
        fields.push(("observed_at", Value::string(observed.as_str())));
    }
    Value::object(fields)
}

/// The JSON form of an at-risk entry.
fn at_risk_json(session: &Session, entry: &akr_core::graph::AtRisk) -> Value {
    let record = session.ledger.get(&entry.id);
    Value::object(vec![
        ("key", Value::string(entry.id.key.to_string())),
        ("rev", Value::integer(i64::from(entry.id.revision))),
        ("kind", Value::string(record.map_or("", |r| r.kind.name()))),
        ("depth", Value::integer(entry.depth as i64)),
        ("via", Value::string(entry.via.name())),
        (
            "path",
            Value::array(
                entry
                    .path
                    .iter()
                    .map(|id| Value::string(id.key.to_string()))
                    .collect(),
            ),
        ),
    ])
}

// -------------------------------------------------------------------------------------
// lock, context
// -------------------------------------------------------------------------------------

fn lock(session: &mut Session, check_only: bool) -> Result<Output, EnvError> {
    session.attach_lock();
    let model = session.resolve();
    let computed = model.to_lock();

    if !check_only {
        let path = session.akr_dir.join("akr.lock");
        std::fs::write(&path, computed.render())
            .map_err(|e| EnvError::new("AKR-C042", format!("cannot write akr.lock: {e}")))?;
        return Ok(Output::text("wrote akr.lock\n"));
    }

    let Some(text) = &session.lock_text else {
        return Err(EnvError::new("AKR-R052", "akr.lock is missing").help("run `akr build`"));
    };
    let recorded = akr_core::lock::Lock::parse(text)
        .map_err(|e| EnvError::new("AKR-R052", format!("akr.lock does not parse: {e}")))?;
    let mut diagnostics =
        akr_core::lock::currency_diagnostics(&recorded, &computed, ".akr/akr.lock");
    diagnostics.extend(
        model
            .diagnostics
            .iter()
            .filter(|d| {
                d.code == akr_core::diagnostics::codes::R051
                    || d.code == akr_core::diagnostics::codes::R052
            })
            .cloned(),
    );
    session.spans.attach_all(&mut diagnostics);

    let exit = if diagnostics.is_empty() {
        Exit::Ok
    } else {
        Exit::Diagnostics
    };
    let out = if diagnostics.is_empty() {
        "akr.lock is current\n".to_owned()
    } else {
        report(&diagnostics, &session.sources, session.global.profile).0
    };
    Ok(Output::text(out)
        .with_result(Value::object(vec![(
            "mismatches",
            Value::integer(diagnostics.len() as i64),
        )]))
        .with_diagnostics(diagnostics, exit))
}

fn context(
    session: &Session,
    goal: &str,
    paths: &[akr_core::model::Glob],
    budget: Option<usize>,
) -> Result<Output, EnvError> {
    let model = session.resolve();
    let queue = session.review_queue();
    let freshness = session.freshness(&queue);

    let mut request = Request::new(goal);
    request.paths = paths.to_vec();
    request.budget = budget;

    let bundle = assemble(&model, &freshness, &request).map_err(|error| {
        let code = match error {
            akr_core::context::ContextError::GoalUnresolved(_) => "AKR-X001",
            akr_core::context::ContextError::GoalTerminal { .. } => "AKR-X002",
            akr_core::context::ContextError::GoalKind { .. } => "AKR-X003",
            akr_core::context::ContextError::BadPath { .. } => "AKR-X011",
            akr_core::context::ContextError::BudgetTooSmall { .. } => "AKR-X021",
        };
        EnvError::new(code, error.to_string())
    })?;

    let text = render_text(&bundle, &model, &freshness);
    let result = render_json(&bundle, &model);
    Ok(Output::text(text).with_result(result))
}

// -------------------------------------------------------------------------------------
// explain
// -------------------------------------------------------------------------------------

const CODES_LANG: &str = include_str!("../../../spec/diagnostics/codes-lang.md");
const CODES_RUNTIME: &str = include_str!("../../../spec/diagnostics/codes-runtime.md");

/// Prints a registry entry for a code, or a catalogue entry for a rule.
///
/// The registries are compiled in: `akr explain` needs no workspace, which is the point —
/// somebody reading a CI log should be able to look a code up without a checkout.
fn explain(subject: &str) -> Output {
    let wanted = subject.to_uppercase();
    for registry in [CODES_LANG, CODES_RUNTIME] {
        for line in registry.lines() {
            if line.starts_with('|') && line.contains(&format!("`{wanted}`")) {
                let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                let mut text = format!("{wanted}\n");
                for (label, cell) in ["title", "severity", "rule", "message", "cause"]
                    .iter()
                    .zip(cells.iter().skip(2))
                {
                    if !cell.is_empty() {
                        text.push_str(&format!("  {label:<10} {cell}\n"));
                    }
                }
                let registry_name = if std::ptr::eq(registry, CODES_LANG) {
                    "spec/diagnostics/codes-lang.md"
                } else {
                    "spec/diagnostics/codes-runtime.md"
                };
                text.push_str(&format!("  registry   {registry_name}\n"));
                return Output::text(text).with_result(Value::object(vec![
                    ("code", Value::string(wanted.clone())),
                    ("registry", Value::string(registry_name)),
                ]));
            }
        }
    }

    if let Some(rule) = akr_core::validate::RULES
        .iter()
        .find(|r| r.id.to_string() == wanted)
    {
        let text = format!(
            "{}\n  title      {}\n  code       {}\n  stage      {:?}\n  catalogue  docs/05-validation-rules.md\n",
            rule.id, rule.title, rule.code, rule.stage
        );
        return Output::text(text).with_result(Value::object(vec![
            ("rule", Value::string(rule.id.to_string())),
            ("code", Value::string(rule.code.as_str())),
        ]));
    }

    Output {
        text: format!(
            "error[AKR-C004]: {subject:?} is neither a registered diagnostic code nor a known rule\n"
        ),
        result: Value::Object(Vec::new()),
        diagnostics: Vec::new(),
        exit: Exit::Usage,
        commit: None,
        source_graph: String::new(),
    }
}

/// Turns a session's diagnostics into JSON, for the envelope.
#[must_use]
pub fn diagnostics_json(
    diagnostics: &[Diagnostic],
    sources: &akr_core::diagnostics::SourceMap,
) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|d| diagnostic_json(d, sources))
        .collect()
}
