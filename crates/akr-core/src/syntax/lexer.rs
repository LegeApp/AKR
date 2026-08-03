//! The lexer: bytes in, tokens with byte spans out.
//!
//! Every construct is decided by its first character (`docs/03` §5), so there is no
//! lookahead beyond one byte and no backtracking. The lexer recovers from every error it
//! reports, so a file with three bad literals produces three diagnostics rather than one.

use crate::diagnostics::{Code, Diagnostic, FileId, Label, Severity, Span, Subject, codes as c};

/// What a token is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// `[a-z][a-z0-9_.-]*` — slot names, kinds, enum members, keys, keywords.
    Word,
    /// A literal beginning with a digit or `-`: dates, timestamps, integers, versions.
    Scalar,
    /// `git:` followed by hex digits.
    Commit,
    /// A quoted string. `value` holds the decoded text.
    Str,
    /// A prose block. `value` holds the dedented text.
    Prose,
    /// A reference beginning with `@`. `value` holds the text after the `@`.
    Ref,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `/`
    Slash,
    /// A `#` comment, retained as trivia.
    Comment,
    /// End of input.
    Eof,
}

/// One token.
#[derive(Debug, Clone)]
pub struct Token {
    /// What it is.
    pub kind: TokenKind,
    /// Where it is.
    pub span: Span,
    /// The decoded value: text for words and scalars, contents for strings and prose,
    /// the body for references, the comment text for comments.
    pub value: String,
    /// 1-based line the token starts on.
    pub line: u32,
    /// Whether a blank line immediately precedes this token.
    pub blank_before: bool,
}

impl Token {
    /// Whether this token is the given keyword.
    #[must_use]
    pub fn is_word(&self, word: &str) -> bool {
        self.kind == TokenKind::Word && self.value == word
    }
}

/// The result of lexing: tokens, and whatever went wrong.
#[derive(Debug)]
pub struct Lexed {
    /// The tokens, ending with [`TokenKind::Eof`].
    pub tokens: Vec<Token>,
    /// Diagnostics raised while lexing.
    pub diagnostics: Vec<Diagnostic>,
}

struct Lexer<'a> {
    src: &'a [u8],
    text: &'a str,
    file: FileId,
    at: usize,
    line: u32,
    line_start: bool,
    blank_run: bool,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

