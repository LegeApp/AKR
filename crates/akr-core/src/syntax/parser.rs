//! A hand-written recursive-descent parser.
//!
//! The grammar is LL(1) after the header (`docs/03` §5): every construct is decided by
//! its first token, and a name followed by `{` is a block while a name followed by
//! anything else is a slot. There is no backtracking.
//!
//! Value *types* are not checked here. A `date` slot given a string parses fine and
//! fails at stage B with a better message than a parse error could give (`docs/03` §5).

use super::cst::{Block, BodyItem, Comment, File, Item, Namespace, Record, Slot, Trivia, Value};
use super::lexer::{Token, TokenKind, lex};
use crate::diagnostics::{Code, Diagnostic, FileId, Label, Severity, Span, Subject, codes as c};

/// The outcome of parsing.
#[derive(Debug)]
pub struct Parsed {
    /// The tree, when the header parsed. Later errors still produce a partial tree.
    pub file: Option<File>,
    /// Everything that went wrong, in source order.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a source file.
#[must_use]
pub fn parse(text: &str, file: FileId) -> Parsed {
    let lexed = lex(text, file);
    let (tokens, leading, trailing) = attach(lexed.tokens);
    let mut parser = Parser {
        tokens,
        leading,
        trailing,
        at: 0,
        file,
        len: u32::try_from(text.len()).unwrap_or(u32::MAX),
        diagnostics: lexed.diagnostics,
        text,
    };
    let parsed = parser.file();
    Parser::sort_diagnostics(&mut parser.diagnostics);
    Parsed {
        file: parsed,
        diagnostics: parser.diagnostics,
    }
}

/// Splits comments out of the token stream and attaches them (D-006).
///
/// A comment on its own line attaches as leading trivia to the next token; a comment
/// after a value on the same line attaches as trailing trivia to the token before it.
/// Attachment is total, which is what makes round-tripping well defined.
fn attach(tokens: Vec<Token>) -> (Vec<Token>, Vec<Vec<Comment>>, Vec<Option<Comment>>) {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    let mut leading: Vec<Vec<Comment>> = Vec::with_capacity(tokens.len());
    let mut trailing: Vec<Option<Comment>> = Vec::with_capacity(tokens.len());
    let mut pending: Vec<Comment> = Vec::new();
    let mut last_line: u32 = 0;

    for token in tokens {
        if token.kind == TokenKind::Comment {
            let comment = Comment {
                text: token.value,
                span: token.span,
                blank_before: token.blank_before,
            };
            if !out.is_empty() && token.line == last_line && pending.is_empty() {
                let index = out.len() - 1;
                trailing[index] = Some(comment);
            } else {
                pending.push(comment);
            }
        } else {
            last_line = token.line;
            out.push(token);
            leading.push(std::mem::take(&mut pending));
            trailing.push(None);
        }
    }
    // Anything left belongs to the end of the file; the Eof token carries it.
    if let Some(slot) = leading.last_mut() {
        slot.extend(pending);
    }
    (out, leading, trailing)
}

struct Parser<'a> {
    tokens: Vec<Token>,
    leading: Vec<Vec<Comment>>,
    trailing: Vec<Option<Comment>>,
    at: usize,
    file: FileId,
    len: u32,
    diagnostics: Vec<Diagnostic>,
    text: &'a str,
}

