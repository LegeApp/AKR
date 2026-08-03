//! The concrete syntax tree: every token that was written, with byte spans and comments.
//!
//! The CST is what the formatter prints and what lowering converts into
//! [`crate::model`] types. It keeps comments (D-006) because the formatter must
//! re-emit them, and it keeps spans because diagnostics must point at them.

use crate::diagnostics::Span;

/// A `#` comment, with the attachment information the formatter needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The text after `#`, trimmed.
    pub text: String,
    /// Where it is.
    pub span: Span,
    /// Whether a blank line preceded it. Only meaningful for leading comments.
    pub blank_before: bool,
}

/// Comments attached to an item (D-006).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trivia {
    /// Own-line comments above the item, in order.
    pub leading: Vec<Comment>,
    /// A comment after the item's value on the same line.
    pub trailing: Option<Comment>,
}

impl Trivia {
    /// Whether the leading group had a blank line above it.
    #[must_use]
    pub fn blank_before(&self) -> bool {
        self.leading.first().is_some_and(|c| c.blank_before)
    }
}

/// A parsed file.
#[derive(Debug, Clone)]
pub struct File {
    /// Comments before the header, which belong to the file.
    pub leading: Vec<Comment>,
    /// `akr` or `akr-lock`.
    pub keyword: String,
    /// The grammar version, as written.
    pub version: String,
    /// Whether a blank line separated the leading comments from the header.
    pub blank_before_header: bool,
    /// The project name.
    pub project: String,
    /// Top-level items.
    pub items: Vec<Item>,
    /// Comments at the end of the file with nothing following them.
    pub trailing: Vec<Comment>,
    /// The whole file.
    pub span: Span,
}

/// A top-level item.
#[derive(Debug, Clone)]
pub enum Item {
    /// A record.
    Record(Record),
    /// A `namespace` declaration in a project file.
    Namespace(Namespace),
    /// Any other top-level block: `defaults`, and the lock file's items.
    Block(Block),
}

impl Item {
    /// The item's trivia.
    #[must_use]
    pub fn trivia(&self) -> &Trivia {
        match self {
            Self::Record(r) => &r.trivia,
            Self::Namespace(n) => &n.trivia,
            Self::Block(b) => &b.trivia,
        }
    }

    /// A sort key: records by key then revision, everything else stable in source order.
    #[must_use]
    pub fn sort_key(&self) -> Option<(String, u32)> {
        match self {
            Self::Record(r) => Some((r.key.clone(), r.revision)),
            _ => None,
        }
    }
}

/// A record.
#[derive(Debug, Clone)]
pub struct Record {
    /// Attached comments.
    pub trivia: Trivia,
    /// The key, as written.
    pub key: String,
    /// Where the key is.
    pub key_span: Span,
    /// The revision number.
    pub revision: u32,
    /// The kind, as written.
    pub kind: String,
    /// Where the kind is.
    pub kind_span: Span,
    /// The body.
    pub body: Vec<BodyItem>,
    /// Comments at the end of the body.
    pub inner_trailing: Vec<Comment>,
    /// The whole record, from `record` to its closing brace.
    pub span: Span,
}

/// A `namespace` declaration.
#[derive(Debug, Clone)]
pub struct Namespace {
    /// Attached comments.
    pub trivia: Trivia,
    /// The namespace segment.
    pub name: String,
    /// Its description.
    pub description: String,
    /// The whole declaration.
    pub span: Span,
}

/// A slot or a block.
#[derive(Debug, Clone)]
pub enum BodyItem {
    /// `name value`.
    Slot(Slot),
    /// `name [head] { ... }`.
    Block(Block),
}

impl BodyItem {
    /// The item's name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Slot(s) => &s.name,
            Self::Block(b) => &b.name,
        }
    }

    /// The item's trivia.
    #[must_use]
    pub fn trivia(&self) -> &Trivia {
        match self {
            Self::Slot(s) => &s.trivia,
            Self::Block(b) => &b.trivia,
        }
    }

    /// The item's span.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Slot(s) => s.span,
            Self::Block(b) => b.span,
        }
    }

    /// The block head, rendered, for sorting blocks by head (D-012).
    #[must_use]
    pub fn head_text(&self) -> String {
        match self {
            Self::Block(b) => b
                .head
                .as_ref()
                .map(Value::render_inline)
                .unwrap_or_default(),
            Self::Slot(_) => String::new(),
        }
    }
}

