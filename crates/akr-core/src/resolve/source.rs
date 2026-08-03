//! Loading a workspace from disk: files in, ledger and build inputs out.
//!
//! This is the front half of `akr build` steps 4–7 (`docs/06-compiler-pipeline.md` §13):
//! discover the sources by a sorted walk, hash their raw bytes, parse, lower, and produce
//! the [`BuildInputs`] the resolver needs.
//!
//! # Canonical text and the content hash
//!
//! `spec/schema/akr-lock.md` §3.3 defines the revision content hash over "the canonically
//! formatted text of that record alone". [`canonical_record_text`] produces exactly that
//! by handing one record at a time to the phase P2 formatter and taking what it emits,
//! which keeps this module out of the business of knowing how a record is written.

use super::{BuildInputs, SourceFile};
use crate::diagnostics::{Diagnostic, FileId};
use crate::hash::source_file_hash;
use crate::model::{Ledger, LogicalKey, RevisionId};
use crate::syntax::cst::{File, Item};
use crate::syntax::{format, lower::lower_all, parse};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io};

/// A workspace read from disk.
#[derive(Debug)]
pub struct Workspace {
    /// The ledger, with every record tagged with the file it came from (V-003).
    pub ledger: Ledger,
    /// Build inputs: sources, their raw-byte hashes, and canonical text per revision.
    pub inputs: BuildInputs,
    /// Diagnostics from stages A and B.
    pub diagnostics: Vec<Diagnostic>,
    /// The `akr.lock` text, if the workspace has one.
    pub lock_text: Option<String>,
}

/// Reads every `.akr` source under an `.akr/` directory.
///
/// Discovery is a recursive walk whose entries are sorted by full path with a plain byte
/// comparison, so file order is the same on every filesystem
/// (`docs/06-compiler-pipeline.md` §3). `akr.lock` is read but is not a source: it is not
/// hashed into the source graph and it contributes no records.
///
/// `root` is the repository root; `akr_dir` is the `.akr` directory beneath it. Paths in
/// the returned [`SourceFile`]s are relative to `root` with forward slashes, which is the
/// form the lock records.
///
/// # Errors
/// Returns any I/O error from walking or reading.
pub fn load_workspace(root: &Path, akr_dir: &Path) -> io::Result<Workspace> {
    let mut paths = Vec::new();
    collect_akr_files(akr_dir, &mut paths)?;
    paths.sort();

    let mut sources = Vec::new();
    let mut parsed_files: Vec<(String, File)> = Vec::new();
    let mut canonical_text: BTreeMap<RevisionId, String> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    let mut lock_text = None;

    for (index, path) in paths.iter().enumerate() {
        let bytes = fs::read(path)?;
        let relative = relative_slash(root, path);
        if path.file_name().is_some_and(|n| n == "akr.lock") {
            lock_text = Some(String::from_utf8_lossy(&bytes).into_owned());
            continue;
        }

        let text = String::from_utf8_lossy(&bytes).into_owned();
        let file_id = FileId(u32::try_from(index).unwrap_or(u32::MAX));
        let parsed = parse(&text, file_id);
        diagnostics.extend(parsed.diagnostics);

        let mut records = 0u32;
        if let Some(file) = parsed.file {
            for (at, item) in file.items.iter().enumerate() {
                if let Item::Record(record) = item {
                    records += 1;
                    if let Ok(key) = LogicalKey::parse(&record.key)
                        && let Some(canonical) = canonical_record_text(&file, at)
                    {
                        canonical_text.insert(RevisionId::new(key, record.revision), canonical);
                    }
                }
            }
            parsed_files.push((relative.clone(), file));
        }

        sources.push(SourceFile {
            path: relative,
            hash: source_file_hash(&bytes),
            records,
        });
    }

    let (ledger, lower_diagnostics) = lower_all(&parsed_files);
    diagnostics.extend(lower_diagnostics);

    Ok(Workspace {
        ledger,
        inputs: BuildInputs {
            sources,
            canonical_text,
            ..BuildInputs::default()
        },
        diagnostics,
        lock_text,
    })
}

/// The canonical text of one record: from the `record` keyword through its closing brace,
/// with LF endings, no leading indentation, and a single trailing newline.
///
/// Produced by formatting a file that contains only that record and dropping the two
/// header lines. Using the real formatter rather than slicing the input is what makes the
/// hash stable across a reformat: text that was already canonical formats to itself, and
/// text that was not formats to the same canonical bytes as its reformatted twin.
///
/// Returns `None` if the index does not name a record.
#[must_use]
pub fn canonical_record_text(file: &File, index: usize) -> Option<String> {
    if !matches!(file.items.get(index), Some(Item::Record(_))) {
        return None;
    }
    let mut single = file.clone();
    single.leading = Vec::new();
    single.trailing = Vec::new();
    single.blank_before_header = false;
    single.items = vec![file.items[index].clone()];

    let rendered = format(&single);
    let start = rendered.find("\nrecord ")? + 1;
    Some(rendered[start..].to_owned())
}

/// Recursively collects `.akr` files, including `akr.lock`.
fn collect_akr_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .collect::<io::Result<Vec<_>>>()?
        .into_iter()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            // The index cache is generated and never a source (D-019).
            if entry.file_name().is_some_and(|n| n == "cache") {
                continue;
            }
            collect_akr_files(&entry, out)?;
        } else if entry.extension().is_some_and(|e| e == "akr" || e == "lock") {
            out.push(entry);
        }
    }
    Ok(())
}

/// A repo-root-relative path with forward slashes, as the lock records them.
fn relative_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
