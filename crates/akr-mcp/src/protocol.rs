//! JSON-RPC 2.0 over stdio: the transport `docs/08-mcp.md` §1 specifies.
//!
//! One JSON document per line, requests in on stdin, responses out on stdout. Line
//! framing rather than `Content-Length` headers because the server is launched as a child
//! process by the agent runtime and the stream is never shared with anything else; a
//! newline is unambiguous, greppable, and replayable from a file, which is what
//! `tests/differential.rs` does.
//!
//! Four methods: `initialize`, `server/discover`, `tools/list`, `tools/call`.
//! Notifications — a request with no `id` — are acknowledged by producing no response, as
//! JSON-RPC requires.
//!
//! # Tool failures are results, not transport errors
//!
//! A tool that refuses returns a *successful* JSON-RPC response whose result carries
//! `isError: true` and §5's payload. The transport succeeded; the ledger said no. Conflating
//! the two would make an agent unable to tell a refusal it should read from a server it
//! should restart.

use akr_core::json::{Value, parse};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::errors::ToolError;
use crate::schema::{TOOLS, input_schema, output_schema};
use crate::tools;

/// The legacy protocol version this server still accepts.
pub const PROTOCOL_LEGACY: &str = "2024-11-05";
/// The current protocol version this server now negotiates.
pub const PROTOCOL_CURRENT: &str = "2026-07-28";
/// Supported protocol versions, in preference order.
pub const SUPPORTED_PROTOCOLS: &[&str] = &[PROTOCOL_CURRENT, PROTOCOL_LEGACY];

/// The server's own version, matching the tool version the CLI reports.
pub const SERVER_VERSION: &str = "0.1.0";

/// A server bound to one workspace.
pub struct Server {
    root: PathBuf,
    surface: Surface,
    accounting: Option<PathBuf>,
}

/// Which half of the tool catalogue this server exposes.
///
/// Tool schemas are a fixed tax on every session that loads them, and an implementation
/// agent that will only ever read pays it for eight write tools it never calls. `read`
/// serves the surface that answers questions; `full` adds the ones that change the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Surface {
    /// Read tools only.
    Read,
    /// Every tool (the default).
    #[default]
    Full,
}

impl Surface {
    /// Parses `--surface`.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "read" => Some(Self::Read),
            "full" => Some(Self::Full),
            _ => None,
        }
    }

    /// Whether this surface exposes `tool`.
    #[must_use]
    pub fn exposes(self, tool: &crate::schema::Tool) -> bool {
        match self {
            Self::Read => !tool.writes,
            Self::Full => true,
        }
    }
}