/// `name value`.
#[derive(Debug, Clone)]
pub struct Slot {
    /// Attached comments.
    pub trivia: Trivia,
    /// The slot name.
    pub name: String,
    /// Where the name is.
    pub name_span: Span,
    /// The value.
    pub value: Value,
    /// The whole slot.
    pub span: Span,
}

/// `name [head] { ... }`.
#[derive(Debug, Clone)]
pub struct Block {
    /// Attached comments.
    pub trivia: Trivia,
    /// The block name.
    pub name: String,
    /// Where the name is.
    pub name_span: Span,
    /// The head, for `claim`, `check`, `disposition` and lock items.
    pub head: Option<Value>,
    /// The contents.
    pub body: Vec<BodyItem>,
    /// Comments at the end of the body.
    pub inner_trailing: Vec<Comment>,
    /// The whole block.
    pub span: Span,
}

/// A value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A bare word: enum member, `true`, `false`, `all`, an identifier.
    Word(String, Span),
    /// A literal beginning with a digit: date, timestamp, integer, version.
    Scalar(String, Span),
    /// `git:` and 40 hex digits.
    Commit(String, Span),
    /// A quoted string, decoded.
    Str(String, Span),
    /// A prose block, dedented.
    Prose(String, Span),
    /// A reference, without the `@`.
    Ref(String, Span),
    /// `[ ... ]`.
    Array(Vec<Value>, Span),
    /// A prefixed value: `ref @key` and `path "glob"` in a scope array.
    Prefixed(String, Box<Value>, Span),
}

impl Value {
    /// Where the value is.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Word(_, s)
            | Self::Scalar(_, s)
            | Self::Commit(_, s)
            | Self::Str(_, s)
            | Self::Prose(_, s)
            | Self::Ref(_, s)
            | Self::Array(_, s)
            | Self::Prefixed(_, _, s) => *s,
        }
    }

    /// The value as one line of canonical source. Prose has no inline form and renders
    /// as an empty string; the formatter handles it separately.
    #[must_use]
    pub fn render_inline(&self) -> String {
        match self {
            Self::Word(w, _) | Self::Scalar(w, _) => w.clone(),
            Self::Commit(hex, _) => format!("git:{hex}"),
            Self::Str(text, _) => format!("\"{}\"", escape(text)),
            Self::Prose(_, _) => String::new(),
            Self::Ref(body, _) => format!("@{body}"),
            Self::Array(items, _) => {
                let inner: Vec<String> = items.iter().map(Value::render_inline).collect();
                if inner.is_empty() {
                    "[ ]".to_owned()
                } else {
                    format!("[ {} ]", inner.join(", "))
                }
            }
            Self::Prefixed(word, inner, _) => format!("{word} {}", inner.render_inline()),
        }
    }

    /// A sort key for array elements (D-012 step 10).
    #[must_use]
    pub fn sort_key(&self) -> (u8, String, u32, String) {
        match self {
            Self::Word(w, _) if w == "all" => (0, String::new(), 0, String::new()),
            Self::Prefixed(word, inner, _) => {
                let rank = if word == "ref" { 1 } else { 2 };
                let (_, key, revision, anchor) = inner.sort_key();
                (rank, key, revision, anchor)
            }
            Self::Ref(body, _) => {
                let (head, anchor) = body.split_once('#').unwrap_or((body.as_str(), ""));
                let (key, revision) = head.split_once('/').unwrap_or((head, ""));
                (
                    1,
                    key.to_owned(),
                    revision.parse().unwrap_or(0),
                    anchor.to_owned(),
                )
            }
            other => (3, other.render_inline(), 0, String::new()),
        }
    }
}

/// Escapes a string for canonical output (D-007).
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}
