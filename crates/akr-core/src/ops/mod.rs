//! The write operations: `propose`, `revise`, `supersede`, `complete`, `abandon`.
//!
//! `docs/07-cli.md` §4 defines one pipeline and every operation here performs all of it:
//!
//! ```text
//! 1. parse     the current ledger
//! 2. apply     the requested change, in memory
//! 3. validate  the RESULTING ledger, strictly
//! 4. format    every touched record canonically
//! 5. write     touched files, atomically
//! ```
//!
//! **Validation is of the result, not the change**, and **failure writes nothing**.
//! Nothing reaches the disk before step 5, so every refusal leaves the working tree
//! byte-identical. `tests/ops_atomicity.rs` hashes every source file before and after
//! each failing path and asserts exactly that.
//!
//! # The lock is not touched
//!
//! No operation here writes `akr.lock`. A lock records a *build*: a commit, a
//! source-graph hash over every file, and the head resolutions at that commit (D-014).
//! A write operation knows none of those, and inventing them would put a fabricated build
//! in the file whose whole job is to be checkable. Every [`Outcome`] therefore carries
//! [`Outcome::lock_stale`], and the caller runs `akr build` afterwards.
//!
//! One consequence is worth stating plainly, because it looks like a bug: an operation
//! that moves a sealed record along its lifecycle — `supersede` setting the old head to
//! `superseded`, `complete` setting `completed` — changes that record's canonical text
//! and therefore its content hash. Between the write and the next `akr build`, `akr
//! check` reports `AKR-R052` (lock stale). That is correct and expected; see the note in
//! the P6 report about `docs/04` §8.3.

mod stage;

use crate::diagnostics::codes::cli;
use crate::diagnostics::{Code, Diagnostic, Label, RuleId, Severity, Subject, codes};
use crate::model::{
    Class, Disposition, Kind, LogicalKey, Outcome as DispositionOutcome, Record, Reference,
    Relation, RevisionId, State,
};
use crate::syntax::{cst, emit};
use crate::validate;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub use stage::{LoadError, Staged};

// -------------------------------------------------------------------------------------
// results
// -------------------------------------------------------------------------------------

/// Which operation produced a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// A new key, or a new proposed revision.
    Propose,
    /// An edit to a proposed head, or a new revision of a sealed one.
    Revise,
    /// A new revision superseding the head.
    Supersede,
    /// A planning record moved to `completed`.
    Complete,
    /// A planning record moved to `abandoned`.
    Abandon,
}

impl Operation {
    /// The subcommand name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Propose => "propose",
            Self::Revise => "revise",
            Self::Supersede => "supersede",
            Self::Complete => "complete",
            Self::Abandon => "abandon",
        }
    }
}

/// What happened to one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    /// The revision did not exist before.
    Created,
    /// The revision existed and its body changed.
    Edited,
    /// Only the revision's `state` slot changed.
    StateChanged {
        /// The state it left.
        from: State,
        /// The state it entered.
        to: State,
    },
}

/// One revision touched by an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// Which revision.
    pub id: RevisionId,
    /// What happened to it.
    pub kind: ChangeKind,
    /// The file it lives in, relative to the `.akr` directory.
    pub file: PathBuf,
}

/// A successful write.
#[derive(Debug, Clone)]
pub struct Applied {
    /// Which operation.
    pub operation: Operation,
    /// Every revision touched, in key order.
    pub changes: Vec<Change>,
    /// Files written, relative to the `.akr` directory.
    pub files: Vec<PathBuf>,
    /// Diagnostics that did not block the write. Empty under the strict default.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether `akr.lock` is now stale and the caller should run `akr build`.
    pub lock_stale: bool,
}

/// An unfinished child a supersession or abandonment must account for (D-017).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnfinishedChild {
    /// The child's key.
    pub key: LogicalKey,
    /// The state it is in, which is why it counts as unfinished.
    pub state: State,
}

/// An acceptance check that is not satisfied (V-020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsatisfiedCheck {
    /// The check identifier.
    pub id: String,
    /// Why it is not satisfied, in one clause.
    pub reason: String,
}