impl<'a> Parser<'a> {
    fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
        diagnostics.sort_by_key(|d| {
            d.primary
                .span
                .map_or((u32::MAX, u32::MAX), |s| (s.start, s.end))
        });
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.at.min(self.tokens.len() - 1)]
    }

    fn kind(&self) -> TokenKind {
        self.peek().kind
    }

    fn at_end(&self) -> bool {
        self.kind() == TokenKind::Eof
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.at.min(self.tokens.len() - 1)].clone();
        if self.at < self.tokens.len() - 1 {
            self.at += 1;
        }
        token
    }

    fn take_leading(&mut self) -> Vec<Comment> {
        let index = self.at.min(self.leading.len() - 1);
        std::mem::take(&mut self.leading[index])
    }

    fn take_trailing_of(&mut self, index: usize) -> Option<Comment> {
        self.trailing.get_mut(index).and_then(Option::take)
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

    fn expect(&mut self, kind: TokenKind, what: &str) -> Option<Token> {
        if self.kind() == kind {
            Some(self.bump())
        } else {
            let token = self.peek().clone();
            let found = describe(&token);
            self.error(
                c::P001,
                token.span,
                format!("expected {what}, found {found}"),
            );
            None
        }
    }

    fn span_to(&self, start: Span) -> Span {
        let end = self.tokens[self.at.saturating_sub(1).min(self.tokens.len() - 1)]
            .span
            .end;
        Span {
            file: self.file,
            start: start.start,
            end,
        }
    }

    // -- file ---------------------------------------------------------------------

    fn file(&mut self) -> Option<File> {
        if self.text.starts_with('\u{feff}') {
            let span = Span {
                file: self.file,
                start: 0,
                end: 3,
            };
            self.error(c::P002, span, "file begins with a UTF-8 byte order mark");
        }
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            let span = Span {
                file: self.file,
                start: self.len,
                end: self.len,
            };
            self.error(c::P004, span, "file does not end with a newline");
        }

        let leading = self.take_leading();
        let header_token = self.peek().clone();
        let blank_before_header = header_token.blank_before;
        if !(header_token.is_word("akr") || header_token.is_word("akr-lock")) {
            self.error(
                c::P005,
                header_token.span,
                "expected `akr` or `akr-lock` header on the first non-comment line",
            );
            return None;
        }
        let keyword = self.bump().value;
        let version = match self.kind() {
            TokenKind::Scalar => self.bump().value,
            _ => {
                let span = self.peek().span;
                self.error(c::P005, span, "expected a grammar version after the header");
                return None;
            }
        };
        self.check_version(&version, header_token.span);

        self.take_leading();
        if !self.peek().is_word("project") {
            let span = self.peek().span;
            self.error(c::P008, span, "expected `project <name>` after the header");
            return None;
        }
        self.bump();
        let project = match self.kind() {
            TokenKind::Word => self.bump().value,
            _ => {
                let span = self.peek().span;
                self.error(c::P008, span, "expected a project name");
                return None;
            }
        };

        let mut items = Vec::new();
        while !self.at_end() {
            let before = self.at;
            if let Some(item) = self.item() {
                items.push(item);
            }
            if self.at == before {
                let token = self.bump();
                self.error(
                    c::P001,
                    token.span,
                    format!("unexpected {}", describe(&token)),
                );
            }
        }
        let trailing = self.take_leading();
        if items.is_empty() && trailing.is_empty() {
            let span = Span {
                file: self.file,
                start: 0,
                end: self.len,
            };
            self.diagnostics.push(Diagnostic {
                code: c::P009,
                severity: Severity::Warning,
                rule: None,
                message: "file contains no records".to_owned(),
                primary: Label {
                    subject: Subject::Ledger,
                    span: Some(span),
                    message: None,
                },
                notes: Vec::new(),
                help: None,
            });
        }

        Some(File {
            leading,
            keyword,
            version,
            blank_before_header,
            project,
            items,
            trailing,
            span: Span {
                file: self.file,
                start: 0,
                end: self.len,
            },
        })
    }

    fn check_version(&mut self, version: &str, span: Span) {
        let Some((major, _)) = version.split_once('.') else {
            self.error(
                c::P005,
                span,
                format!("malformed grammar version {version:?}"),
            );
            return;
        };
        if major != "0" {
            self.error(
                c::P006,
                span,
                format!("grammar version {version} is not supported by akr 0.1"),
            );
        } else if version != "0.1" {
            self.diagnostics.push(Diagnostic {
                code: c::P007,
                severity: Severity::Warning,
                rule: None,
                message: format!("grammar version {version} is newer than 0.1; parsing as 0.1"),
                primary: Label {
                    subject: Subject::Ledger,
                    span: Some(span),
                    message: None,
                },
                notes: Vec::new(),
                help: None,
            });
        }
    }

    // -- items --------------------------------------------------------------------

    fn item(&mut self) -> Option<Item> {
        let leading = self.take_leading();
        let token = self.peek().clone();
        if token.is_word("record") {
            return self.record(leading).map(Item::Record);
        }
        if token.is_word("namespace") {
            return self.namespace(leading).map(Item::Namespace);
        }
        if token.kind == TokenKind::Word {
            return self.block(leading).map(Item::Block);
        }
        None
    }

    fn namespace(&mut self, leading: Vec<Comment>) -> Option<Namespace> {
        let start = self.bump().span;
        let name = self.expect(TokenKind::Word, "a namespace segment")?.value;
        let description = self
            .expect(TokenKind::Str, "a namespace description")?
            .value;
        let index = self.at - 1;
        let trailing = self.take_trailing_of(index);
        Some(Namespace {
            trivia: Trivia { leading, trailing },
            name,
            description,
            span: self.span_to(start),
        })
    }

    fn record(&mut self, leading: Vec<Comment>) -> Option<Record> {
        let start = self.bump().span;
        let key_token = self.expect(TokenKind::Word, "a record key")?;
        self.check_key(&key_token);
        self.expect(TokenKind::Slash, "`/` and a revision number")?;
        let revision_token = self.expect(TokenKind::Scalar, "a revision number")?;
        let revision = self.revision(&revision_token);
        self.expect(TokenKind::Colon, "`:` and a kind")?;
        let kind_token = self.expect(TokenKind::Word, "a record kind")?;
        let (body, inner_trailing) = self.body()?;
        Some(Record {
            trivia: Trivia {
                leading,
                trailing: None,
            },
            key: key_token.value,
            key_span: key_token.span,
            revision,
            kind: kind_token.value,
            kind_span: kind_token.span,
            body,
            inner_trailing,
            span: self.span_to(start),
        })
    }

    fn revision(&mut self, token: &Token) -> u32 {
        let text = &token.value;
        if text.len() > 1 && text.starts_with('0') {
            self.error(
                c::P025,
                token.span,
                "revision must be a positive integer without a leading zero",
            );
        }
        match text.parse::<u32>() {
            Ok(0) | Err(_) => {
                self.error(c::P025, token.span, "revision must be a positive integer");
                1
            }
            Ok(n) => n,
        }
    }

    fn body(&mut self) -> Option<(Vec<BodyItem>, Vec<Comment>)> {
        let open = self.expect(TokenKind::LBrace, "`{`")?;
        let mut items = Vec::new();
        let mut seen_slots: Vec<String> = Vec::new();
        let mut seen_heads: Vec<(String, String)> = Vec::new();
        loop {
            let leading = self.take_leading();
            if self.kind() == TokenKind::RBrace {
                self.bump();
                return Some((items, leading));
            }
            if self.at_end() {
                self.error(
                    c::P044,
                    open.span,
                    "unclosed `{`; the record or block never closes",
                );
                return Some((items, leading));
            }
            let before = self.at;
            if let Some(item) = self.body_item(leading) {
                self.check_uniqueness(&item, &mut seen_slots, &mut seen_heads);
                items.push(item);
            }
            if self.at == before {
                let token = self.bump();
                self.error(
                    c::P001,
                    token.span,
                    format!("expected a slot or block name, found {}", describe(&token)),
                );
            }
        }
    }

    /// Records are addressed by key, so a malformed one is caught before anything else
    /// tries to resolve it.
    fn check_key(&mut self, token: &Token) {
        use crate::model::IdentError;
        match crate::model::LogicalKey::parse(&token.value) {
            Ok(_) => {}
            Err(IdentError::BadKeyLength(n)) => self.error(
                c::P042,
                token.span,
                format!("key must have 2 to 8 segments, found {n}"),
            ),
            Err(error) => self.error(c::P041, token.span, error.to_string()),
        }
    }

    /// Slots are unique within a body; only `claim`, `check`, `source` and `disposition`
    /// repeat, and those must have distinct heads (D-012).
    fn check_uniqueness(
        &mut self,
        item: &BodyItem,
        seen_slots: &mut Vec<String>,
        seen_heads: &mut Vec<(String, String)>,
    ) {
        let name = item.name().to_owned();
        let repeatable = matches!(name.as_str(), "claim" | "check" | "source" | "disposition");
        if repeatable {
            let head = item.head_text();
            if head.is_empty() {
                return;
            }
            let entry = (name.clone(), head.clone());
            if seen_heads.contains(&entry) {
                self.error(
                    c::P032,
                    item.span(),
                    format!("{name} `{head}` appears twice"),
                );
            } else {
                seen_heads.push(entry);
            }
        } else if seen_slots.contains(&name) {
            self.error(
                c::P031,
                item.span(),
                format!("slot `{name}` appears twice in this record or block"),
            );
        } else {
            seen_slots.push(name);
        }
    }

    fn body_item(&mut self, leading: Vec<Comment>) -> Option<BodyItem> {
        if self.kind() != TokenKind::Word {
            return None;
        }
        // One token of lookahead decides slot versus block (`docs/03` §5).
        let next = self.tokens.get(self.at + 1).map(|t| t.kind);
        let after_head = self.tokens.get(self.at + 2).map(|t| t.kind);
        let is_block = next == Some(TokenKind::LBrace)
            || (matches!(
                next,
                Some(TokenKind::Word | TokenKind::Ref | TokenKind::Str)
            ) && after_head == Some(TokenKind::LBrace));
        if is_block {
            self.block(leading).map(BodyItem::Block)
        } else {
            self.slot(leading).map(BodyItem::Slot)
        }
    }

    fn slot(&mut self, leading: Vec<Comment>) -> Option<Slot> {
        let name_token = self.bump();
        let value = self.value()?;
        let index = self.at - 1;
        let trailing = self.take_trailing_of(index);
        Some(Slot {
            trivia: Trivia { leading, trailing },
            name: name_token.value,
            name_span: name_token.span,
            value,
            span: self.span_to(name_token.span),
        })
    }

    fn block(&mut self, leading: Vec<Comment>) -> Option<Block> {
        let name_token = self.bump();
        let head = if self.kind() == TokenKind::LBrace {
            None
        } else {
            Some(self.value()?)
        };
        let (body, inner_trailing) = self.body()?;
        Some(Block {
            trivia: Trivia {
                leading,
                trailing: None,
            },
            name: name_token.value,
            name_span: name_token.span,
            head,
            body,
            inner_trailing,
            span: self.span_to(name_token.span),
        })
    }

    // -- values -------------------------------------------------------------------

    fn value(&mut self) -> Option<Value> {
        match self.kind() {
            TokenKind::Word => {
                let token = self.bump();
                // `ref @key` and `path "glob"` are scope terms (D-010).
                if (token.value == "ref" && self.kind() == TokenKind::Ref)
                    || (token.value == "path" && self.kind() == TokenKind::Str)
                {
                    let inner = self.value()?;
                    let span = Span {
                        file: self.file,
                        start: token.span.start,
                        end: inner.span().end,
                    };
                    return Some(Value::Prefixed(token.value, Box::new(inner), span));
                }
                Some(Value::Word(token.value, token.span))
            }
            TokenKind::Scalar => {
                let token = self.bump();
                self.check_scalar(&token);
                Some(Value::Scalar(token.value, token.span))
            }
            TokenKind::Commit => {
                let token = self.bump();
                Some(Value::Commit(token.value, token.span))
            }
            TokenKind::Str => {
                let token = self.bump();
                Some(Value::Str(token.value, token.span))
            }
            TokenKind::Prose => {
                let token = self.bump();
                Some(Value::Prose(token.value, token.span))
            }
            TokenKind::Ref => {
                let token = self.bump();
                self.check_reference(&token);
                Some(Value::Ref(token.value, token.span))
            }
            TokenKind::LBracket => self.array(),
            _ => {
                let token = self.peek().clone();
                self.error(
                    c::P001,
                    token.span,
                    format!("expected a value, found {}", describe(&token)),
                );
                None
            }
        }
    }

    fn array(&mut self) -> Option<Value> {
        let open = self.bump();
        let mut items = Vec::new();
        loop {
            self.take_leading();
            match self.kind() {
                TokenKind::RBracket => {
                    self.bump();
                    let span = Span {
                        file: self.file,
                        start: open.span.start,
                        end: self.tokens[self.at - 1].span.end,
                    };
                    return Some(Value::Array(items, span));
                }
                TokenKind::Eof => {
                    self.error(c::P045, open.span, "unclosed `[`");
                    return Some(Value::Array(items, open.span));
                }
                TokenKind::Comma => {
                    self.bump();
                }
                _ => {
                    let before = self.at;
                    if let Some(value) = self.value() {
                        items.push(value);
                    }
                    if self.at == before {
                        self.bump();
                    }
                }
            }
        }
    }

    fn check_scalar(&mut self, token: &Token) {
        let text = &token.value;
        let is_digits = text.bytes().all(|b| b.is_ascii_digit());
        if is_digits && text.len() > 1 && text.starts_with('0') {
            self.error(
                c::P024,
                token.span,
                "integer literal may not have a leading zero",
            );
            return;
        }
        if is_digits || text.parse::<i64>().is_ok() {
            return;
        }
        if text.contains('T') || text.contains('+') {
            if !(text.len() == 20 && text.ends_with('Z')) {
                self.error(
                    c::P023,
                    token.span,
                    "timestamp must end in `Z`; offsets are not permitted",
                );
                return;
            }
            if crate::model::Date::parse(&text[..10]).is_err() {
                self.error(
                    c::P022,
                    token.span,
                    format!("{text} is not a valid calendar date"),
                );
            }
            return;
        }
        if text.matches('-').count() == 2 {
            if crate::model::Date::parse(text).is_err() {
                self.error(
                    c::P022,
                    token.span,
                    format!("{text} is not a valid calendar date"),
                );
            }
            return;
        }
        if text.matches('.').count() == 1
            && text
                .split('.')
                .all(|p| p.bytes().all(|b| b.is_ascii_digit()))
        {
            return; // a version, only legal in the header
        }
        self.error(
            c::P001,
            token.span,
            format!("{text} is not a valid literal"),
        );
    }

    fn check_reference(&mut self, token: &Token) {
        if crate::model::Reference::parse(&token.value).is_err() {
            self.error(
                c::P043,
                token.span,
                "reference must be @key[/revision][#anchor]".to_owned(),
            );
        }
    }
}

fn describe(token: &Token) -> String {
    match token.kind {
        TokenKind::Eof => "end of file".to_owned(),
        TokenKind::Prose => "a prose block".to_owned(),
        TokenKind::Str => "a string".to_owned(),
        _ => format!("`{}`", token.value),
    }
}