impl Server {
    /// A server for the workspace at `root`, exposing every tool.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            surface: Surface::Full,
            accounting: None,
        }
    }

    /// Restricts the catalogue to one surface.
    #[must_use]
    pub fn with_surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    /// Appends one JSONL line per tool call to `path`.
    #[must_use]
    pub fn with_accounting(mut self, path: impl Into<PathBuf>) -> Self {
        self.accounting = Some(path.into());
        self
    }

    /// The workspace root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Handles one request, returning the response, or `None` for a notification.
    #[must_use]
    pub fn handle(&self, request: &Value) -> Option<Value> {
        let id = request.get("id").cloned();
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        // A notification carries no `id` and gets no response, per JSON-RPC 2.0.
        let id = id.filter(|value| !value.is_null())?;

        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "server/discover" => Ok(self.server_discover(&params)),
            "tools/list" => Ok(self.tools_list()),
            "tools/call" => Ok(self.tools_call(&params)),
            "ping" => Ok(Value::Object(Vec::new())),
            other => Err((-32601, format!("unknown method {other:?}"))),
        };

        Some(match result {
            Ok(result) => Value::object(vec![
                ("jsonrpc", Value::string("2.0")),
                ("id", id),
                ("result", result),
            ]),
            Err((code, message)) => Value::object(vec![
                ("jsonrpc", Value::string("2.0")),
                ("id", id),
                (
                    "error",
                    Value::object(vec![
                        ("code", Value::integer(code)),
                        ("message", Value::string(message)),
                    ]),
                ),
            ]),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let protocol_version = select_protocol(params);
        Value::object(vec![
            ("protocolVersion", Value::string(protocol_version)),
            (
                "capabilities",
                Value::object(vec![("tools", Value::Object(Vec::new()))]),
            ),
            (
                "serverInfo",
                Value::object(vec![
                    ("name", Value::string("akr-mcp")),
                    ("version", Value::string(SERVER_VERSION)),
                    // Published so a client can see a stale server before it trips over
                    // one: the friction this answers was diagnosed twice as a ledger bug.
                    (
                        "vocabularyVersion",
                        Value::string(crate::skew::SERVER_VOCABULARY),
                    ),
                ]),
            ),
            (
                "instructions",
                Value::string(format!(
                    "AKR knowledge ledger at {}. Call knowledge.context before touching \
                     code, and knowledge.validate before handing work back.",
                    self.root.display()
                )),
            ),
        ])
    }

    fn server_discover(&self, params: &Value) -> Value {
        let protocol_version = select_protocol(params);
        Value::object(vec![
            ("protocolVersion", Value::string(protocol_version)),
            (
                "serverInfo",
                Value::object(vec![
                    ("name", Value::string("akr-mcp")),
                    ("version", Value::string(SERVER_VERSION)),
                    // Published so a client can see a stale server before it trips over
                    // one: the friction this answers was diagnosed twice as a ledger bug.
                    (
                        "vocabularyVersion",
                        Value::string(crate::skew::SERVER_VOCABULARY),
                    ),
                ]),
            ),
            (
                "capabilities",
                Value::object(vec![("tools", Value::Object(Vec::new()))]),
            ),
            (
                "instructions",
                Value::string(format!(
                    "AKR knowledge ledger at {}. Call knowledge.context before touching \
                     code, and knowledge.validate before handing work back.",
                    self.root.display()
                )),
            ),
            (
                "supportedProtocols",
                Value::array(
                    SUPPORTED_PROTOCOLS
                        .iter()
                        .map(|version| Value::string(version.to_string()))
                        .collect(),
                ),
            ),
        ])
    }

    fn tools_list(&self) -> Value {
        Value::object(vec![(
            "tools",
            Value::array(
                TOOLS
                    .iter()
                    .filter(|tool| self.surface.exposes(tool))
                    .map(|tool| {
                        Value::object(vec![
                            ("name", Value::string(tool.name)),
                            ("description", Value::string(tool.description)),
                            (
                                "inputSchema",
                                input_schema(tool.name).unwrap_or(Value::Object(Vec::new())),
                            ),
                            (
                                "outputSchema",
                                output_schema(tool.name).unwrap_or(Value::Object(Vec::new())),
                            ),
                            (
                                "annotations",
                                Value::object(vec![
                                    ("readOnlyHint", Value::bool(!tool.writes)),
                                    // Nothing in AKR deletes knowledge
                                    // (`01-architecture.md` §9), so no tool is destructive
                                    // even when it writes.
                                    ("destructiveHint", Value::bool(false)),
                                    (
                                        "idempotentHint",
                                        Value::bool(!matches!(tool.name, "knowledge.revise")),
                                    ),
                                ]),
                            ),
                        ])
                    })
                    .collect(),
            ),
        )])
    }

    fn tools_call(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or("");
        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(Value::Object(Vec::new()));

        let started = std::time::Instant::now();
        let outcome = guarded_tool_call(|| tools::call(&self.root, name, &arguments));
        let (text, structured, is_error) = match outcome {
            Ok(payload) => {
                let requested_hard_tokens = (name == "knowledge.context")
                    .then(|| {
                        arguments
                            .get("budget_tokens")
                            .and_then(Value::as_integer)
                            .and_then(|value| usize::try_from(value).ok())
                    })
                    .flatten();
                let enforced = crate::budget::enforce(
                    name,
                    payload.text(),
                    payload.structured(),
                    requested_hard_tokens,
                );
                (enforced.text, enforced.structured, false)
            }
            Err(mut error) => {
                // A failing call is where an agent meets a stale server, and a type error
                // about a slot it never wrote is the least explicable thing the surface
                // can say. Naming the skew here turns it into a one-line remedy.
                if let Some(skew) = crate::skew::detect(&self.root) {
                    error.diagnostics.push(skew.diagnostic());
                }
                (None, error.to_json(), true)
            }
        };

        crate::budget::record(
            self.accounting.as_deref(),
            &crate::budget::Call {
                tool: name.to_owned(),
                input_bytes: arguments.to_pretty().len(),
                text_output_bytes: text.as_ref().map_or(0, String::len),
                structured_output_bytes: structured.to_pretty().len(),
                estimated_output_tokens: crate::budget::estimate_tokens(
                    text.as_deref().unwrap_or_default(),
                ) + crate::budget::estimate_tokens(
                    &structured.to_pretty(),
                ),
                truncated: structured.get("truncated").and_then(Value::as_bool) == Some(true),
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            },
        );

        content(text.as_deref(), &structured, is_error)
    }
}

