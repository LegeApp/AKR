//! Deterministic Markdown extraction for review candidates.

use crate::hash::Sha256;
use crate::ingest::review::{
    CandidateFingerprint, CandidateId, CandidateKind, IngestCandidate, SourceSpan, SupportBlock,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableMode {
    /// Create one candidate per table data row.
    #[default]
    Rows,
    /// Attach the entire table as a support block.
    Support,
}

#[derive(Debug, Clone)]
pub struct ExtractOptions {
    pub table_mode: TableMode,
    pub extractor_version: String,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            table_mode: TableMode::Rows,
            extractor_version: "ingest-v1".to_owned(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Extraction {
    pub candidates: Vec<IngestCandidate>,
    pub diagnostics: Vec<ExtractionDiagnostic>,
}

#[derive(Debug, Clone)]
pub enum ExtractionDiagnostic {
    OrphanSupport { span: SourceSpan, details: String },
    MalformedFencedCode { span: SourceSpan, details: String },
    AmbiguousTable { span: SourceSpan, details: String },
    UnsupportedStructure { span: SourceSpan, details: String },
}

#[derive(Debug, Clone)]
struct Line {
    line_no: u32,
    start: usize,
    end: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct PendingCandidate {
    id: CandidateId,
    kind: CandidateKind,
    section_path: Vec<String>,
    parent: Option<CandidateId>,
    start_byte: usize,
    end_byte: usize,
    start_line: u32,
    end_line: u32,
}

impl PendingCandidate {
    fn new(
        id: CandidateId,
        kind: CandidateKind,
        section_path: Vec<String>,
        parent: Option<CandidateId>,
        line: &Line,
    ) -> Self {
        Self {
            id,
            kind,
            section_path,
            parent,
            start_byte: line.start,
            end_byte: line.end,
            start_line: line.line_no,
            end_line: line.line_no,
        }
    }

    fn append_line(&mut self, line: &Line) {
        self.end_byte = line.end;
        self.end_line = line.line_no;
    }
}

pub fn extract_markdown_items(source: &str, options: ExtractOptions) -> Extraction {
    let lines = split_lines(source);
    let mut candidates = Vec::new();
    let mut diagnostics = Vec::new();
    let mut section_path: Vec<String> = Vec::new();
    let mut list_stack: Vec<(CandidateId, usize)> = Vec::new();
    let mut active: Option<PendingCandidate> = None;
    let mut in_comment = false;

    let mut i = 0usize;
    while i < lines.len() {
        let line = &lines[i];
        let raw = line.text.as_str();
        let text = raw.trim_end_matches('\n').trim_end_matches('\r');
        let trimmed = text.trim();

        if in_comment {
            if trimmed.contains("-->") {
                in_comment = false;
            }
            i += 1;
            continue;
        }

        if trimmed.starts_with("<!--") {
            if !trimmed.contains("-->") {
                in_comment = true;
            }
            i += 1;
            continue;
        }

        if parse_setext_heading(&lines, i).is_some() {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.clear();
            let level = parse_setext_heading(&lines, i).expect("validated");
            let title = lines[i]
                .text
                .trim_end_matches('\n')
                .trim_end_matches('\r')
                .trim()
                .to_owned();
            update_section_path(&mut section_path, level, title);
            i += 2;
            continue;
        }

        if let Some((fence, language)) = parse_fenced_code_start(trimmed) {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            let close = find_fence_close(&lines, i + 1, &fence);
            if let Some(close_line) = close {
                let support = SupportBlock {
                    span: SourceSpan {
                        start_byte: line.start,
                        end_byte: lines[close_line].end,
                        start_line: line.line_no,
                        end_line: lines[close_line].line_no,
                    },
                    language,
                    raw_text: source[line.start..lines[close_line].end].to_owned(),
                };
                if !attach_support_to_last_candidate(&mut candidates, support, &section_path) {
                    diagnostics.push(ExtractionDiagnostic::OrphanSupport {
                        span: SourceSpan {
                            start_byte: line.start,
                            end_byte: lines[close_line].end,
                            start_line: line.line_no,
                            end_line: lines[close_line].line_no,
                        },
                        details: "code block had no candidate in current section".to_owned(),
                    });
                }
                i = close_line + 1;
                continue;
            }
            diagnostics.push(ExtractionDiagnostic::MalformedFencedCode {
                span: SourceSpan {
                    start_byte: line.start,
                    end_byte: source.len(),
                    start_line: line.line_no,
                    end_line: lines.last().map_or(line.line_no, |l| l.line_no),
                },
                details: "missing closing fence".to_owned(),
            });
            i += 1;
            continue;
        }

        if let Some(level) = parse_atx_heading(trimmed) {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.clear();
            let title = parse_heading_text(trimmed);
            update_section_path(&mut section_path, level, title);
            i += 1;
            continue;
        }

        if is_thematic_break(trimmed) || is_reference_definition(trimmed) {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.clear();
            i += 1;
            continue;
        }

        if let Some((header, rows, end)) = parse_table_block(&lines, i) {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.clear();
            match options.table_mode {
                TableMode::Rows => {
                    for row_line_no in rows {
                        let row = &lines[row_line_no];
                        let raw = row
                            .text
                            .trim_end_matches('\n')
                            .trim_end_matches('\r')
                            .to_owned();
                        let semantic = normalize_text(&raw);
                        let id = next_candidate_id(&candidates);
                        candidates.push(IngestCandidate::new(
                            id,
                            id.as_u32(),
                            SourceSpan {
                                start_byte: row.start,
                                end_byte: row.end,
                                start_line: row.line_no,
                                end_line: row.line_no,
                            },
                            section_path.clone(),
                            None,
                            CandidateKind::TableRow,
                            raw,
                            semantic,
                            Vec::new(),
                        ));
                    }
                }
                TableMode::Support => {
                    let start = lines[i].start;
                    let end_byte = lines[end.saturating_sub(1)].end;
                    let support = SupportBlock {
                        span: SourceSpan {
                            start_byte: start,
                            end_byte,
                            start_line: lines[i].line_no,
                            end_line: lines[end.saturating_sub(1)].line_no,
                        },
                        language: None,
                        raw_text: source[start..end_byte].to_owned(),
                    };
                    if !attach_support_to_last_candidate(&mut candidates, support, &section_path) {
                        diagnostics.push(ExtractionDiagnostic::OrphanSupport {
                            span: SourceSpan {
                                start_byte: start,
                                end_byte,
                                start_line: lines[i].line_no,
                                end_line: lines[end.saturating_sub(1)].line_no,
                            },
                            details: "table support had no candidate in current section".to_owned(),
                        });
                    }
                }
            }
            let _ = header;
            i = end;
            continue;
        }

        if let Some((indent, start_pos)) = parse_list_item_start(trimmed) {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.retain(|&(_, indent_or)| indent_or <= indent);
            let parent = list_stack.last().map(|(id, _)| *id);
            let id = next_candidate_id(&candidates);
            let mut pending = PendingCandidate::new(
                id,
                CandidateKind::ListItem,
                section_path.clone(),
                parent,
                &Line {
                    line_no: line.line_no,
                    start: line.start,
                    end: line.start + start_pos,
                    text: line.text[0..start_pos].to_owned(),
                },
            );
            pending.append_line(line);
            active = Some(pending);
            list_stack.push((id, indent));
            i += 1;
            continue;
        }

        if parse_blockquote_start(trimmed).is_some() {
            if let Some(active_item) = active.as_ref()
                && active_item.kind != CandidateKind::BlockQuote
                && let Some(active) = active.take()
            {
                finalize_candidate(source, active, &mut candidates);
            }
            if active.is_none() {
                let id = next_candidate_id(&candidates);
                active = Some(PendingCandidate::new(
                    id,
                    CandidateKind::BlockQuote,
                    section_path.clone(),
                    None,
                    line,
                ));
            }
            if let Some(active_item) = active.as_mut() {
                active_item.append_line(line);
            }
            i += 1;
            continue;
        }

        if is_blank_line(trimmed) {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.clear();
            i += 1;
            continue;
        }

        if trimmed.starts_with('<') && !trimmed.starts_with("<!--") && !trimmed.starts_with("</") {
            if let Some(active) = active.take() {
                finalize_candidate(source, active, &mut candidates);
            }
            list_stack.clear();
            diagnostics.push(ExtractionDiagnostic::UnsupportedStructure {
                span: SourceSpan {
                    start_byte: line.start,
                    end_byte: line.end,
                    start_line: line.line_no,
                    end_line: line.line_no,
                },
                details: "embedded html block is ignored".to_owned(),
            });
            i += 1;
            continue;
        }

        if let Some(active_item) = active.as_mut() {
            if should_continue_paragraph(active_item, line, &list_stack) {
                active_item.append_line(line);
                i += 1;
                continue;
            }
            if active_item.kind == CandidateKind::ListItem && is_indented_code(trimmed) {
                let support_start = line.start;
                let mut end = i + 1;
                while end < lines.len()
                    && is_indented_code(
                        lines[end]
                            .text
                            .trim_end_matches('\n')
                            .trim_end_matches('\r')
                            .trim(),
                    )
                {
                    end += 1;
                }
                let support = SupportBlock {
                    span: SourceSpan {
                        start_byte: support_start,
                        end_byte: lines[end.saturating_sub(1)].end,
                        start_line: line.line_no,
                        end_line: lines[end.saturating_sub(1)].line_no,
                    },
                    language: None,
                    raw_text: source[support_start..lines[end.saturating_sub(1)].end].to_owned(),
                };
                finalize_candidate(source, active_item_clone(active_item), &mut candidates);
                active = None;
                let _ = attach_support_to_last_candidate(&mut candidates, support, &section_path);
                i = end;
                continue;
            }
        }

        if active.is_none() {
            let id = next_candidate_id(&candidates);
            active = Some(PendingCandidate::new(
                id,
                CandidateKind::Paragraph,
                section_path.clone(),
                None,
                line,
            ));
        } else if let Some(active) = active.as_mut() {
            active.append_line(line);
        }
        i += 1;
    }

    if let Some(active) = active {
        finalize_candidate(source, active, &mut candidates);
    }

    let mut extraction = Extraction {
        candidates,
        diagnostics,
    };
    assign_stable_fingerprints(&mut extraction.candidates, &options.extractor_version);
    extraction
}

fn split_lines(source: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    for (line_no, part) in (1u32..).zip(source.split_inclusive('\n')) {
        let end = start + part.len();
        lines.push(Line {
            line_no,
            start,
            end,
            text: part.to_owned(),
        });
        start = end;
    }
    if source.is_empty() {
        lines.push(Line {
            line_no: 1,
            start: 0,
            end: 0,
            text: String::new(),
        });
    }
    lines
}

fn is_blank_line(text: &str) -> bool {
    text.trim().is_empty()
}

fn parse_setext_heading(lines: &[Line], index: usize) -> Option<usize> {
    if index + 1 >= lines.len() {
        return None;
    }
    let candidate = lines[index]
        .text
        .trim_end_matches('\n')
        .trim_end_matches('\r');
    if candidate.trim().is_empty() {
        return None;
    }
    let delim = lines[index + 1]
        .text
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .trim();
    if delim.chars().all(|c| c == '=') && !delim.is_empty() && delim.len() >= 3 {
        return Some(1);
    }
    if delim.chars().all(|c| c == '-') && !delim.is_empty() && delim.len() >= 3 {
        return Some(2);
    }
    None
}

fn parse_fenced_code_start(text: &str) -> Option<(String, Option<String>)> {
    let trimmed = text.trim();
    if trimmed.starts_with("```") {
        let rest = trimmed.trim_start_matches("```");
        let language = rest.split_whitespace().next().filter(|s| !s.is_empty());
        return Some(("```".to_owned(), language.map(ToOwned::to_owned)));
    }
    if trimmed.starts_with("~~~") {
        let rest = trimmed.trim_start_matches("~~~");
        let language = rest.split_whitespace().next().filter(|s| !s.is_empty());
        return Some(("~~~".to_owned(), language.map(ToOwned::to_owned)));
    }
    None
}

fn find_fence_close(lines: &[Line], start: usize, marker: &str) -> Option<usize> {
    let mut i = start;
    while i < lines.len() {
        let text = lines[i]
            .text
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .trim();
        if text == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_atx_heading(trimmed: &str) -> Option<usize> {
    let bytes = trimmed.as_bytes();
    let mut level = 0usize;
    while level < bytes.len() && bytes[level] == b'#' {
        level += 1;
    }
    if !(1..=6).contains(&level) {
        return None;
    }
    if !bytes.get(level).is_some_and(|c| c.is_ascii_whitespace()) {
        return None;
    }
    Some(level)
}

fn parse_heading_text(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut level = 0usize;
    while level < bytes.len() && bytes[level] == b'#' {
        level += 1;
    }
    text[level..].trim().trim_end_matches('#').trim().to_owned()
}

fn is_thematic_break(trimmed: &str) -> bool {
    let text = trimmed;
    if text.len() < 3 {
        return false;
    }
    let no_space = text.split_whitespace().collect::<Vec<_>>().join("");
    if no_space.is_empty() {
        return false;
    }
    no_space.chars().all(|c| matches!(c, '-' | '*' | '_'))
}

fn is_reference_definition(trimmed: &str) -> bool {
    trimmed.starts_with('[') && trimmed.contains("]: ")
}

fn parse_list_item_start(trimmed: &str) -> Option<(usize, usize)> {
    let mut indent = 0usize;
    for ch in trimmed.chars() {
        if ch == ' ' || ch == '\t' {
            indent += 1;
            continue;
        }
        break;
    }
    let rest = trimmed.get(indent..)?;
    if rest.starts_with("- ") || rest.starts_with("+ ") || rest.starts_with("* ") {
        return Some((indent, indent + 2));
    }
    let first_space = rest.find(' ').unwrap_or(0);
    let marker = &rest[..first_space];
    let body = &rest[first_space..];
    let looks_numbered = !marker.is_empty()
        && marker[..marker.len().saturating_sub(0)]
            .chars()
            .all(|c| c == '.' || c == ')' || c.is_ascii_digit())
        && (marker.ends_with('.') || marker.ends_with(')'));
    if looks_numbered && body.starts_with(' ') {
        return Some((indent, indent + first_space + 1));
    }
    None
}

fn parse_blockquote_start(trimmed: &str) -> Option<usize> {
    let content = trimmed.trim_start();
    if content.starts_with('>') {
        return Some(trimmed.len() - content.len());
    }
    None
}

fn is_indented_code(trimmed: &str) -> bool {
    trimmed.starts_with("    ") || trimmed.starts_with('\t')
}

fn parse_table_block(lines: &[Line], index: usize) -> Option<(String, Vec<usize>, usize)> {
    if index + 1 >= lines.len() {
        return None;
    }
    let header_raw = lines[index]
        .text
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .trim();
    let delimiter = lines[index + 1]
        .text
        .trim_end_matches('\n')
        .trim_end_matches('\r')
        .trim();
    if !is_table_row(header_raw) {
        return None;
    }
    if !is_table_delimiter(delimiter) {
        return None;
    }
    let header_cells = split_table_cells(header_raw);
    if header_cells.len() < 2 {
        return None;
    }
    let mut rows = Vec::new();
    let mut i = index + 2;
    while i < lines.len() {
        let row_text = lines[i]
            .text
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .trim();
        if !is_table_row(row_text) {
            break;
        }
        let cells = split_table_cells(row_text);
        if cells.len() != header_cells.len() {
            break;
        }
        rows.push(i);
        i += 1;
    }
    if rows.is_empty() {
        return Some((lines[index].text.clone(), rows, i));
    }
    Some((lines[index].text.clone(), rows, i))
}

fn is_table_delimiter(text: &str) -> bool {
    let cells = split_table_cells(text);
    if cells.len() < 2 {
        return false;
    }
    for cell in cells {
        let c = cell.trim();
        if c.len() < 3 {
            return false;
        }
        let first_ok = c.starts_with(':') || c.ends_with(':') || c.starts_with('-');
        let dashes = c.chars().filter(|ch| *ch == '-').count();
        if !first_ok
            || dashes < 1
            || c.chars()
                .any(|ch| !(ch == '-' || ch == ':' || ch.is_ascii_whitespace()))
        {
            return false;
        }
    }
    true
}

fn is_table_row(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.contains('|')
}

fn split_table_cells(text: &str) -> Vec<String> {
    text.trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_owned())
        .collect()
}

fn should_continue_paragraph(
    active: &PendingCandidate,
    line: &Line,
    list_stack: &[(CandidateId, usize)],
) -> bool {
    match active.kind {
        CandidateKind::ListItem => {
            if let Some((_, indent)) = list_stack.last() {
                let current = leading_whitespace_count(line.text.as_str());
                current > *indent && !is_blank_line(&line.text)
            } else {
                false
            }
        }
        CandidateKind::BlockQuote => line.text.trim_start().starts_with('>'),
        CandidateKind::Paragraph | CandidateKind::TableRow => {
            !(parse_atx_heading(line.text.trim()).is_some()
                || parse_setext_heading(std::slice::from_ref(line), 0).is_some()
                || parse_fenced_code_start(line.text.trim_end_matches('\n').trim_end_matches('\r'))
                    .is_some()
                || parse_table_block(std::slice::from_ref(line), 0).is_some()
                || parse_list_item_start(line.text.trim()).is_some()
                || is_thematic_break(line.text.trim()))
        }
    }
}

fn finalize_candidate(
    source: &str,
    pending: PendingCandidate,
    candidates: &mut Vec<IngestCandidate>,
) {
    let raw = source[pending.start_byte..pending.end_byte].to_owned();
    if raw.trim().is_empty() {
        return;
    }
    let semantic = normalize_text(&raw);
    candidates.push(IngestCandidate::new(
        pending.id,
        pending.id.as_u32(),
        SourceSpan {
            start_byte: pending.start_byte,
            end_byte: pending.end_byte,
            start_line: pending.start_line,
            end_line: pending.end_line,
        },
        pending.section_path,
        pending.parent,
        pending.kind,
        raw,
        semantic,
        Vec::new(),
    ));
}

fn active_item_clone(active: &PendingCandidate) -> PendingCandidate {
    active.clone()
}

fn attach_support_to_last_candidate(
    candidates: &mut [IngestCandidate],
    support: SupportBlock,
    section_path: &[String],
) -> bool {
    if let Some(candidate) = candidates
        .iter_mut()
        .rev()
        .find(|candidate| candidate.section_path == section_path)
    {
        candidate.support.push(support);
        true
    } else {
        false
    }
}

fn update_section_path(path: &mut Vec<String>, level: usize, title: String) {
    if level == 0 {
        return;
    }
    if path.len() >= level {
        path.truncate(level - 1);
    }
    while path.len() + 1 < level {
        path.push("".to_owned());
    }
    path.push(title);
}

/// Assigns deterministic fingerprints for a candidate sequence.
///
/// The algorithm intentionally avoids hash randomisation so repeated extractions over the
/// same source keep candidate identities stable across machines and invocations.
pub fn assign_stable_fingerprints(candidates: &mut [IngestCandidate], extractor_version: &str) {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for candidate in candidates.iter_mut() {
        let base = candidate_signature(candidate, extractor_version);
        let next = seen.entry(base.clone()).or_insert(0);
        *next += 1;
        let signature = format!("{base}|{next}");
        let mut hasher = Sha256::new();
        hasher.update(signature.as_bytes());
        candidate.fingerprint = CandidateFingerprint::new(hasher.finish().to_hex());
    }
}

fn candidate_signature(candidate: &IngestCandidate, extractor_version: &str) -> String {
    let mut parts = String::new();
    parts.push_str(extractor_version);
    parts.push('|');
    parts.push_str(&candidate.section_path.join("/"));
    parts.push('|');
    parts.push_str(&format!("{:?}", candidate.kind));
    parts.push('|');
    parts.push_str(&candidate.raw_text);
    parts.push('|');
    parts.push_str(&candidate.semantic_text);
    parts
}

fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn next_candidate_id(candidates: &[IngestCandidate]) -> CandidateId {
    CandidateId::new(u32::try_from(candidates.len() + 1).expect("candidate count fits u32"))
}

fn leading_whitespace_count(text: &str) -> usize {
    text.chars().take_while(|c| c.is_ascii_whitespace()).count()
}
