//! `akr import` (`docs/07-cli.md` §6, `docs/12-migration.md`).
//!
//! The command is three phases, and only the last one writes:
//!
//! 1. **Read and extract** — `akr_core::import::extract`, the deterministic floor: one
//!    draft claim per heading, verbatim excerpts. `AKR-M001`/`AKR-M002` end it here.
//! 2. **Plan** — keys are proposed from `--namespace` (or the document's location) and
//!    checked against the ledger (`AKR-M012`, `AKR-M013`); the document's own dead links
//!    become `AKR-M022` warnings; an empty extraction is `AKR-M011`.
//! 3. **Write** — one `akr_core::ops::import` call: every drafted record, the tracking
//!    record and its checks in a single validated, atomic write.
//!
//! `--lenient` is the one place warnings are downgraded (D-013), and it changes the exit
//! status only: the warning list is identical with and without it. Without it, any
//! warning is `AKR-M041` and phase 3 never runs — that holds for `--dry-run` too, so the
//! recommended first invocation shows exactly what the writing one will decide.

use crate::args::Profile;
use crate::commands::Output;
use crate::session::{EnvError, Exit, Session};
use akr_core::diagnostics::{Diagnostic, Label, Severity, Subject, codes::migration};
use akr_core::import::{DraftClaim, Extraction, Format, extract, slug_of};
use akr_core::json::Value;
use akr_core::model::{LogicalKey, Segment};
use akr_core::ops::{ImportRequest, ImportedRecord};
use std::path::{Path, PathBuf};