/// Contains an implementation panic to the request that triggered it.
///
/// The ledger write pipeline performs its durable write only after validation and uses
/// atomic file replacement. A panic is nevertheless an internal bug, so the response is
/// retryable once and the long-lived stdio server stays available for diagnostics.
fn guarded_tool_call<T>(call: impl FnOnce() -> Result<T, ToolError>) -> Result<T, ToolError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(call)) {
        Ok(result) => result,
        Err(_) => Err(ToolError::new(
            "AKR-X099",
            "the AKR tool implementation failed unexpectedly; the server contained the failure",
        )),
    }
}

/// An MCP tool result: the payload as JSON text, plus the structured form.
///
/// Both, deliberately. `content` is what a client that only knows about text will show a
/// model; `structuredContent` is what a client that understands the schema will parse.
/// Emitting one and not the other would make the server work well with half the runtimes.
fn content(text: Option<&str>, structured: &Value, is_error: bool) -> Value {
    let text = text
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| structured.to_pretty());
    Value::object(vec![
        (
            "content",
            Value::array(vec![Value::object(vec![
                ("type", Value::string("text")),
                ("text", Value::string(text)),
            ])]),
        ),
        ("resultType", Value::string("tool")),
        ("structuredContent", structured.clone()),
        ("isError", Value::bool(is_error)),
    ])
}

fn select_protocol(parameters: &Value) -> &'static str {
    parameters
        .get("protocolVersion")
        .and_then(Value::as_str)
        .and_then(|requested| {
            SUPPORTED_PROTOCOLS
                .iter()
                .copied()
                .find(|protocol| *protocol == requested)
        })
        .unwrap_or(PROTOCOL_LEGACY)
}

/// Reads newline-delimited JSON-RPC from `input` and writes responses to `output`.
///
/// # Errors
/// Any I/O failure on either stream. A malformed line is answered with a JSON-RPC parse
/// error rather than ending the session: one bad message should not take down a server an
/// agent is mid-task with.
pub fn serve(server: &Server, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match parse(&line) {
            Ok(request) => server.handle(&request),
            Err(error) => Some(Value::object(vec![
                ("jsonrpc", Value::string("2.0")),
                ("id", Value::Null),
                (
                    "error",
                    Value::object(vec![
                        ("code", Value::integer(-32700)),
                        ("message", Value::string(format!("parse error: {error}"))),
                    ]),
                ),
            ])),
        };
        if let Some(response) = response {
            writeln!(output, "{}", compact(&response))?;
            output.flush()?;
        }
    }
    Ok(())
}

/// One response, on one line.
///
/// [`Value::to_pretty`] is for humans reading an envelope; a transport frame is delimited
/// by newlines, so it cannot contain them.
fn compact(value: &Value) -> String {
    value
        .to_pretty()
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use akr_core::json::parse;

    use super::{Server, guarded_tool_call, serve};

    #[test]
    fn a_tool_panic_becomes_a_retryable_internal_error() {
        let error = guarded_tool_call::<()>(|| panic!("injected tool panic"))
            .expect_err("the panic is contained");
        let payload = error.to_json().to_pretty();
        assert!(payload.contains("\"class\": \"internal\""), "{payload}");
        assert!(payload.contains("\"retryable\": true"), "{payload}");
        assert!(payload.contains("AKR-X099"), "{payload}");
    }

    #[test]
    fn a_thousand_requests_and_a_malformed_frame_do_not_end_the_session() {
        let mut input = String::new();
        for id in 0..1_000 {
            let request = match id % 3 {
                0 => format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"ping"}}"#),
                1 => format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list"}}"#),
                _ => format!(
                    r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/call","params":{{"name":"knowledge.unknown","arguments":{{}}}}}}"#
                ),
            };
            input.push_str(&request);
            input.push('\n');
            if id == 499 {
                input.push_str("{malformed\n");
            }
        }

        let mut output = Vec::new();
        serve(
            &Server::new("/workspace-not-opened-by-this-test"),
            Cursor::new(input),
            &mut output,
        )
        .expect("the session remains available");

        let output = String::from_utf8(output).expect("responses are UTF-8");
        let responses = output.lines().collect::<Vec<_>>();
        assert_eq!(responses.len(), 1_001);
        for response in &responses {
            parse(response).expect("every response is valid JSON");
        }
        assert!(responses[500].contains("\"code\": -32700"));
        assert!(
            responses
                .last()
                .is_some_and(|line| line.contains("\"id\": 999"))
        );
    }
}
