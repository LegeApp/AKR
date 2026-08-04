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
//! there is no `Deserialize`, no reflection, and no attempt at a public data-interchange
//! API. A [`parse`] arrived with P6c, because `akr-mcp` has to read JSON-RPC as well as
//! write it; its two deliberate narrowings are documented on the function.

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

// -------------------------------------------------------------------------------------
// Parsing
// -------------------------------------------------------------------------------------

impl Value {
    /// The value of an object key, or `None`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The string this value holds, or `None`.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// The integer this value holds, or `None`.
    #[must_use]
    pub const fn as_integer(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// The boolean this value holds, or `None`.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// The array this value holds, or `None`.
    #[must_use]
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Whether this is `null` — which a JSON-RPC caller uses to mean "not supplied".
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// What went wrong while parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// A one-line description.
    pub message: String,
    /// The byte offset the parser stopped at.
    pub offset: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}

/// Parses a JSON document.
///
/// Written because `akr-mcp` has to *read* JSON-RPC as well as write it, and the crate's
/// dependency list has been empty since P1 (`docs/13-implementation-roadmap.md` §4). It is
/// deliberately small: it accepts RFC 8259 with two documented narrowings, and rejects
/// anything else rather than guessing.
///
/// **Numbers become [`Value::Integer`].** A fractional or exponent form is an error. No
/// field of any AKR schema is fractional — token budgets, depths, limits and revision
/// numbers are all counts — and silently truncating a float would turn a caller's mistake
/// into a plausible wrong answer.
///
/// **Duplicate keys keep the last.** Which matches every mainstream parser, and matters
/// only for input nobody should be sending.
///
/// # Errors
/// [`ParseError`] for malformed input, with the offset it stopped at.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let bytes = text.as_bytes();
    let mut cursor = 0;
    let value = parse_value(bytes, &mut cursor)?;
    skip_whitespace(bytes, &mut cursor);
    if cursor != bytes.len() {
        return Err(error("trailing input after the document", cursor));
    }
    Ok(value)
}

fn error(message: &str, offset: usize) -> ParseError {
    ParseError {
        message: message.to_owned(),
        offset,
    }
}

fn skip_whitespace(bytes: &[u8], cursor: &mut usize) {
    while *cursor < bytes.len() && matches!(bytes[*cursor], b' ' | b'\t' | b'\n' | b'\r') {
        *cursor += 1;
    }
}

fn parse_value(bytes: &[u8], cursor: &mut usize) -> Result<Value, ParseError> {
    skip_whitespace(bytes, cursor);
    let Some(&byte) = bytes.get(*cursor) else {
        return Err(error("unexpected end of input", *cursor));
    };
    match byte {
        b'{' => parse_object(bytes, cursor),
        b'[' => parse_array(bytes, cursor),
        b'"' => parse_string(bytes, cursor).map(Value::String),
        b't' => literal(bytes, cursor, "true", Value::Bool(true)),
        b'f' => literal(bytes, cursor, "false", Value::Bool(false)),
        b'n' => literal(bytes, cursor, "null", Value::Null),
        b'-' | b'0'..=b'9' => parse_number(bytes, cursor),
        _ => Err(error("expected a value", *cursor)),
    }
}

fn literal(
    bytes: &[u8],
    cursor: &mut usize,
    word: &str,
    value: Value,
) -> Result<Value, ParseError> {
    if bytes[*cursor..].starts_with(word.as_bytes()) {
        *cursor += word.len();
        Ok(value)
    } else {
        Err(error("expected a literal", *cursor))
    }
}

