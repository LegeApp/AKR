//! Per-tool output budgets, and the accounting that says whether they are working.
//!
//! `sources/context-reduction.md`. An MCP tool result does not cost what it weighs once —
//! it stays in the conversation, so every later request in the session pays for it again.
//! A four-thousand-token result read twenty times later is eighty thousand token-turns of
//! exposure. Claude Code warns at ten thousand tokens and permits twenty-five; those
//! limits are right for an exceptional log retrieval and much too generous for routine
//! project state.
//!
//! So the limits live here instead, per tool, and they are deliberately small.
//!
//! # Truncation is honest or it does not happen
//!
//! A result over its hard limit is replaced by a summary and a cursor. It is not quietly
//! shortened, and it never claims to be complete: the context budget in `akr-core` was
//! once annotating prose as truncated while emitting all of it, and an agent that cannot
//! trust a truncation marker will simply ask for everything.

use akr_core::json::Value;

/// What a tool's output should cost, and what it must not exceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// What the result should come in under.
    pub target_tokens: usize,
    /// The point past which the result is replaced by a summary and a cursor.
    pub hard_tokens: usize,
}

impl Budget {
    const fn new(target_tokens: usize, hard_tokens: usize) -> Self {
        Self {
            target_tokens,
            hard_tokens,
        }
    }
}

/// The budget for a tool.
///
/// These are engineering targets rather than protocol requirements, and they are the
/// table from `sources/context-reduction.md` with one change: validation *failure* gets
/// more room than validation success, because a diagnostic nobody can read costs a whole
/// extra round trip.
#[must_use]
pub fn budget_for(tool: &str) -> Budget {
    match tool {
        "knowledge.start" => Budget::new(1_500, 2_000),
        "knowledge.search" | "knowledge.source_search" => Budget::new(600, 1_000),
        "knowledge.get" => Budget::new(1_000, 1_500),
        "knowledge.source_get" => Budget::new(1_500, 2_500),
        "knowledge.context" => Budget::new(3_000, 4_000),
        "knowledge.impact" => Budget::new(800, 1_500),
        "knowledge.explain" => Budget::new(800, 1_500),
        "knowledge.validate" => Budget::new(500, 3_000),
        // Every write tool. A successful write has nothing to say but where it landed.
        _ => Budget::new(300, 500),
    }
}

/// The crude token estimate the context budget uses: words plus punctuation.
///
/// The same estimator on both sides on purpose. Two estimators would disagree, and the
/// one that mattered would be whichever was wrong.
#[must_use]
pub fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace()
        .map(|word| 1 + word.chars().filter(|c| c.is_ascii_punctuation()).count() / 3)
        .sum()
}

/// What enforcement did to a result.
#[derive(Debug, Clone, PartialEq)]
pub struct Enforced {
    /// The text half, possibly replaced by a summary.
    pub text: Option<String>,
    /// The structured half, possibly replaced by a summary.
    pub structured: Value,
    /// Whether anything was withheld.
    pub truncated: bool,
    /// The estimate before enforcement.
    pub estimated_tokens: usize,
}

/// Applies `tool`'s budget to a result.
///
/// Under the hard limit, nothing happens — most calls are small and paying a rewrite for
/// them would be silly. Over it, both halves are replaced by a compact summary naming the
/// tool, the size that was refused and how to ask for less.
#[must_use]
pub fn enforce(
    tool: &str,
    text: Option<&str>,
    structured: &Value,
    requested_hard_tokens: Option<usize>,
) -> Enforced {
    let budget = budget_for(tool);
    // `knowledge.context` already assembled to the caller's explicit budget. Applying a
    // second ceiling afterwards both wastes that work and makes the input contract
    // untrue, especially because MCP duplicates some information across text and
    // structured halves. Other tools have no caller-selected budget and keep their fixed
    // engineering limit.
    let hard_tokens = requested_hard_tokens.map_or(budget.hard_tokens, |_| usize::MAX);
    let rendered = structured.to_pretty();
    // Both halves, because both reach the model: a client that shows `content` and a
    // client that parses `structuredContent` are each paying for their own half.
    let estimated = estimate_tokens(text.unwrap_or_default()) + estimate_tokens(&rendered);

    if estimated <= hard_tokens {
        return Enforced {
            text: text.map(ToOwned::to_owned),
            structured: structured.clone(),
            truncated: false,
            estimated_tokens: estimated,
        };
    }

    let advice = narrowing_advice(tool);
    let summary = format!(
        "{tool} produced about {estimated} tokens, over its {} limit, so it was withheld \
         rather than truncated.\n{advice}",
        hard_tokens
    );
    let structured = Value::object(vec![
        ("truncated", Value::bool(true)),
        ("tool", Value::string(tool.to_owned())),
        (
            "estimated_tokens",
            Value::integer(i64::try_from(estimated).unwrap_or(i64::MAX)),
        ),
        (
            "hard_limit_tokens",
            Value::integer(i64::try_from(hard_tokens).unwrap_or(i64::MAX)),
        ),
        ("continuation", narrowing_arguments(tool)),
        ("help", Value::string(advice)),
        // Whatever shape the caller expected, the top-level counts survive: a caller can
        // tell "nothing matched" from "too much matched" without a second call.
        ("counts", counts_of(structured)),
    ]);

    Enforced {
        text: Some(summary),
        structured,
        truncated: true,
        estimated_tokens: estimated,
    }
}