/// A refused write. Nothing was written.
#[derive(Debug, Clone)]
pub struct Refused {
    /// Which operation.
    pub operation: Operation,
    /// The most specific code for the refusal.
    pub code: Code,
    /// The one-line reason.
    pub message: String,
    /// Every diagnostic the resulting ledger produced.
    pub diagnostics: Vec<Diagnostic>,
    /// Children needing a disposition, for `supersede` and `abandon`.
    pub unfinished_children: Vec<UnfinishedChild>,
    /// Checks blocking a `complete`.
    pub unsatisfied_checks: Vec<UnsatisfiedCheck>,
    /// A suggested fix, ready to render under `help:`.
    pub help: Option<String>,
}

impl Refused {
    fn new(operation: Operation, code: Code, message: impl Into<String>) -> Self {
        Self {
            operation,
            code,
            message: message.into(),
            diagnostics: Vec::new(),
            unfinished_children: Vec::new(),
            unsatisfied_checks: Vec::new(),
            help: None,
        }
    }

    fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The refusal as a diagnostic, for callers that render one stream.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            code: self.code,
            severity: Severity::Error,
            rule: None,
            message: self.message.clone(),
            primary: Label::new(Subject::Ledger),
            notes: Vec::new(),
            help: self.help.clone(),
        }
    }
}

/// The outcome of a write operation.
pub type WriteResult = Result<Applied, Refused>;

/// Backwards-compatible alias for the success type.
pub type Outcome = Applied;

// -------------------------------------------------------------------------------------
// context and requests
// -------------------------------------------------------------------------------------

/// Where to write, and under which diagnostic profile.
#[derive(Debug, Clone)]
pub struct WriteContext {
    /// The `.akr` directory.
    pub akr_dir: PathBuf,
    /// Whether warnings count as errors. `true` is the default profile (D-013).
    pub strict: bool,
    /// Author recorded on records this operation creates.
    pub author: Option<String>,
}

impl WriteContext {
    /// A context for the given `.akr` directory, strict, with no author.
    #[must_use]
    pub fn new(akr_dir: impl Into<PathBuf>) -> Self {
        Self {
            akr_dir: akr_dir.into(),
            strict: true,
            author: None,
        }
    }

    /// Sets the author recorded on new records.
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }
}

/// How `revise` should treat the head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviseMode {
    /// Edit a `proposed` head in place; create a new revision from a sealed one.
    #[default]
    Auto,
    /// Edit the head in place. A sealed head is `AKR-C032`.
    InPlace,
    /// Always create revision n+1.
    NewRevision,
}

/// An edit to apply to a record.
#[derive(Debug, Clone, Default)]
pub struct Edits {
    /// Replace the title.
    pub title: Option<String>,
    /// Move to a state. An illegal transition fails validation with `AKR-T011`.
    pub state: Option<State>,
    /// Replace the whole record. Everything but the identifier is taken from it.
    pub replace_with: Option<Box<Record>>,
}

/// A disposition supplied on the command line, before its target is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispositionRequest {
    /// The child being dispositioned.
    pub child: LogicalKey,
    /// What happened to it.
    pub outcome: DispositionOutcome,
    /// Where it went.
    pub into: Option<LogicalKey>,
    /// Why.
    pub note: Option<String>,
}

// -------------------------------------------------------------------------------------
// the operations
// -------------------------------------------------------------------------------------

/// Creates revision 1 of a new key, in the initial state of its class.
///
/// The record is written to the conventional file for its namespace and kind group,
/// creating it if needed. An existing key is refused: use [`revise`].
///
/// # Errors
/// Refuses if the key exists, or if the resulting ledger does not validate.
pub fn propose(
    context: &WriteContext,
    key: &LogicalKey,
    kind: Kind,
    title: &str,
    template: Option<Record>,
) -> WriteResult {
    let mut staged = load(context, Operation::Propose)?;
    if !staged.ledger.revisions_of(key).is_empty() {
        return Err(Refused::new(
            Operation::Propose,
            codes::L041,
            format!("{key} already exists"),
        )
        .with_help(format!("use `akr revise {key}` to change it")));
    }

    let id = RevisionId::new(key.clone(), 1);
    let mut record = template.unwrap_or_else(|| blank(&id, kind));
    record.id = id.clone();
    record.kind = kind;
    if !title.is_empty() {
        record.title = title.to_owned();
    }
    if record.title.is_empty() {
        record.title = key.to_string();
    }
    if record.author.is_none() {
        record.author.clone_from(&context.author);
    }
    if !kind.class().states().contains(&record.state) {
        record.state = kind.class().initial()[0];
    }

    let file = conventional_file(key, kind);
    apply(
        context,
        staged_mut(&mut staged),
        Operation::Propose,
        &file,
        &record,
        ChangeKind::Created,
    )
}

