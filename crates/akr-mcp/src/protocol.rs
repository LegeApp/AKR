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
}

impl Server {
    /// A server for the workspace at `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
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
            "tools/list" => Ok(Self::tools_list()),
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

    fn tools_list() -> Value {
        Value::object(vec![(
            "tools",
            Value::array(
                TOOLS
                    .iter()
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

        match tools::call(&self.root, name, &arguments) {
            Ok(payload) => content(payload.text(), payload.structured(), false),
            Err(error) => content(None, &error.to_json(), true),
        }
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
