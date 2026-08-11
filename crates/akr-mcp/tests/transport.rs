//! The installed shape of the MCP server: one subprocess, one long-lived stdio session.

mod support;

use akr_core::json::{Value, parse};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use support::Example;

fn call(
    input: &mut impl Write,
    output: &mut impl BufRead,
    id: usize,
    method: &str,
    params: &str,
) -> Value {
    writeln!(
        input,
        r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#
    )
    .expect("request writes");
    input.flush().expect("request flushes");
    let mut line = String::new();
    output.read_line(&mut line).expect("response reads");
    assert!(!line.is_empty(), "server closed before response {id}");
    parse(line.trim()).expect("response is JSON")
}

#[test]
fn a_real_subprocess_survives_mixed_reads_writes_and_rejections() {
    let example = Example::materialise("mcp-subprocess-stress");
    let mut child = Command::new(env!("CARGO_BIN_EXE_akr-mcp"))
        .arg("--dir")
        .arg(example.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("server starts");
    let mut input = child.stdin.take().expect("stdin");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));

    let mut id = 1usize;
    let initialized = call(
        &mut input,
        &mut output,
        id,
        "initialize",
        r#"{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"stress","version":"1"}}"#,
    );
    assert!(initialized.get("result").is_some());

    for round in 0..25 {
        for (tool, arguments) in [
            ("knowledge.explain", r#"{"subject":"decision"}"#.to_owned()),
            (
                "knowledge.context",
                r#"{"goal":"@sys.milestone.m3-playable-day/1","budget_tokens":4000}"#.to_owned(),
            ),
            (
                "knowledge.get",
                r#"{"ref":"@sys.term.playable-day/1","detail":"summary"}"#.to_owned(),
            ),
            ("knowledge.validate", r#"{"limit":5}"#.to_owned()),
            ("knowledge.unknown", r#"{}"#.to_owned()),
            (
                "knowledge.papercut",
                format!(
                    r#"{{"agent":"stress","namespace":"sys","message":"subprocess stress report {round}"}}"#
                ),
            ),
        ] {
            id += 1;
            let params = format!(r#"{{"name":"{tool}","arguments":{arguments}}}"#);
            let response = call(&mut input, &mut output, id, "tools/call", &params);
            assert!(response.get("result").is_some(), "{}", response.to_pretty());
        }
    }

    drop(input);
    let completed = child.wait_with_output().expect("server exits on EOF");
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    assert!(
        completed.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
}