/// How to ask this tool for less. Concrete arguments, not "try narrowing your query".
fn narrowing_advice(tool: &str) -> String {
    match tool {
        "knowledge.search" | "knowledge.source_search" => {
            "Call again with a smaller `limit`, or narrow with `kinds`, `states` or \
             `documents`."
        }
        "knowledge.get" => "Call again with `detail: \"summary\"`, or with `relations: false`.",
        "knowledge.source_get" => {
            "Call again with `detail: \"snippet\"`, or name a `chunk` instead of a \
             document `id`."
        }
        "knowledge.context" => {
            "Call again with a smaller `budget_tokens`, or with `paths` narrowed to the \
             files you are about to touch."
        }
        "knowledge.impact" => "Call again with a smaller `depth`.",
        "knowledge.validate" => {
            "Call again with a smaller `limit`, or continue from `next_offset`."
        }
        _ => "Call again with narrower arguments.",
    }
    .to_owned()
}

/// A ready-made retry, so the next call is a copy rather than a guess.
fn narrowing_arguments(tool: &str) -> Value {
    let arguments = match tool {
        "knowledge.get" => vec![("detail", Value::string("summary"))],
        "knowledge.source_get" => vec![("detail", Value::string("snippet"))],
        "knowledge.search" | "knowledge.source_search" => vec![("limit", Value::integer(5))],
        "knowledge.context" => vec![("budget_tokens", Value::integer(2_000))],
        "knowledge.impact" => vec![("depth", Value::integer(1))],
        "knowledge.validate" => vec![("limit", Value::integer(3)), ("offset", Value::integer(0))],
        _ => return Value::Null,
    };
    Value::object(vec![
        ("tool", Value::string(tool.to_owned())),
        ("arguments", Value::object(arguments)),
    ])
}

/// The scalar and array-length fields of a payload, so a withheld result still counts.
fn counts_of(structured: &Value) -> Value {
    let Value::Object(fields) = structured else {
        return Value::Object(Vec::new());
    };
    let mut out = Vec::new();
    for (name, value) in fields {
        match value {
            Value::Array(items) => out.push((
                format!("{name}_count"),
                Value::integer(i64::try_from(items.len()).unwrap_or(i64::MAX)),
            )),
            Value::Integer(number) => out.push((name.clone(), Value::Integer(*number))),
            Value::Bool(flag) => out.push((name.clone(), Value::Bool(*flag))),
            _ => {}
        }
    }
    Value::Object(out)
}

// ---------------------------------------------------------------------------------------
// accounting
// ---------------------------------------------------------------------------------------

/// One line of the accounting log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The tool name.
    pub tool: String,
    /// Bytes of arguments.
    pub input_bytes: usize,
    /// Bytes of the text half.
    pub text_output_bytes: usize,
    /// Bytes of the structured half.
    pub structured_output_bytes: usize,
    /// The estimate over both halves.
    pub estimated_output_tokens: usize,
    /// Whether the budget withheld the result.
    pub truncated: bool,
    /// Wall clock, milliseconds.
    pub duration_ms: u64,
}

