//! A minimal JSON writer, for the `--format json` envelope of `docs/07-cli.md` §5.
//!
//! # Why hand-rolled
//!
//! The envelope's contract says "object keys are emitted in a fixed order", which a
//! serialiser driven by a map type cannot promise and a derive macro promises only by
//! accident of field order. [`Value::Object`] is a `Vec` of pairs, so the order is the
//! order the caller wrote — which is the property the specification actually asks for.
//!
//! It is also one fewer dependency, and this crate has kept its dependency list empty
//! since P1 (`docs/13-implementation-roadmap.md` §4). Nothing here is general-purpose:
//! there is no parser, no `Deserialize`, and no attempt at a public data-interchange API.

use std::fmt::Write as _;

/// A JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A number, written as an integer.
    Integer(i64),
    /// A string.
    String(String),
    /// An array.
    Array(Vec<Value>),
    /// An object. **Order is preserved**, which is the whole point.
    Object(Vec<(String, Value)>),
}

impl Value {
    /// A string value.
    #[must_use]
    pub fn string(text: impl Into<String>) -> Self {
        Self::String(text.into())
    }

    /// An integer value.
    #[must_use]
    pub const fn integer(value: i64) -> Self {
        Self::Integer(value)
    }

    /// A boolean value.
    #[must_use]
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// An array value.
    #[must_use]
    pub const fn array(items: Vec<Value>) -> Self {
        Self::Array(items)
    }

    /// An object value, from ordered pairs.
    #[must_use]
    pub fn object(fields: Vec<(&str, Value)>) -> Self {
        Self::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }

    /// Renders with two-space indentation and a trailing newline.
    #[must_use]
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out.push('\n');
        out
    }

    fn write(&self, out: &mut String, depth: usize) {
        let pad = "  ".repeat(depth);
        let inner = "  ".repeat(depth + 1);
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Self::Integer(value) => {
                let _ = write!(out, "{value}");
            }
            Self::String(text) => write_string(out, text),
            Self::Array(items) if items.is_empty() => out.push_str("[]"),
            Self::Array(items) => {
                out.push_str("[\n");
                for (at, item) in items.iter().enumerate() {
                    out.push_str(&inner);
                    item.write(out, depth + 1);
                    if at + 1 < items.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push(']');
            }
            Self::Object(fields) if fields.is_empty() => out.push_str("{}"),
            Self::Object(fields) => {
                out.push_str("{\n");
                for (at, (key, value)) in fields.iter().enumerate() {
                    out.push_str(&inner);
                    write_string(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                    if at + 1 < fields.len() {
                        out.push(',');
                    }
                    out.push('\n');
                }
                out.push_str(&pad);
                out.push('}');
            }
        }
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