/// Edits the head of a key, or creates revision n+1 of it.
///
/// A `proposed` head is edited in place: D-015 makes proposed revisions editable, and
/// creating revision 2 of a proposal nobody accepted would be noise. A sealed head
/// produces revision n+1 with a `supersedes` edge, which is the only way to change a
/// settled record.
///
/// # Errors
/// Refuses on an unknown key, an in-place edit of a sealed head (`AKR-C032`), or a
/// resulting ledger that does not validate.
pub fn revise(
    context: &WriteContext,
    key: &LogicalKey,
    mode: ReviseMode,
    edits: &Edits,
) -> WriteResult {
    let mut staged = load(context, Operation::Revise)?;
    let head = head_of(&staged, key, Operation::Revise)?.clone();
    let file = file_of(&staged, &head.id, Operation::Revise)?;

    let in_place = match mode {
        ReviseMode::InPlace => {
            if head.is_sealed() {
                return Err(Refused::new(
                    Operation::Revise,
                    cli::C032,
                    format!(
                        "{} is sealed ({}); create a new revision",
                        head.id, head.state
                    ),
                )
                .with_help(format!("run `akr revise {key}` without --in-place")));
            }
            true
        }
        ReviseMode::NewRevision => false,
        ReviseMode::Auto => !head.is_sealed(),
    };

    if !in_place {
        // A new revision must retire the old one in the same write. Leaving both live
        // would be two live heads (V-012), and `docs/07` §4 refuses to write a ledger
        // that does not validate — so the two cannot be separated. See the P6 report on
        // `docs/04` §2.1, which describes the unretired intermediate state.
        let mut record = edited(&head, edits);
        record.id = RevisionId::new(key.clone(), head.id.revision + 1);
        record.state = head.kind.class().initial()[0];
        if let Some(state) = edits.state {
            record.state = state;
        }
        return retire_and_replace(context, &mut staged, Operation::Revise, &head, record, &[]);
    }

    let mut record = edited(&head, edits);
    record.id = head.id.clone();
    let change = if edits.state.is_some() && edits.title.is_none() && edits.replace_with.is_none() {
        ChangeKind::StateChanged {
            from: head.state,
            to: record.state,
        }
    } else {
        ChangeKind::Edited
    };
    apply(
        context,
        staged_mut(&mut staged),
        Operation::Revise,
        &file,
        &record,
        change,
    )
}

/// The shared core of `supersede` and of `revise` on a sealed head.
///
/// Both create revision n+1, point it at n with `supersedes`, and move n to `superseded`
/// in the same write. For planning records both demand a disposition for every unfinished
/// child (D-017): the requirement belongs to the act of replacing a plan, not to the name
/// of the command that does it.
fn retire_and_replace(
    context: &WriteContext,
    staged: &mut Staged,
    operation: Operation,
    head: &Record,
    mut record: Record,
    dispositions: &[DispositionRequest],
) -> WriteResult {
    let key = head.id.key.clone();
    let file = file_of(staged, &head.id, operation)?;

    if head.kind.class() == Class::Planning {
        let children = unfinished_children(staged, &head.id);
        let supplied: BTreeSet<&LogicalKey> = dispositions.iter().map(|d| &d.child).collect();
        let missing: Vec<UnfinishedChild> = children
            .into_iter()
            .filter(|c| !supplied.contains(&c.key))
            .collect();
        if !missing.is_empty() {
            let help = missing
                .iter()
                .map(|c| format!("  --disposition {}=carried_forward:<target>", c.key))
                .collect::<Vec<_>>()
                .join("\n");
            let mut refusal = Refused::new(
                operation,
                codes::R014,
                format!(
                    "{} unfinished {} of {}",
                    missing.len(),
                    if missing.len() == 1 {
                        "child"
                    } else {
                        "children"
                    },
                    head.id
                ),
            )
            .with_help(format!("rerun with, for example,\n{help}"));
            refusal.unfinished_children = missing;
            return Err(refusal);
        }
    }

    record.relations.insert(
        Relation::Supersedes,
        vec![Reference::pinned(key, head.id.revision)],
    );
    if !dispositions.is_empty() {
        record.dispositions = dispositions.iter().map(build_disposition).collect();
        record.dispositions.sort_by(|a, b| a.target.cmp(&b.target));
    }
    if record.author.is_none() {
        record.author.clone_from(&context.author);
    }

    let mut retired = head.clone();
    retired.state = State::Superseded;

    apply_many(
        context,
        staged,
        operation,
        &[
            (
                file.clone(),
                retired,
                ChangeKind::StateChanged {
                    from: head.state,
                    to: State::Superseded,
                },
            ),
            (file, record, ChangeKind::Created),
        ],
    )
}

