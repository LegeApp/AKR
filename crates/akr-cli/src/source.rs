//! `akr source *` — immutable source library in `sources/`.
//!
//! `sources/` is append-only, content-hashed. Agents and humans must never
//! edit files there; `akr source verify` (and `akr check`) report `AKR-S021`
//! on mismatch, and `akr source supersede` is the only mutation path.

use crate::commands::Output;
use crate::session::{EnvError, Exit, Session};
use akr_core::json::Value;
use akr_core::source::{self, SourceDiagnostic, SourceDocument, SourceOrigin};
use std::path::{Path, PathBuf};

/// `akr source add <path> ...`
pub fn add(
    session: &Session,
    path: &Path,
    id: Option<&str>,
    title: Option<&str>,
    origin: Option<&str>,
    observed_at: Option<&str>,
    scope: Option<&str>,
) -> Result<Output, EnvError> {
    let workspace_root = session.root.clone();
    let origin = origin.unwrap_or("external");
    let origin_ty = SourceOrigin::from_str(origin).ok_or_else(|| {
        EnvError::new(
            "AKR-C004",
            format!("origin {origin:?} is not external|internal-reference"),
        )
    })?;

    let bytes = std::fs::read(path)
        .map_err(|e| EnvError::new("AKR-C042", format!("cannot read {}: {e}", path.display())))?;
    let content_hash = source::hash_bytes(&bytes);
    let byte_len = bytes.len() as u64;

    let stem = id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| derive_id(path, &bytes));
    let safe_id = sanitize_id(&stem);
    if safe_id.is_empty() {
        return Err(EnvError::new("AKR-C004", "source id must be non-empty"));
    }

    // Deduplicate by id.
    let mut catalog = source::load_catalog(&workspace_root).map_err(|d| to_env(d))?;
    if catalog.iter().any(|d| d.id == safe_id) {
        return Err(EnvError::new(
            "AKR-C042",
            format!("source {safe_id:?} already registered; use `akr source supersede`"),
        ));
    }

    let short = short_hash(&content_hash);
    let file_name = format!("{safe_id}--{short}.md");
    let rel_path = PathBuf::from("sources").join("external").join(&file_name);
    let dest = workspace_root.join(&rel_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            EnvError::new(
                "AKR-C042",
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    if dest.exists() {
        return Err(EnvError::new(
            "AKR-C042",
            format!("{} already exists", dest.display()),
        ));
    }
    std::fs::write(&dest, &bytes)
        .map_err(|e| EnvError::new("AKR-C042", format!("cannot write {}: {e}", dest.display())))?;

    let title = title.unwrap_or(&safe_id).to_owned();
    let added_at = today_iso();
    let doc = SourceDocument {
        id: safe_id.clone(),
        title,
        origin: origin_ty,
        media_type: "text/markdown".into(),
        path: rel_path.to_string_lossy().replace('\\', "/"),
        content_hash: content_hash.clone(),
        byte_len,
        added_at,
        observed_at: observed_at.map(ToOwned::to_owned),
        scope: scope.map(ToOwned::to_owned),
        supersedes: None,
    };
    catalog.push(doc.clone());
    source::save_catalog(&workspace_root, &catalog).map_err(|d| to_env(d))?;

    let text = format!(
        "registered source {safe_id}\n  {content_hash}\n  {}\n",
        doc.path
    );
    let result = Value::object(vec![
        ("id".into(), Value::string(safe_id)),
        ("content_hash".into(), Value::string(content_hash)),
        ("path".into(), Value::string(doc.path)),
        ("byte_len".into(), Value::integer(byte_len as i64)),
    ]);
    Ok(Output::plain(text, result))
}

/// `akr source list`
pub fn list(session: &Session, all: bool) -> Result<Output, EnvError> {
    let catalog = source::load_catalog(&session.root).map_err(|d| to_env(d))?;
    let mut text = String::new();
    if catalog.is_empty() {
        text.push_str("no sources registered\n");
    } else {
        for doc in &catalog {
            if !all && is_superseded(doc, &catalog) {
                continue;
            }
            text.push_str(&format!(
                "{}  {}  {}  {}\n",
                doc.id,
                doc.origin.as_str(),
                doc.content_hash,
                doc.path
            ));
        }
    }
    let result = Value::object(vec![
        (
            "sources".into(),
            Value::array(catalog.iter().map(doc_to_json).collect()),
        ),
        ("count".into(), Value::integer(catalog.len() as i64)),
    ]);
    Ok(Output::plain(text, result))
}

/// `akr source get <id>`
pub fn get(
    session: &Session,
    id: &str,
    whole: bool,
    lines: Option<&str>,
    section: Option<&str>,
) -> Result<Output, EnvError> {
    let catalog = source::load_catalog(&session.root).map_err(|d| to_env(d))?;
    let doc = catalog
        .iter()
        .find(|d| d.id == id)
        .ok_or_else(|| EnvError::new("AKR-C042", format!("source {id:?} not found")))?;
    let file = session.root.join(&doc.path);
    let bytes = std::fs::read(&file)
        .map_err(|e| EnvError::new("AKR-C042", format!("cannot read {}: {e}", file.display())))?;
    let text_raw = String::from_utf8_lossy(&bytes).into_owned();

    // Verify hash before serving (defensive).
    let found = source::hash_bytes(&bytes);
    if found != doc.content_hash {
        return Err(EnvError::new(
            "AKR-S021",
            format!(
                "registered source bytes do not match their content hash\nsource: {}\nexpected: {}\nfound: {}",
                doc.id, doc.content_hash, found
            ),
        ));
    }

    let output = if let Some(sec) = section {
        extract_section(&text_raw, sec)
            .ok_or_else(|| EnvError::new("AKR-C004", format!("section {sec:?} not found")))?
    } else if let Some(range) = lines {
        extract_lines(&text_raw, range)?
    } else if whole || lines.is_none() && section.is_none() {
        text_raw.clone()
    } else {
        text_raw.clone()
    };

    let result = Value::object(vec![
        ("id".into(), Value::string(doc.id.clone())),
        ("path".into(), Value::string(doc.path.clone())),
        (
            "content_hash".into(),
            Value::string(doc.content_hash.clone()),
        ),
        ("text".into(), Value::string(output.clone())),
    ]);
    Ok(Output::plain(output, result))
}

/// `akr source verify`
pub fn verify(session: &Session) -> Result<Output, EnvError> {
    let diags = source::verify_catalog(&session.root);
    if diags.is_empty() {
        return Ok(Output::plain(
            "all sources verified\n",
            Value::object(vec![("ok".into(), Value::bool(true))]),
        ));
    }
    let mut text = String::new();
    for d in &diags {
        text.push_str(&format!("{d}\n\n"));
    }
    let result = Value::object(vec![
        ("ok".into(), Value::bool(false)),
        (
            "diagnostics".into(),
            Value::array(diags.iter().map(|d| Value::string(d.to_string())).collect()),
        ),
    ]);
    // Return diagnostics but let caller decide exit. Here we surface as Output with diagnostics counts.
    // For `akr check` we need Diagnostics; for CLI `source verify` we emit text and exit diagnostics.
    // Use plain output; check will handle.
    Ok(Output::plain(text, result))
}

/// `akr source supersede <old-id> <new-path>`
pub fn supersede(
    session: &Session,
    old_id: &str,
    new_path: &Path,
    new_id: Option<&str>,
) -> Result<Output, EnvError> {
    let workspace_root = session.root.clone();
    let mut catalog = source::load_catalog(&workspace_root).map_err(|d| to_env(d))?;
    let old = catalog
        .iter()
        .find(|d| d.id == old_id)
        .ok_or_else(|| EnvError::new("AKR-C042", format!("source {old_id:?} not found")))?
        .clone();

    let bytes = std::fs::read(new_path).map_err(|e| {
        EnvError::new(
            "AKR-C042",
            format!("cannot read {}: {e}", new_path.display()),
        )
    })?;
    let content_hash = source::hash_bytes(&bytes);
    let byte_len = bytes.len() as u64;

    let new_id_owned = new_id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{old_id}--v2"));
    let safe_new_id = sanitize_id(&new_id_owned);
    if catalog.iter().any(|d| d.id == safe_new_id) {
        return Err(EnvError::new(
            "AKR-C042",
            format!("source {safe_new_id:?} already exists"),
        ));
    }
    let short = short_hash(&content_hash);
    let file_name = format!("{safe_new_id}--{short}.md");
    let rel_path = PathBuf::from("sources").join("external").join(&file_name);
    let dest = workspace_root.join(&rel_path);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            EnvError::new(
                "AKR-C042",
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    std::fs::write(&dest, &bytes)
        .map_err(|e| EnvError::new("AKR-C042", format!("cannot write {}: {e}", dest.display())))?;

    let doc = SourceDocument {
        id: safe_new_id.clone(),
        title: old.title.clone(),
        origin: old.origin.clone(),
        media_type: "text/markdown".into(),
        path: rel_path.to_string_lossy().replace('\\', "/"),
        content_hash: content_hash.clone(),
        byte_len,
        added_at: today_iso(),
        observed_at: None,
        scope: old.scope.clone(),
        supersedes: Some(old_id.to_owned()),
    };
    catalog.push(doc.clone());
    source::save_catalog(&workspace_root, &catalog).map_err(|d| to_env(d))?;

    let text = format!(
        "superseded {old_id} with {safe_new_id}\n  {content_hash}\n  {}\n",
        doc.path
    );
    let result = Value::object(vec![
        ("old_id".into(), Value::string(old_id.to_owned())),
        ("new_id".into(), Value::string(safe_new_id)),
        ("content_hash".into(), Value::string(content_hash)),
        ("path".into(), Value::string(doc.path)),
    ]);
    Ok(Output::plain(text, result))
}

fn resolve_workspace_file(root: &Path, requested: &Path) -> Result<PathBuf, EnvError> {
    if requested.is_absolute() {
        return Err(EnvError::new(
            "AKR-C042",
            format!("absolute path {} is not allowed", requested.display()),
        ));
    }
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let joined = root.join(requested);
    let canonical_file = joined.canonicalize().map_err(|e| {
        EnvError::new(
            "AKR-C042",
            format!("cannot resolve {}: {e}", requested.display()),
        )
    })?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(EnvError::new(
            "AKR-C042",
            format!("{} is outside the workspace", requested.display()),
        ));
    }
    Ok(canonical_file)
}

fn sanitize_id(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c);
        } else if c.is_whitespace() || c == '/' || c == '\\' {
            out.push('-');
        } else {
            out.push('-');
        }
    }
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').to_owned()
}