/// Lexes a source file.
///
/// Never panics and always terminates: every branch consumes at least one byte.
#[must_use]
pub fn lex(text: &str, file: FileId) -> Lexed {
    let mut lexer = Lexer {
        src: text.as_bytes(),
        text,
        file,
        at: 0,
        line: 1,
        line_start: true,
        blank_run: false,
        tokens: Vec::new(),
        diagnostics: Vec::new(),
    };
    lexer.run();
    Lexed {
        tokens: lexer.tokens,
        diagnostics: lexer.diagnostics,
    }
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.get(self.at).copied()
    }

    /// Advances past one whole UTF-8 character.
    ///
    /// Advancing by a byte would land inside a multi-byte character, and the next slice
    /// would panic. Every branch that steps over "one character" must use this.
    fn advance_char(&mut self) {
        if self.at < self.src.len() {
            self.at += 1;
            while self.at < self.src.len() && (self.src[self.at] & 0xC0) == 0x80 {
                self.at += 1;
            }
        }
    }

    fn span(&self, start: usize) -> Span {
        Span {
            file: self.file,
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(self.at).unwrap_or(u32::MAX),
        }
    }

    fn error(&mut self, code: Code, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            rule: None,
            message: message.into(),
            primary: Label {
                subject: Subject::Ledger,
                span: Some(span),
                message: None,
            },
            notes: Vec::new(),
            help: None,
        });
    }

    fn push(&mut self, kind: TokenKind, start: usize, value: String) {
        let span = self.span(start);
        let blank_before = self.blank_run;
        self.blank_run = false;
        self.tokens.push(Token {
            kind,
            span,
            value,
            line: self.line,
            blank_before,
        });
    }

    fn run(&mut self) {
        while let Some(byte) = self.peek() {
            match byte {
                b'\n' => {
                    self.at += 1;
                    if self.line_start {
                        self.blank_run = true;
                    }
                    self.line += 1;
                    self.line_start = true;
                }
                b'\r' => {
                    let start = self.at;
                    self.at += 1;
                    let span = self.span(start);
                    self.error(
                        c::P003,
                        span,
                        format!(
                            "carriage return at line {}; AKR files use LF line endings",
                            self.line
                        ),
                    );
                }
                b' ' | b'\t' => {
                    self.at += 1;
                }
                b'#' => self.comment(),
                b'"' => self.quoted(),
                b'@' => self.reference(),
                b'{' | b'}' | b'[' | b']' | b',' | b':' | b'/' => self.punct(byte),
                b'a'..=b'z' => self.word(),
                b'0'..=b'9' => self.scalar(),
                b'-' if matches!(self.src.get(self.at + 1), Some(b'0'..=b'9')) => self.scalar(),
                _ => {
                    let start = self.at;
                    // A whole character, so a multi-byte glyph produces one diagnostic
                    // rather than four.
                    self.advance_char();
                    let span = self.span(start);
                    let found = &self.text[start..self.at];
                    self.error(c::P001, span, format!("unexpected character {found:?}"));
                    self.line_start = false;
                }
            }
        }
        let start = self.at;
        self.push(TokenKind::Eof, start, String::new());
    }

    fn punct(&mut self, byte: u8) {
        let start = self.at;
        self.at += 1;
        self.line_start = false;
        let kind = match byte {
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b':' => TokenKind::Colon,
            _ => TokenKind::Slash,
        };
        self.push(kind, start, self.text[start..self.at].to_owned());
    }

    fn comment(&mut self) {
        let start = self.at;
        let own_line = self.line_start;
        while let Some(byte) = self.peek() {
            if byte == b'\n' {
                break;
            }
            self.at += 1;
        }
        let text = self.text[start + 1..self.at].trim().to_owned();
        let span = self.span(start);
        let blank_before = self.blank_run;
        self.blank_run = false;
        self.tokens.push(Token {
            kind: TokenKind::Comment,
            span,
            value: text,
            line: self.line,
            // A comment records whether it began its own line: that is what decides
            // leading versus trailing attachment (D-006).
            blank_before: blank_before && own_line,
        });
        if !own_line {
            // Mark trailing comments by leaving line_start false; the attach pass reads
            // token lines instead, so nothing else is needed here.
        }
        self.line_start = false;
    }

    fn word(&mut self) {
        let start = self.at;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || byte == b'_'
                || byte == b'-'
                || byte == b'.'
            {
                self.at += 1;
            } else {
                break;
            }
        }
        // `git:` is the one word that continues past a colon (D-008).
        if &self.text[start..self.at] == "git" && self.peek() == Some(b':') {
            self.at += 1;
            let hex_start = self.at;
            while let Some(byte) = self.peek() {
                if byte.is_ascii_alphanumeric() {
                    self.at += 1;
                } else {
                    break;
                }
            }
            let hex = self.text[hex_start..self.at].to_owned();
            let span = self.span(start);
            if hex.len() != 40 {
                self.error(
                    c::P021,
                    span,
                    format!("commit hash must be 40 hex digits, found {}", hex.len()),
                );
            } else if hex.bytes().any(|b| b.is_ascii_uppercase()) {
                self.error(c::P026, span, "commit hash must be lowercase hex");
            } else if !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                self.error(c::P021, span, "commit hash must be 40 hex digits");
            }
            self.line_start = false;
            self.push(TokenKind::Commit, start, hex);
            return;
        }
        self.line_start = false;
        let value = self.text[start..self.at].to_owned();
        self.push(TokenKind::Word, start, value);
    }

    fn scalar(&mut self) {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        while let Some(byte) = self.peek() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b':' | b'.' | b'+') {
                self.at += 1;
            } else {
                break;
            }
        }
        self.line_start = false;
        let value = self.text[start..self.at].to_owned();
        self.push(TokenKind::Scalar, start, value);
    }

    fn reference(&mut self) {
        let start = self.at;
        self.at += 1;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'/' | b'#' | b'_') {
                self.at += 1;
            } else {
                break;
            }
        }
        self.line_start = false;
        let value = self.text[start + 1..self.at].to_owned();
        self.push(TokenKind::Ref, start, value);
    }

    fn quoted(&mut self) {
        let start = self.at;
        if self.text[self.at..].starts_with("\"\"\"") {
            self.prose(start);
        } else {
            self.string(start);
        }
    }

    fn string(&mut self, start: usize) {
        self.at += 1; // opening quote
        let mut value = String::new();
        loop {
            let Some(byte) = self.peek() else {
                let span = self.span(start);
                self.error(c::P013, span, "unterminated quoted string");
                break;
            };
            match byte {
                b'"' => {
                    self.at += 1;
                    break;
                }
                b'\n' => {
                    // Report once and swallow what the author meant to be one string,
                    // up to the next quote. Reporting every following token would bury
                    // the real fault under recovery noise.
                    let span = self.span(start);
                    self.error(
                        c::P011,
                        span,
                        "unescaped newline in a quoted string; use a prose block",
                    );
                    while let Some(byte) = self.peek() {
                        self.at += 1;
                        if byte == b'\n' {
                            self.line += 1;
                        }
                        if byte == b'"' {
                            break;
                        }
                    }
                    break;
                }
                b'\\' => {
                    let escape_start = self.at;
                    self.at += 1;
                    match self.peek() {
                        Some(b'"') => value.push('"'),
                        Some(b'\\') => value.push('\\'),
                        Some(b'n') => value.push('\n'),
                        Some(b't') => value.push('\t'),
                        Some(b'r') => value.push('\r'),
                        Some(b'u') => {
                            self.at += 1;
                            if self.peek() == Some(b'{') {
                                self.at += 1;
                                let hex_start = self.at;
                                while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                                    self.at += 1;
                                }
                                let hex = &self.text[hex_start..self.at];
                                let ok = (1..=6).contains(&hex.len())
                                    && self.peek() == Some(b'}')
                                    && u32::from_str_radix(hex, 16)
                                        .ok()
                                        .and_then(char::from_u32)
                                        .inspect(|ch| value.push(*ch))
                                        .is_some();
                                if !ok {
                                    let span = self.span(escape_start);
                                    self.error(c::P012, span, "malformed \\u{...} escape");
                                }
                            } else {
                                let span = self.span(escape_start);
                                self.error(c::P012, span, "malformed \\u escape");
                                continue;
                            }
                        }
                        _ => {
                            let span = self.span(escape_start);
                            let found = self.text[self.at..]
                                .chars()
                                .next()
                                .map_or_else(|| "end of input".to_owned(), |ch| format!("\\{ch}"));
                            self.error(
                                c::P012,
                                span,
                                format!(
                                    "unknown escape {found}; legal escapes are \\\" \\\\ \\n \\t \\r \\u{{...}}"
                                ),
                            );
                        }
                    }
                    self.advance_char();
                }
                _ => {
                    let ch_start = self.at;
                    self.advance_char();
                    value.push_str(&self.text[ch_start..self.at]);
                }
            }
        }
        self.line_start = false;
        self.push(TokenKind::Str, start, value);
    }

    fn prose(&mut self, start: usize) {
        self.at += 3;
        // Rule 1: content begins on the line after the opening delimiter.
        let rest = &self.text[self.at..];
        let after_open = rest.find('\n');
        let Some(newline) = after_open else {
            let span = self.span(start);
            self.error(c::P014, span, "unterminated prose block");
            self.at = self.src.len();
            self.push(TokenKind::Prose, start, String::new());
            return;
        };
        if !rest[..newline].trim().is_empty() {
            let span = Span {
                file: self.file,
                start: u32::try_from(start).unwrap_or(u32::MAX),
                end: u32::try_from(self.at + newline).unwrap_or(u32::MAX),
            };
            self.error(
                c::P016,
                span,
                "prose block content must begin on the line after `\"\"\"`",
            );
        }
        self.at += newline + 1;
        self.line += 1;

        let mut lines: Vec<&str> = Vec::new();
        let mut closed = false;
        let mut tab_line: Option<u32> = None;
        loop {
            let remainder = &self.text[self.at..];
            if remainder.is_empty() {
                break;
            }
            let end = remainder.find('\n').unwrap_or(remainder.len());
            let line = &remainder[..end];
            if line.trim() == "\"\"\"" {
                if line.trim_start().len() != 3 {
                    // Content shares the closing line.
                }
                self.at += line.len();
                if self.at < self.src.len() {
                    // leave the newline for the main loop
                }
                closed = true;
                break;
            }
            if line.trim_end().len() != line.trim_start_matches(' ').trim_end().len()
                && line.starts_with('\t')
            {
                tab_line = Some(self.line);
            }
            if line
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .any(|b| b == b'\t')
            {
                tab_line = Some(self.line);
            }
            lines.push(line);
            self.at += end;
            if self.at < self.src.len() {
                self.at += 1;
                self.line += 1;
            }
        }

        if let Some(line) = tab_line {
            let span = self.span(start);
            self.error(
                c::P015,
                span,
                format!("tab character in prose indentation at line {line}"),
            );
        }
        if !closed {
            let span = self.span(start);
            self.error(c::P014, span, "unterminated prose block");
        }

        let value = dedent(&lines);
        self.line_start = false;
        self.push(TokenKind::Prose, start, value);
    }
}

/// Applies the D-007 dedent: strip trailing whitespace, remove the common leading-space
/// prefix of non-blank lines, and drop leading and trailing blank lines.
#[must_use]
pub fn dedent(lines: &[&str]) -> String {
    let trimmed: Vec<&str> = lines.iter().map(|l| l.trim_end()).collect();
    let common = trimmed
        .iter()
        .filter(|l| !l.is_empty())
        .map(|l| l.len() - l.trim_start_matches(' ').len())
        .min()
        .unwrap_or(0);
    let mut out: Vec<&str> = trimmed
        .iter()
        .map(|l| if l.is_empty() { "" } else { &l[common..] })
        .collect();
    while out.first().is_some_and(|l| l.is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out.join("\n")
}