/// Creates a revision superseding the head, moving the old head to `superseded`.
///
/// For planning records, every unfinished `part_of` child of the old head must have a
/// disposition (D-017). Missing ones are listed in the refusal, which is the whole point
/// of the command: it is the moment the author knows the answer and the only moment
/// anyone will ask.
///
/// # Errors
/// Refuses on an unknown key, a missing disposition, or a resulting ledger that does not
/// validate.
pub fn supersede(
    context: &WriteContext,
    key: &LogicalKey,
    dispositions: &[DispositionRequest],
) -> WriteResult {
    let mut staged = load(context, Operation::Supersede)?;
    let head = head_of(&staged, key, Operation::Supersede)?.clone();

    let mut record = head.clone();
    record.id = RevisionId::new(key.clone(), head.id.revision + 1);
    record.state = head.kind.class().initial()[0];
    record.dispositions.clear();

    retire_and_replace(
        context,
        &mut staged,
        Operation::Supersede,
        &head,
        record,
        dispositions,
    )
}

/// Moves a `milestone` or `work` record to `completed`.
///
/// Every acceptance check must be satisfied (V-020). An unsatisfied one is refused with
/// the check named, and nothing is written.
///
/// # Errors
/// Refuses on an unknown key, a non-planning kind, an unsatisfied check, or a resulting
/// ledger that does not validate.
pub fn complete(
    context: &WriteContext,
    key: &LogicalKey,
    check_evidence: &[(String, Reference)],
) -> WriteResult {
    let mut staged = load(context, Operation::Complete)?;
    let head = head_of(&staged, key, Operation::Complete)?.clone();
    let file = file_of(&staged, &head.id, Operation::Complete)?;

    if !matches!(head.kind, Kind::Milestone | Kind::Work) {
        return Err(Refused::new(
            Operation::Complete,
            codes::T011,
            format!(
                "{} is a {}; only milestone and work records complete",
                head.id, head.kind
            ),
        ));
    }

    let mut record = head.clone();
    if let Some(acceptance) = &mut record.acceptance {
        for (id, reference) in check_evidence {
            if let Some(check) = acceptance.checks.iter_mut().find(|c| c.id.as_str() == id) {
                if !check.verified_by.contains(reference) {
                    check.verified_by.push(reference.clone());
                }
            }
        }
    }
    record.state = State::Completed;

    // Ask V-020 directly, so the refusal can name the checks rather than echo a
    // diagnostic stream. The pipeline validates the whole result again below.
    let probe = {
        let mut probe = crate::model::Ledger::new(staged.ledger.project.clone());
        let mut records: Vec<Record> = staged.ledger.records().to_vec();
        for existing in &mut records {
            if existing.id == record.id {
                existing.clone_from(&record);
            }
        }
        probe.extend(records);
        validate::v020_acceptance_satisfied(&probe)
    };
    if !probe.is_empty() {
        let unsatisfied: Vec<UnsatisfiedCheck> = probe
            .iter()
            .map(|d| UnsatisfiedCheck {
                id: check_name(&d.message).unwrap_or_default(),
                reason: d.message.clone(),
            })
            .collect();
        let mut refusal = Refused::new(
            Operation::Complete,
            codes::R022,
            format!(
                "{} has {} unsatisfied acceptance check(s)",
                head.id,
                unsatisfied.len()
            ),
        )
        .with_help("record the evidence, then rerun with --check <id>=<evidence-ref>");
        refusal.diagnostics = probe;
        refusal.unsatisfied_checks = unsatisfied;
        return Err(refusal);
    }

    apply(
        context,
        staged_mut(&mut staged),
        Operation::Complete,
        &file,
        &record,
        ChangeKind::StateChanged {
            from: head.state,
            to: State::Completed,
        },
    )
}