fn parse_number(bytes: &[u8], cursor: &mut usize) -> Result<Value, ParseError> {
    let start = *cursor;
    if bytes.get(*cursor) == Some(&b'-') {
        *cursor += 1;
    }
    while bytes.get(*cursor).is_some_and(u8::is_ascii_digit) {
        *cursor += 1;
    }
    if *cursor == start || (bytes[start] == b'-' && *cursor == start + 1) {
        return Err(error("expected a digit", *cursor));
    }
    if matches!(bytes.get(*cursor), Some(b'.' | b'e' | b'E')) {
        return Err(error(
            "fractional and exponent numbers are not accepted; every numeric field is a count",
            *cursor,
        ));
    }
    std::str::from_utf8(&bytes[start..*cursor])
        .ok()
        .and_then(|text| text.parse().ok())
        .map(Value::Integer)
        .ok_or_else(|| error("number does not fit in a 64-bit integer", start))
}

fn parse_string(bytes: &[u8], cursor: &mut usize) -> Result<String, ParseError> {
    if bytes.get(*cursor) != Some(&b'"') {
        return Err(error("expected a string", *cursor));
    }
    *cursor += 1;
    let mut out = String::new();
    loop {
        let Some(&byte) = bytes.get(*cursor) else {
            return Err(error("unterminated string", *cursor));
        };
        match byte {
            b'"' => {
                *cursor += 1;
                return Ok(out);
            }
            b'\\' => {
                *cursor += 1;
                let Some(&escape) = bytes.get(*cursor) else {
                    return Err(error("unterminated escape", *cursor));
                };
                *cursor += 1;
                match escape {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{8}'),
                    b'f' => out.push('\u{c}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => out.push(parse_unicode_escape(bytes, cursor)?),
                    _ => return Err(error("unknown escape", *cursor)),
                }
            }
            _ => {
                // Multi-byte UTF-8 passes through unchanged: the input is already a `&str`,
                // so every sequence is valid and the only job is to find its end.
                let start = *cursor;
                let width = utf8_width(byte);
                *cursor += width;
                let Some(slice) = bytes.get(start..*cursor) else {
                    return Err(error("truncated UTF-8 sequence", start));
                };
                out.push_str(
                    std::str::from_utf8(slice).map_err(|_| error("invalid UTF-8", start))?,
                );
            }
        }
    }
}

const fn utf8_width(byte: u8) -> usize {
    match byte {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

fn parse_unicode_escape(bytes: &[u8], cursor: &mut usize) -> Result<char, ParseError> {
    let unit = hex4(bytes, cursor)?;
    // A surrogate pair is two escapes; a lone surrogate is an error rather than U+FFFD,
    // because silently substituting a replacement character corrupts a key.
    if (0xD800..0xDC00).contains(&unit) {
        if bytes.get(*cursor) != Some(&b'\\') || bytes.get(*cursor + 1) != Some(&b'u') {
            return Err(error("lone high surrogate", *cursor));
        }
        *cursor += 2;
        let low = hex4(bytes, cursor)?;
        if !(0xDC00..0xE000).contains(&low) {
            return Err(error("expected a low surrogate", *cursor));
        }
        let combined = 0x1_0000 + ((unit - 0xD800) << 10) + (low - 0xDC00);
        return char::from_u32(combined).ok_or_else(|| error("invalid surrogate pair", *cursor));
    }
    char::from_u32(unit).ok_or_else(|| error("invalid escape", *cursor))
}

fn hex4(bytes: &[u8], cursor: &mut usize) -> Result<u32, ParseError> {
    let slice = bytes
        .get(*cursor..*cursor + 4)
        .ok_or_else(|| error("truncated \\u escape", *cursor))?;
    let text = std::str::from_utf8(slice).map_err(|_| error("invalid \\u escape", *cursor))?;
    let value = u32::from_str_radix(text, 16).map_err(|_| error("invalid \\u escape", *cursor))?;
    *cursor += 4;
    Ok(value)
}

fn parse_array(bytes: &[u8], cursor: &mut usize) -> Result<Value, ParseError> {
    *cursor += 1;
    let mut items = Vec::new();
    skip_whitespace(bytes, cursor);
    if bytes.get(*cursor) == Some(&b']') {
        *cursor += 1;
        return Ok(Value::Array(items));
    }
    loop {
        items.push(parse_value(bytes, cursor)?);
        skip_whitespace(bytes, cursor);
        match bytes.get(*cursor) {
            Some(b',') => *cursor += 1,
            Some(b']') => {
                *cursor += 1;
                return Ok(Value::Array(items));
            }
            _ => return Err(error("expected `,` or `]`", *cursor)),
        }
    }
}

fn parse_object(bytes: &[u8], cursor: &mut usize) -> Result<Value, ParseError> {
    *cursor += 1;
    let mut fields: Vec<(String, Value)> = Vec::new();
    skip_whitespace(bytes, cursor);
    if bytes.get(*cursor) == Some(&b'}') {
        *cursor += 1;
        return Ok(Value::Object(fields));
    }
    loop {
        skip_whitespace(bytes, cursor);
        let key = parse_string(bytes, cursor)?;
        skip_whitespace(bytes, cursor);
        if bytes.get(*cursor) != Some(&b':') {
            return Err(error("expected `:`", *cursor));
        }
        *cursor += 1;
        let value = parse_value(bytes, cursor)?;
        fields.retain(|(existing, _)| existing != &key);
        fields.push((key, value));
        skip_whitespace(bytes, cursor);
        match bytes.get(*cursor) {
            Some(b',') => *cursor += 1,
            Some(b'}') => {
                *cursor += 1;
                return Ok(Value::Object(fields));
            }
            _ => return Err(error("expected `,` or `}`", *cursor)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, parse};

    #[test]
    fn round_trips_every_shape() {
        let document = Value::object(vec![
            ("null", Value::Null),
            ("yes", Value::bool(true)),
            ("no", Value::bool(false)),
            ("count", Value::integer(-42)),
            ("text", Value::string("a \"quoted\" line\nand a tab\t")),
            (
                "list",
                Value::array(vec![Value::integer(1), Value::string("two")]),
            ),
            ("nested", Value::object(vec![("deep", Value::integer(3))])),
        ]);
        let parsed = parse(&document.to_pretty()).expect("round trips");
        assert_eq!(parsed, document);
    }

    #[test]
    fn key_order_survives_a_round_trip() {
        // The property the envelope's contract actually asks for, and the reason this
        // module exists: a map-backed parser would return these sorted.
        let document = Value::object(vec![
            ("zeta", Value::integer(1)),
            ("alpha", Value::integer(2)),
            ("mu", Value::integer(3)),
        ]);
        let Value::Object(fields) = parse(&document.to_pretty()).expect("parses") else {
            panic!("expected an object");
        };
        let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["zeta", "alpha", "mu"]);
    }

    #[test]
    fn unicode_escapes_and_surrogate_pairs() {
        assert_eq!(
            parse(r#""Aé😀""#).expect("parses"),
            Value::string("Aé😀")
        );
        assert!(parse(r#""\ud83d""#).is_err(), "a lone surrogate is an error");
    }

    #[test]
    fn a_fractional_number_is_refused_rather_than_truncated() {
        // Every numeric field in every AKR schema is a count. Truncating 8000.5 to 8000
        // would turn a caller's mistake into a plausible wrong answer.
        let error = parse("{\"budget_tokens\": 8000.5}").expect_err("refuses");
        assert!(error.message.contains("count"), "{error}");
    }

    #[test]
    fn malformed_input_reports_where_it_stopped() {
        for text in ["{", "[1,]", "{\"a\" 1}", "tru", "\"unterminated", "1 2"] {
            assert!(parse(text).is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn accessors_read_what_the_parser_produced() {
        let value = parse(r#"{"a": {"b": [1, "two", true, null]}}"#).expect("parses");
        let inner = value.get("a").and_then(|v| v.get("b")).expect("nested");
        let items = inner.as_array().expect("an array");
        assert_eq!(items[0].as_integer(), Some(1));
        assert_eq!(items[1].as_str(), Some("two"));
        assert_eq!(items[2].as_bool(), Some(true));
        assert!(items[3].is_null());
        assert!(value.get("missing").is_none());
    }
}