impl Call {
    /// The JSONL line this call writes: one compact JSON document, no newlines.
    ///
    /// Written by hand rather than through the pretty printer, because JSONL means one
    /// document per line and the pretty printer's whole job is putting newlines in.
    #[must_use]
    pub fn to_line(&self) -> String {
        let tool = self.tool.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "{{\"tool\":\"{tool}\",\"input_bytes\":{},\"text_output_bytes\":{},\
             \"structured_output_bytes\":{},\"estimated_output_tokens\":{},\
             \"truncated\":{},\"duration_ms\":{}}}",
            self.input_bytes,
            self.text_output_bytes,
            self.structured_output_bytes,
            self.estimated_output_tokens,
            self.truncated,
            self.duration_ms,
        )
    }

    /// Parses a line back, for `akr mcp stats`. Unreadable lines are skipped by the caller.
    #[must_use]
    pub fn from_line(line: &str) -> Option<Self> {
        let value = akr_core::json::parse(line).ok()?;
        let integer = |name: &str| -> usize {
            value
                .get(name)
                .and_then(Value::as_integer)
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(0)
        };
        Some(Self {
            tool: value.get("tool")?.as_str()?.to_owned(),
            input_bytes: integer("input_bytes"),
            text_output_bytes: integer("text_output_bytes"),
            structured_output_bytes: integer("structured_output_bytes"),
            estimated_output_tokens: integer("estimated_output_tokens"),
            truncated: value
                .get("truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            duration_ms: u64::try_from(integer("duration_ms")).unwrap_or(0),
        })
    }
}

/// Appends a call to the accounting log, if one is configured.
///
/// Best-effort by design: accounting that could fail a tool call would be a way for
/// instrumentation to break the thing it is measuring. A full disk loses a line.
pub fn record(path: Option<&std::path::Path>, call: &Call) {
    let Some(path) = path else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{}", call.to_line());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(words: usize) -> Value {
        Value::object(vec![(
            "results",
            Value::array(
                (0..words)
                    .map(|n| Value::string(format!("result number {n} with some words in it")))
                    .collect(),
            ),
        )])
    }

    #[test]
    fn a_small_result_passes_through_untouched() {
        let structured = Value::object(vec![("ok", Value::bool(true))]);
        let enforced = enforce("knowledge.validate", Some("ok\n"), &structured, None);
        assert!(!enforced.truncated);
        assert_eq!(enforced.text.as_deref(), Some("ok\n"));
        assert_eq!(enforced.structured, structured);
    }

    #[test]
    fn an_oversized_result_is_withheld_rather_than_shortened() {
        let structured = big(400);
        let enforced = enforce("knowledge.search", None, &structured, None);
        assert!(enforced.truncated);
        // Withheld, not trimmed: a partial list that does not say it is partial is worse
        // than no list.
        assert_ne!(enforced.structured, structured);
        assert_eq!(
            enforced.structured.get("truncated"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn a_withheld_result_says_how_to_ask_for_less() {
        let enforced = enforce("knowledge.get", None, &big(400), None);
        let continuation = enforced
            .structured
            .get("continuation")
            .expect("a ready-made retry");
        assert_eq!(
            continuation.get("arguments").and_then(|a| a.get("detail")),
            Some(&Value::String("summary".to_owned()))
        );
        assert!(
            enforced.text.expect("a summary").contains("detail"),
            "the text half has to carry the advice too"
        );
    }

    #[test]
    fn a_withheld_result_still_carries_its_counts() {
        let enforced = enforce("knowledge.search", None, &big(400), None);
        assert_eq!(
            enforced
                .structured
                .get("counts")
                .and_then(|c| c.get("results_count")),
            Some(&Value::Integer(400)),
            "a caller must be able to tell 'nothing matched' from 'too much matched'"
        );
    }

    #[test]
    fn both_halves_count_towards_the_budget() {
        // The duplication the design set warns about: a client that shows text and parses
        // structure pays for both, so the guard has to measure both.
        let structured = big(200);
        let text = structured.to_pretty();
        let with_text = enforce("knowledge.search", Some(&text), &structured, None);
        let without = enforce("knowledge.search", None, &structured, None);
        assert!(with_text.estimated_tokens > without.estimated_tokens);
    }

    #[test]
    fn writes_get_the_smallest_budget() {
        assert!(
            budget_for("knowledge.propose").hard_tokens
                < budget_for("knowledge.context").hard_tokens
        );
    }

    #[test]
    fn an_explicit_context_budget_replaces_the_default_transport_ceiling() {
        let structured = big(1_000);
        let estimated = estimate_tokens(&structured.to_pretty());
        assert!(estimated > budget_for("knowledge.context").hard_tokens);
        let enforced = enforce("knowledge.context", None, &structured, Some(1));
        assert!(!enforced.truncated);
        assert_eq!(enforced.structured, structured);
    }

    #[test]
    fn a_call_line_is_one_json_document() {
        let line = Call {
            tool: "knowledge.get".into(),
            input_bytes: 42,
            text_output_bytes: 100,
            structured_output_bytes: 200,
            estimated_output_tokens: 80,
            truncated: false,
            duration_ms: 7,
        }
        .to_line();
        assert!(!line.contains('\n'));
        assert!(line.contains("\"tool\":\"knowledge.get\""), "{line}");
    }
}