/// Runs `akr import`.
///
/// # Errors
/// [`EnvError`] only for a malformed `--tracking` key. Everything about the document or
/// the ledger — missing source, bad format, collisions — is a diagnostic with exit 1:
/// the invocation was fine, the tool looked, and this is what it found.
pub fn run(
    session: &Session,
    path: &Path,
    namespace: Option<&str>,
    tracking: Option<&str>,
    dry_run: bool,
) -> Result<Output, EnvError> {
    let document = repo_relative(session, path);
    let strict = session.global.profile == Profile::Strict;

    // Phase 1 — read.
    let absolute = session.root.join(&document);
    if !absolute.exists() {
        return Ok(diagnostics_output(
            session,
            vec![error(
                migration::M001,
                format!("import source {document} does not exist"),
            )],
        ));
    }
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(format) = Format::from_extension(&extension) else {
        return Ok(diagnostics_output(
            session,
            vec![
                error(
                    migration::M002,
                    format!("{document}: {extension:?} is not an importable format"),
                )
                .with_help("0.1 imports Markdown (.md) and plain text (.txt) only"),
            ],
        ));
    };
    let text = std::fs::read_to_string(&absolute)
        .map_err(|e| EnvError::new("AKR-C011", format!("cannot read {document}: {e}")))?;
    let extraction = extract(&text, format);

    // Phase 2 — plan.
    let namespace = namespace.map_or_else(|| namespace_of(&document), str::to_owned);
    let mut warnings = link_warnings(session, &document, &extraction);
    if extraction.claims.is_empty() {
        warnings.push(warning(
            migration::M011,
            format!("{document}: no durable claim extracted"),
        ));
    }

    let mut errors = Vec::new();
    let declared =
        Segment::new(&namespace).is_ok_and(|ns| session.ledger.project.namespaces.contains(&ns));
    if !declared {
        errors.push(
            error(
                migration::M013,
                format!("namespace {namespace} is not declared in project.akr"),
            )
            .with_help("declare it in project.akr, or rerun with --namespace <ns>"),
        );
    }

    let tracking = match tracking {
        Some(text) => crate::write::parse_key(text)?,
        None => {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let slug = some_slug(&slug_of(&stem), "document");
            parse_or_die(&format!("{namespace}.work.{slug}-import"))
        }
    };

    let mut records = Vec::new();
    if declared {
        for claim in &extraction.claims {
            let key = parse_or_die(&format!("{namespace}.{}.{}", claim.kind.name(), claim.slug));
            if !session.ledger.revisions_of(&key).is_empty() {
                errors.push(
                    error(
                        migration::M012,
                        format!(
                            "{key} already exists; imported records may not overwrite \
                             ledger records"
                        ),
                    )
                    .with_help(&format!(
                        "add the source block to the existing record instead: \
                         `akr revise {key}`"
                    )),
                );
                continue;
            }
            records.push(ImportedRecord {
                key,
                kind: claim.kind,
                title: claim.title.clone(),
                body: claim.excerpt.clone(),
                excerpt: claim.excerpt.clone(),
                check_id: check_id_of(claim),
            });
        }
    }

    if !errors.is_empty() {
        errors.splice(0..0, warnings);
        return Ok(diagnostics_output(session, errors));
    }

    let mut text_out = plan_text(&document, &extraction, &records, &tracking, dry_run);

    // The strict gate (AKR-M041): warnings end the import before anything is written,
    // dry or not, so both invocations decide identically.
    let gated = strict && !warnings.is_empty();
    if gated {
        warnings.push(error(
            migration::M041,
            format!(
                "import produced {} warning{}; rerun with --lenient after reviewing them",
                warnings.len(),
                if warnings.len() == 1 { "" } else { "s" }
            ),
        ));
    }

    if dry_run || gated || records.is_empty() {
        for diagnostic in &warnings {
            text_out.push_str(&akr_core::diagnostics::render(diagnostic, &session.sources));
        }
        let reason = if dry_run {
            "--dry-run"
        } else if gated {
            "AKR-M041"
        } else {
            "no durable claims"
        };
        text_out.push_str(&format!("nothing written ({reason})\n"));
        let exit = if gated { Exit::Diagnostics } else { Exit::Ok };
        return Ok(
            Output::plain(text_out, plan_json(&records, &tracking, false))
                .with_diagnostics(warnings, exit),
        );
    }

    // Phase 3 — write, as one operation.
    let request = ImportRequest {
        document: document.clone(),
        records,
        tracking,
    };
    let mut output = crate::write::render(
        session,
        akr_core::ops::import(&crate::write::context_of(session), &request),
    )?;
    output.text = {
        let mut combined = text_out;
        for diagnostic in &warnings {
            combined.push_str(&akr_core::diagnostics::render(diagnostic, &session.sources));
        }
        combined.push_str(&output.text);
        combined
    };
    warnings.extend(std::mem::take(&mut output.diagnostics));
    output.diagnostics = warnings;
    Ok(output)
}

// -------------------------------------------------------------------------------------
// planning helpers
// -------------------------------------------------------------------------------------

/// The transcript form of `docs/12` §7: what the import proposes, before any warning.
fn plan_text(
    document: &str,
    extraction: &Extraction,
    records: &[ImportedRecord],
    tracking: &LogicalKey,
    dry_run: bool,
) -> String {
    let mut out = format!(
        "{document} — {} durable claim{}, {} paragraph{} skipped\n\n",
        extraction.claims.len(),
        if extraction.claims.len() == 1 {
            ""
        } else {
            "s"
        },
        extraction.paragraphs_skipped,
        if extraction.paragraphs_skipped == 1 {
            ""
        } else {
            "s"
        },
    );
    let verb = if dry_run {
        "would propose"
    } else {
        "proposing    "
    };
    let width = records
        .iter()
        .map(|r| r.key.to_string().len())
        .max()
        .unwrap_or(0);
    for (index, record) in records.iter().enumerate() {
        out.push_str(&format!(
            "  {verb}  {:<width$}   {:<11} (claim {})\n",
            record.key.to_string(),
            record.kind.name(),
            index + 1,
        ));
    }
    if !records.is_empty() {
        let verb = if dry_run {
            "would add    "
        } else {
            "adding       "
        };
        out.push_str(&format!(
            "  {verb}  {} check{} to @{tracking}\n\n",
            records.len(),
            if records.len() == 1 { "" } else { "s" },
        ));
    }
    out
}