fn derive_id(path: &Path, bytes: &[u8]) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("source");
    let short = short_hash(&source::hash_bytes(bytes));
    format!("{stem}--{short}")
}

fn short_hash(content_hash: &str) -> String {
    let h = content_hash.strip_prefix("sha256:").unwrap_or(content_hash);
    h[..8.min(h.len())].to_owned()
}

fn today_iso() -> String {
    // Use SystemTime like session::current_date, but output ISO.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", year, m, d)
}

fn is_superseded(doc: &SourceDocument, catalog: &[SourceDocument]) -> bool {
    catalog
        .iter()
        .any(|d| d.supersedes.as_deref() == Some(&doc.id))
}

fn doc_to_json(d: &SourceDocument) -> Value {
    Value::object(vec![
        ("id".into(), Value::string(d.id.clone())),
        ("title".into(), Value::string(d.title.clone())),
        ("origin".into(), Value::string(d.origin.as_str().to_owned())),
        ("path".into(), Value::string(d.path.clone())),
        ("content_hash".into(), Value::string(d.content_hash.clone())),
        ("byte_len".into(), Value::integer(d.byte_len as i64)),
        ("added_at".into(), Value::string(d.added_at.clone())),
    ])
}

fn extract_lines(text: &str, range: &str) -> Result<String, EnvError> {
    let (start, end) = parse_range(range)?;
    let lines: Vec<&str> = text.lines().collect();
    if start < 1 || end < start || end as usize > lines.len() {
        return Err(EnvError::new(
            "AKR-C004",
            format!("line range {range:?} out of bounds (1..{})", lines.len()),
        ));
    }
    let slice = &lines[(start as usize - 1)..(end as usize)];
    Ok(slice.join("\n") + "\n")
}