/// Moves a planning record to `abandoned`, demanding a disposition for every unfinished
/// child.
///
/// The reason is recorded as a leading comment on the record. There is no `note` slot in
/// the vocabulary, and comments are excluded from the seal hash (D-015), so this is the
/// one place a reason can land without changing what the record says. See the P6 report:
/// `docs/07` §6 says the reason "lands in a `note`", which does not exist.
///
/// # Errors
/// Refuses on an unknown key, a non-planning kind, a missing disposition, or a resulting
/// ledger that does not validate.
pub fn abandon(
    context: &WriteContext,
    key: &LogicalKey,
    reason: &str,
    dispositions: &[DispositionRequest],
) -> WriteResult {
    let mut staged = load(context, Operation::Abandon)?;
    let head = head_of(&staged, key, Operation::Abandon)?.clone();
    let file = file_of(&staged, &head.id, Operation::Abandon)?;

    if head.kind.class() != Class::Planning {
        return Err(Refused::new(
            Operation::Abandon,
            codes::T011,
            format!(
                "{} is a {}; only planning records are abandoned",
                head.id, head.kind
            ),
        ));
    }
    if reason.trim().is_empty() {
        return Err(Refused::new(
            Operation::Abandon,
            cli::C031,
            "a reason is required".to_owned(),
        )
        .with_help("rerun with --reason \"<why>\""));
    }

    let children = unfinished_children(&staged, &head.id);
    let supplied: BTreeSet<&LogicalKey> = dispositions.iter().map(|d| &d.child).collect();
    let missing: Vec<UnfinishedChild> = children
        .into_iter()
        .filter(|c| !supplied.contains(&c.key))
        .collect();
    if !missing.is_empty() {
        let help = missing
            .iter()
            .map(|c| format!("  --disposition {}=intentionally_dropped", c.key))
            .collect::<Vec<_>>()
            .join("\n");
        let mut refusal = Refused::new(
            Operation::Abandon,
            codes::R014,
            format!("{} unfinished children of {}", missing.len(), head.id),
        )
        .with_help(format!(
            "abandoning a plan silently is what D-017 exists to prevent; rerun with\n{help}"
        ));
        refusal.unfinished_children = missing;
        return Err(refusal);
    }

    let mut record = head.clone();
    record.state = State::Abandoned;
    if !dispositions.is_empty() {
        record.dispositions = dispositions.iter().map(build_disposition).collect();
        record.dispositions.sort_by(|a, b| a.target.cmp(&b.target));
    }

    apply_with_comment(
        context,
        &mut staged,
        Operation::Abandon,
        &file,
        &record,
        ChangeKind::StateChanged {
            from: head.state,
            to: State::Abandoned,
        },
        Some(format!("abandoned: {}", reason.trim())),
    )
}

// -------------------------------------------------------------------------------------
// pipeline
// -------------------------------------------------------------------------------------

fn load(context: &WriteContext, operation: Operation) -> Result<Staged, Refused> {
    Staged::load(&context.akr_dir)
        .map_err(|error| Refused::new(operation, cli::C012, error.to_string()))
}

fn staged_mut(staged: &mut Staged) -> &mut Staged {
    staged
}

fn head_of<'a>(
    staged: &'a Staged,
    key: &LogicalKey,
    operation: Operation,
) -> Result<&'a Record, Refused> {
    staged.ledger.head(key).map_err(|error| {
        Refused::new(operation, codes::L001, error.to_string())
            .with_help(format!("`akr propose {key} --kind <kind>` creates it"))
    })
}

fn file_of(staged: &Staged, id: &RevisionId, operation: Operation) -> Result<PathBuf, Refused> {
    staged
        .ledger
        .get(id)
        .and_then(|r| r.file.as_ref())
        .map(PathBuf::from)
        .ok_or_else(|| Refused::new(operation, codes::L006, format!("{id} has no source file")))
}

fn apply(
    context: &WriteContext,
    staged: &mut Staged,
    operation: Operation,
    file: &Path,
    record: &Record,
    change: ChangeKind,
) -> WriteResult {
    apply_many(
        context,
        staged,
        operation,
        &[(file.to_path_buf(), record.clone(), change)],
    )
}