fn plan_json(records: &[ImportedRecord], tracking: &LogicalKey, written: bool) -> Value {
    Value::object(vec![
        ("operation", Value::string("import")),
        (
            "proposals",
            Value::array(
                records
                    .iter()
                    .map(|r| {
                        Value::object(vec![
                            ("key", Value::string(r.key.to_string())),
                            ("kind", Value::string(r.kind.name())),
                            ("title", Value::string(r.title.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("tracking", Value::string(tracking.to_string())),
        ("written", Value::bool(written)),
    ])
}

/// `AKR-M022` for every relative link in the document that resolves to nothing.
fn link_warnings(session: &Session, document: &str, extraction: &Extraction) -> Vec<Diagnostic> {
    let base = Path::new(document).parent().unwrap_or(Path::new(""));
    let head = session
        .commit
        .as_ref()
        .map_or_else(|| "HEAD".to_owned(), |c| c.as_str()[..8].to_owned());
    let mut out = Vec::new();
    for link in &extraction.links {
        let target = normalise(&base.join(&link.target));
        if !session.root.join(&target).exists() {
            out.push(warning(
                migration::M022,
                format!(
                    "source path \"{}\" does not exist at {head} ({document}:{}:{})",
                    target.display(),
                    link.line,
                    link.column
                ),
            ));
        }
    }
    out
}

/// Lexical `..`/`.` resolution, for paths that may not exist.
fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The document's path relative to the workspace root, kept as given when outside it.
fn repo_relative(session: &Session, path: &Path) -> String {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let root = session
        .root
        .canonicalize()
        .unwrap_or_else(|_| session.root.clone());
    joined
        .canonicalize()
        .ok()
        .and_then(|p| p.strip_prefix(&root).map(Path::to_path_buf).ok())
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

/// The default namespace: the document's first path segment (`docs/12` §3).
fn namespace_of(document: &str) -> String {
    let first = Path::new(document)
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    some_slug(&slug_of(&first), "docs")
}

fn check_id_of(claim: &DraftClaim) -> Segment {
    Segment::new(&format!("{}-claim", claim.slug))
        .unwrap_or_else(|_| Segment::new("imported-claim").expect("a valid literal segment"))
}

fn some_slug(slug: &str, fallback: &str) -> String {
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug.to_owned()
    }
}

/// A key built from validated parts; the parts make failure unreachable.
fn parse_or_die(text: &str) -> LogicalKey {
    LogicalKey::parse(text).expect("namespace, kind name and slug are each valid segments")
}

// -------------------------------------------------------------------------------------
// diagnostics
// -------------------------------------------------------------------------------------

fn error(code: akr_core::diagnostics::Code, message: String) -> Diagnostic {
    diagnostic(code, Severity::Error, message)
}

fn warning(code: akr_core::diagnostics::Code, message: String) -> Diagnostic {
    diagnostic(code, Severity::Warning, message)
}

fn diagnostic(
    code: akr_core::diagnostics::Code,
    severity: Severity,
    message: String,
) -> Diagnostic {
    Diagnostic {
        code,
        severity,
        rule: None,
        message,
        primary: Label::new(Subject::Ledger),
        notes: Vec::new(),
        help: None,
    }
}

trait WithHelp {
    fn with_help(self, help: &str) -> Self;
}

impl WithHelp for Diagnostic {
    fn with_help(mut self, help: &str) -> Self {
        self.help = Some(help.to_owned());
        self
    }
}

/// Errors rendered and exit 1: the tool did its job, and this is what it found.
fn diagnostics_output(session: &Session, diagnostics: Vec<Diagnostic>) -> Output {
    let mut text = String::new();
    for diagnostic in &diagnostics {
        text.push_str(&akr_core::diagnostics::render(diagnostic, &session.sources));
    }
    Output::plain(text, Value::Object(Vec::new())).with_diagnostics(diagnostics, Exit::Diagnostics)
}