fn parse_range(s: &str) -> Result<(u32, u32), EnvError> {
    let mut parts = s.split(':');
    let a = parts
        .next()
        .ok_or_else(|| EnvError::new("AKR-C004", format!("invalid range {s:?}")))?;
    let b = parts
        .next()
        .ok_or_else(|| EnvError::new("AKR-C004", format!("invalid range {s:?}")))?;
    if parts.next().is_some() {
        return Err(EnvError::new("AKR-C004", format!("invalid range {s:?}")));
    }
    let start: u32 = a
        .parse()
        .map_err(|_| EnvError::new("AKR-C004", format!("invalid range {s:?}")))?;
    let end: u32 = b
        .parse()
        .map_err(|_| EnvError::new("AKR-C004", format!("invalid range {s:?}")))?;
    Ok((start, end))
}

fn extract_section(text: &str, heading: &str) -> Option<String> {
    let needle = heading.trim();
    let lines: Vec<&str> = text.lines().collect();
    let mut start: Option<usize> = None;
    let mut level: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        if let Some((lv, title)) = parse_heading(line) {
            if title.trim() == needle {
                start = Some(i);
                level = Some(lv);
                break;
            }
        }
    }
    let start = start?;
    let lv = level.unwrap_or(1);
    let mut end = lines.len();
    for (i, line) in lines.iter().enumerate().skip(start + 1) {
        if let Some((l, _)) = parse_heading(line) {
            if l <= lv {
                end = i;
                break;
            }
        }
    }
    Some(lines[start..end].join("\n") + "\n")
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim();
    let mut lvl = 0;
    for c in trimmed.chars() {
        if c == '#' {
            lvl += 1;
        } else {
            break;
        }
    }
    if lvl == 0 || lvl > 6 {
        return None;
    }
    if !trimmed[lvl..].starts_with(' ') && !trimmed[lvl..].is_empty() {
        return None;
    }
    Some((lvl, trimmed[lvl..].trim().to_owned()))
}

fn to_env(d: SourceDiagnostic) -> EnvError {
    match d {
        SourceDiagnostic::HashMismatch { .. } => EnvError::new("AKR-S021", d.to_string()),
        SourceDiagnostic::MissingFile { .. } => EnvError::new("AKR-S021", d.to_string()),
        SourceDiagnostic::CatalogError(msg) => EnvError::new("AKR-C042", msg),
    }
}