fn apply_with_comment(
    context: &WriteContext,
    staged: &mut Staged,
    operation: Operation,
    file: &Path,
    record: &Record,
    change: ChangeKind,
    comment: Option<String>,
) -> WriteResult {
    apply_inner(
        context,
        staged,
        operation,
        &[(file.to_path_buf(), record.clone(), change)],
        comment,
    )
}

fn apply_many(
    context: &WriteContext,
    staged: &mut Staged,
    operation: Operation,
    edits: &[(PathBuf, Record, ChangeKind)],
) -> WriteResult {
    apply_inner(context, staged, operation, edits, None)
}

/// Steps 2 through 5 of `docs/07` §4.
fn apply_inner(
    context: &WriteContext,
    staged: &mut Staged,
    operation: Operation,
    edits: &[(PathBuf, Record, ChangeKind)],
    comment: Option<String>,
) -> WriteResult {
    let project = staged.project.clone();
    let mut touched: Vec<PathBuf> = Vec::new();
    let mut changes: Vec<Change> = Vec::new();

    // Step 2: apply in memory.
    for (file, record, change) in edits {
        let Some(node) = emit::record_node(record, &project) else {
            return Err(Refused::new(
                operation,
                cli::C031,
                format!("{} could not be rendered as canonical source", record.id),
            ));
        };
        let mut tree = staged
            .trees
            .get(file)
            .cloned()
            .unwrap_or_else(|| empty_file(&project));
        splice(&mut tree, node, comment.as_deref());
        staged.set_tree(file, &tree);
        if !touched.contains(file) {
            touched.push(file.clone());
        }
        changes.push(Change {
            id: record.id.clone(),
            kind: change.clone(),
            file: file.clone(),
        });
    }

    // Step 3: validate the result.
    staged.reparse();
    let mut diagnostics = staged.diagnostics.clone();
    diagnostics.extend(validate::validate_all(&staged.ledger));
    let errors = Staged::errors(&diagnostics, context.strict);
    if !errors.is_empty() {
        let mut refusal = Refused::new(
            operation,
            cli::C031,
            format!(
                "write aborted: the resulting ledger did not validate ({} diagnostics); \
                 nothing was written",
                errors.len()
            ),
        );
        refusal.diagnostics = errors;
        return Err(refusal);
    }

    // Steps 4 and 5: the text is already canonical; write it.
    touched.sort();
    staged
        .commit(&touched)
        .map_err(|error| Refused::new(operation, cli::C031, format!("write failed: {error}")))?;

    changes.sort_by(|a, b| a.id.cmp(&b.id));
    let lock_stale = changes
        .iter()
        .any(|c| !matches!(c.kind, ChangeKind::Edited));
    Ok(Applied {
        operation,
        changes,
        files: touched,
        diagnostics: diagnostics
            .into_iter()
            .filter(|d| d.severity == Severity::Warning)
            .collect(),
        lock_stale,
    })
}

/// Replaces a record in a tree, or inserts it, preserving comments where it can.
///
/// Record-level trivia and slot-level comments whose slot survives the edit are carried
/// over. Comments on a slot the edit removed are lost, which is the honest cost of
/// regenerating a record from the model.
fn splice(tree: &mut cst::File, mut node: cst::Record, comment: Option<&str>) {
    let existing = tree
        .items
        .iter()
        .position(|item| matches!(item, cst::Item::Record(r) if r.key == node.key && r.revision == node.revision));

    if let Some(at) = existing
        && let cst::Item::Record(old) = &tree.items[at]
    {
        node.trivia = old.trivia.clone();
        node.inner_trailing = old.inner_trailing.clone();
        for item in &mut node.body {
            let name = item.name().to_owned();
            let carried = old
                .body
                .iter()
                .find(|o| o.name() == name)
                .map(|o| o.trivia().clone());
            if let Some(trivia) = carried {
                match item {
                    cst::BodyItem::Slot(slot) => slot.trivia = trivia,
                    cst::BodyItem::Block(block) => block.trivia = trivia,
                }
            }
        }
    }

    if let Some(text) = comment {
        let already = node.trivia.leading.iter().any(|c| c.text == text);
        if !already {
            node.trivia.leading.push(cst::Comment {
                text: text.to_owned(),
                span: node.span,
                blank_before: false,
            });
        }
    }

    match existing {
        Some(at) => tree.items[at] = cst::Item::Record(node),
        None => tree.items.push(cst::Item::Record(node)),
    }
}

fn empty_file(project: &str) -> cst::File {
    use crate::diagnostics::{FileId, Span};
    cst::File {
        leading: Vec::new(),
        keyword: "akr".to_owned(),
        version: "0.1".to_owned(),
        blank_before_header: false,
        project: project.to_owned(),
        items: Vec::new(),
        trailing: Vec::new(),
        span: Span {
            file: FileId(0),
            start: 0,
            end: 0,
        },
    }
}

// -------------------------------------------------------------------------------------
// helpers
// -------------------------------------------------------------------------------------

fn blank(id: &RevisionId, kind: Kind) -> Record {
    Record {
        id: id.clone(),
        kind,
        title: String::new(),
        state: kind.class().initial()[0],
        scope: if kind.class().scope_required() {
            vec![crate::model::ScopeTerm::All]
        } else {
            Vec::new()
        },
        topic: None,
        content: std::collections::BTreeMap::new(),
        claims: Vec::new(),
        retired_claims: Vec::new(),
        acceptance: None,
        dispositions: Vec::new(),
        relations: std::collections::BTreeMap::new(),
        acknowledged: false,
        author: None,
        created_at: None,
        sources: Vec::new(),
        file: None,
    }
}

fn edited(head: &Record, edits: &Edits) -> Record {
    let mut record = edits
        .replace_with
        .clone()
        .map_or_else(|| head.clone(), |r| *r);
    record.id = head.id.clone();
    record.kind = head.kind;
    record.file.clone_from(&head.file);
    if let Some(title) = &edits.title {
        record.title.clone_from(title);
    }
    if let Some(state) = edits.state {
        record.state = state;
    }
    record
}

fn build_disposition(request: &DispositionRequest) -> Disposition {
    Disposition {
        target: Reference::head(request.child.clone()),
        outcome: request.outcome,
        into: request.into.clone().map(Reference::head),
        note: request.note.clone(),
    }
}

/// Live planning records whose `part_of` pins the given revision (D-017, V-017).
fn unfinished_children(staged: &Staged, parent: &RevisionId) -> Vec<UnfinishedChild> {
    let mut children: Vec<UnfinishedChild> = staged
        .ledger
        .records()
        .iter()
        .filter(|candidate| {
            candidate.kind.class() == Class::Planning
                && candidate.is_live()
                && candidate.targets(Relation::PartOf).iter().any(|t| {
                    t.key == parent.key && t.revision.is_some_and(|r| r == parent.revision)
                })
        })
        .map(|c| UnfinishedChild {
            key: c.id.key.clone(),
            state: c.state,
        })
        .collect();
    children.sort_by(|a, b| a.key.cmp(&b.key));
    children.dedup_by(|a, b| a.key == b.key);
    children
}

/// The conventional file for a key's namespace and kind group (D-018).
#[must_use]
pub fn conventional_file(key: &LogicalKey, kind: Kind) -> PathBuf {
    let group = match kind {
        Kind::Term => "terms",
        Kind::Requirement => "requirements",
        Kind::Policy => "policies",
        Kind::Constraint => "constraints",
        Kind::Decision => "decisions",
        Kind::Observation => "observations",
        Kind::Evidence => "evidence",
        Kind::Assessment => "assessments",
        Kind::Milestone => "milestones",
        Kind::Work => "work",
        Kind::Track => "tracks",
        Kind::Question => "questions",
    };
    PathBuf::from("records")
        .join(key.namespace().as_str())
        .join(format!("{group}.akr"))
}

/// Pulls a check identifier out of a V-020 message, for the structured refusal.
fn check_name(message: &str) -> Option<String> {
    let at = message.find("check `")? + "check `".len();
    let rest = &message[at..];
    rest.find('`').map(|end| rest[..end].to_owned())
}

/// The rule a refusal corresponds to, where there is one.
#[must_use]
pub fn refusal_rule(code: Code) -> Option<RuleId> {
    validate::RULES
        .iter()
        .find(|r| r.code == code)
        .map(|r| r.id)
}
