//! Exit criterion 4 of P6: `knowledge.context` and `akr context` produce the same bundle
//! from the same request — and the same for every other read tool.
//!
//! Both surfaces are exercised as the processes they ship as. Testing the library functions
//! against each other would prove only that a call to one function equals a call to the same
//! function; running the two binaries proves that the *adapters* agree, which is where the
//! drift `docs/08-mcp.md` §1 warns about would actually appear.

mod support;

use akr_core::json::{Value, parse};
use support::{Example, mcp_binary};

/// One `tools/call`, over a fresh server process.
///
/// A fresh process per call is deliberate: §7 says a read tool's effect on the repository
/// is nil and that two calls against the same sources return byte-identical results. A
/// server that quietly cached between calls would pass a test that reused one process.
fn call(example: &Example, tool: &str, arguments: &str) -> Value {
    let request = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\
         \"params\":{{\"name\":\"{tool}\",\"arguments\":{arguments}}}}}\n"
    );
    let response = rpc(example, &request);
    response
        .first()
        .and_then(|value| value.get("result"))
        .and_then(|result| result.get("structuredContent"))
        .cloned()
        .unwrap_or(Value::Null)
}

/// Whether the last call returned an error result.
fn is_error(example: &Example, tool: &str, arguments: &str) -> bool {
    let request = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\
         \"params\":{{\"name\":\"{tool}\",\"arguments\":{arguments}}}}}\n"
    );
    rpc(example, &request)
        .first()
        .and_then(|value| value.get("result"))
        .and_then(|result| result.get("isError"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Feeds newline-delimited JSON-RPC to a server process and parses every response.
fn rpc(example: &Example, requests: &str) -> Vec<Value> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let mut child = Command::new(mcp_binary())
        .arg("--dir")
        .arg(example.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("akr-mcp runs");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(requests.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("akr-mcp exits");
    assert!(
        output.status.success(),
        "akr-mcp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse(line).expect("a JSON-RPC response"))
        .collect()
}

/// The `result` object of an `akr ... --format json` run.
fn cli_result(example: &Example, args: &[&str]) -> Value {
    let mut full = vec!["--format", "json"];
    full.extend_from_slice(args);
    let run = example.run(&full);
    let document = parse(&run.stdout).unwrap_or_else(|error| {
        panic!(
            "akr {} produced no JSON: {error}\n{}",
            args.join(" "),
            run.output()
        )
    });
    document.get("result").cloned().unwrap_or(Value::Null)
}

// -------------------------------------------------------------------------------------
// Exit criterion 4
// -------------------------------------------------------------------------------------

#[test]
fn knowledge_context_and_akr_context_produce_the_same_bundle() {
    let example = Example::materialise("differential-context");
    let tool = call(
        &example,
        "knowledge.context",
        r#"{"goal":"sys.milestone.m3-playable-day","paths":["sim/src/project/**"]}"#,
    );
    let cli = cli_result(
        &example,
        &[
            "context",
            "--goal",
            "sys.milestone.m3-playable-day",
            "--paths",
            "sim/src/project/**",
        ],
    );
    assert_eq!(
        tool.to_pretty(),
        cli.to_pretty(),
        "the bundle must be identical, not merely equivalent"
    );
    assert!(tool.get("sections").is_some(), "{}", tool.to_pretty());
}

#[test]
fn a_budget_reaches_the_same_assembly_through_both_surfaces() {
    let example = Example::materialise("differential-budget");
    let tool = call(
        &example,
        "knowledge.context",
        r#"{"goal":"sys.milestone.m3-playable-day","budget_tokens":900}"#,
    );
    let cli = cli_result(
        &example,
        &[
            "context",
            "--goal",
            "sys.milestone.m3-playable-day",
            "--budget",
            "900",
        ],
    );
    assert_eq!(tool.to_pretty(), cli.to_pretty());
}

// -------------------------------------------------------------------------------------
// Every read tool
// -------------------------------------------------------------------------------------

#[test]
fn knowledge_get_and_akr_get_agree() {
    let example = Example::materialise("differential-get");
    for reference in [
        "@sys.policy.tandem-work",
        "@sim.obs.projection-gaps",
        "@sys.work.m3-plan/1",
    ] {
        let tool = call(
            &example,
            "knowledge.get",
            &format!("{{\"ref\":\"{reference}\"}}"),
        );
        let cli = cli_result(&example, &["get", reference, "--relations"]);
        assert_eq!(tool.to_pretty(), cli.to_pretty(), "for {reference}");
    }
}

#[test]
fn knowledge_impact_and_akr_impact_agree_in_both_modes() {
    let example = Example::materialise("differential-impact");
    let tool = call(
        &example,
        "knowledge.impact",
        r#"{"ref":"@sim.obs.projection-gaps"}"#,
    );
    let cli = cli_result(&example, &["impact", "@sim.obs.projection-gaps"]);
    assert_eq!(tool.to_pretty(), cli.to_pretty());

    let range = format!("{}..{}", example.commit(2), example.commit(4));
    let tool = call(
        &example,
        "knowledge.impact",
        &format!("{{\"git_diff\":\"{range}\"}}"),
    );
    let cli = cli_result(&example, &["impact", "--git-diff", &range]);
    assert_eq!(tool.to_pretty(), cli.to_pretty());
}

#[test]
fn knowledge_validate_agrees_with_akr_check() {
    let example = Example::materialise("differential-validate");
    let tool = call(&example, "knowledge.validate", "{}");
    let cli = cli_result(&example, &["check"]);

    assert_eq!(tool.get("ok").and_then(Value::as_bool), Some(true));
    for field in ["records", "revisions", "stale", "at_risk"] {
        assert_eq!(
            tool.get("counts").and_then(|c| c.get(field)),
            cli.get(field),
            "counts.{field}"
        );
    }
    assert_eq!(
        tool.get("diagnostics").and_then(Value::as_array),
        Some(&[][..]),
        "a clean ledger has no diagnostics"
    );
}

#[cfg(feature = "fts5")]
#[test]
fn knowledge_search_and_akr_search_agree() {
    let example = Example::materialise("differential-search");
    assert_eq!(example.run(&["build"]).code, 0);

    let tool = call(&example, "knowledge.search", r#"{"query":"projection"}"#);
    let cli = cli_result(&example, &["search", "projection"]);
    assert_eq!(tool.to_pretty(), cli.to_pretty());
    assert!(
        tool.get("results")
            .and_then(Value::as_array)
            .is_some_and(|results| !results.is_empty()),
        "two empty result sets agree trivially: {}",
        tool.to_pretty()
    );

    // Filters travel through the tool the same way they travel through the flags, which is
    // the part an agent would notice first if it drifted.
    let tool = call(
        &example,
        "knowledge.search",
        r#"{"query":"day","kinds":["milestone"],"limit":3}"#,
    );
    let cli = cli_result(
        &example,
        &["search", "day", "--kind", "milestone", "--limit", "3"],
    );
    assert_eq!(tool.to_pretty(), cli.to_pretty());
}

#[test]
fn a_cache_without_a_ranker_fails_the_same_way_on_both_surfaces() {
    // P7 exit criterion 4 reaches the tool surface too: an agent must learn that search is
    // unavailable, not that the ledger is empty.
    let example = Example::materialise("differential-search-degraded");
    assert_eq!(example.run(&["build"]).code, 0);

    // No cache at all is the condition both surfaces have to survive, and it is reachable
    // in either build: the binary without FTS5 never had a ranker, and the one with FTS5
    // just lost the file its ranker lived in.
    std::fs::remove_dir_all(example.root().join(".akr/cache")).expect("the cache goes");

    let run = example.run(&["search", "projection"]);
    assert_eq!(run.code, 3, "{}", run.output());

    let payload = call(&example, "knowledge.search", r#"{"query":"projection"}"#);
    let error = payload.get("error").expect("an error payload");
    assert_eq!(
        error.get("class").and_then(Value::as_str),
        Some("environment")
    );
}

// -------------------------------------------------------------------------------------
// §7 — read/write separation and idempotency
// -------------------------------------------------------------------------------------

#[test]
fn read_tools_are_byte_identical_across_calls_and_touch_nothing() {
    let example = Example::materialise("differential-idempotent");
    let before = example.sources();
    let first = call(
        &example,
        "knowledge.context",
        r#"{"goal":"sys.milestone.m3-playable-day"}"#,
    );
    let second = call(
        &example,
        "knowledge.context",
        r#"{"goal":"sys.milestone.m3-playable-day"}"#,
    );
    assert_eq!(first.to_pretty(), second.to_pretty());
    assert_eq!(before, example.sources(), "a read tool wrote something");
}

/// The same agreement, on the other worked example.
///
/// `save-your-skin` is where every other test here lives, and a differential property that
/// held on exactly one ledger would be evidence about that ledger rather than about the
/// adapters. `sys-tandem` is shaped differently — three source roots, a superseded
/// assessment, five milestones — so a bundle that agrees across both surfaces on it too is
/// §1's invariant rather than a coincidence of one example's shape.
#[test]
fn both_surfaces_agree_on_the_other_example_too() {
    let example = Example::of(&support::SYS_TANDEM, "differential-sys-tandem");

    let tool = call(
        &example,
        "knowledge.context",
        r#"{"goal":"tandem.milestone.m5-one-playable-day"}"#,
    );
    let cli = cli_result(
        &example,
        &["context", "--goal", "tandem.milestone.m5-one-playable-day"],
    );
    assert_eq!(tool.to_pretty(), cli.to_pretty());
    assert!(tool.get("sections").is_some(), "{}", tool.to_pretty());

    // A key whose head is a supersession, which `save-your-skin` does not exercise through
    // `knowledge.get`: the tool has to agree with the CLI about *which* revision is head.
    for reference in [
        "@tandem.assessment.central-fact",
        "@simulator.question.wild-threshold",
        "@engine.req.no-debug-surfaces",
    ] {
        let tool = call(
            &example,
            "knowledge.get",
            &format!("{{\"ref\":\"{reference}\"}}"),
        );
        let cli = cli_result(&example, &["get", reference, "--relations"]);
        assert_eq!(tool.to_pretty(), cli.to_pretty(), "for {reference}");
        // Two surfaces that both failed would also "agree", so the record has to be here.
        assert!(
            tool.get("key").and_then(Value::as_str).is_some(),
            "no record came back for {reference}: {}",
            tool.to_pretty()
        );
    }

    let tool = call(
        &example,
        "knowledge.impact",
        r#"{"ref":"@tandem.assessment.central-fact"}"#,
    );
    let cli = cli_result(&example, &["impact", "@tandem.assessment.central-fact"]);
    assert_eq!(tool.to_pretty(), cli.to_pretty());
    assert!(
        tool.get("dependents")
            .and_then(Value::as_array)
            .is_some_and(|dependents| !dependents.is_empty()),
        "an impact query with nothing downstream would agree trivially: {}",
        tool.to_pretty()
    );
}

#[test]
fn every_declared_tool_has_a_schema_and_an_implementation() {
    let example = Example::materialise("differential-catalogue");
    let responses = rpc(
        &example,
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{}}\n",
    );
    let tools = responses[0]
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(Value::as_array)
        .expect("a tool list");
    assert_eq!(tools.len(), 11, "the catalogue is closed for 0.1");

    for tool in tools {
        let name = tool.get("name").and_then(Value::as_str).expect("a name");
        assert!(name.starts_with("knowledge."), "{name}");
        let schema = tool.get("inputSchema").expect("a schema");
        assert_eq!(
            schema.get("type").and_then(Value::as_str),
            Some("object"),
            "{name}"
        );
        assert!(schema.get("properties").is_some(), "{name}");

        // Every declared tool answers, and a tool with required arguments says so rather
        // than guessing. A tool listed but unimplemented would be worse than one absent:
        // the agent would plan around it.
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("a required list");
        if !required.is_empty() {
            assert!(
                is_error(&example, name, "{}"),
                "{name} requires {required:?} but accepted an empty call"
            );
        }
    }

    // And a name outside the catalogue is refused rather than ignored.
    let payload = call(&example, "knowledge.query", "{}");
    assert_eq!(
        payload
            .get("error")
            .and_then(|e| e.get("class"))
            .and_then(Value::as_str),
        Some("usage")
    );
}

#[test]
fn a_notification_gets_no_response_and_a_bad_line_does_not_end_the_session() {
    let example = Example::materialise("differential-protocol");
    let responses = rpc(
        &example,
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n\
         not json at all\n\
         {\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\",\"params\":{}}\n",
    );
    // Two responses: the parse error and the ping. The notification is silent.
    assert_eq!(responses.len(), 2, "{responses:?}");
    assert_eq!(
        responses[0]
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(Value::as_integer),
        Some(-32700)
    );
    assert_eq!(responses[1].get("id").and_then(Value::as_integer), Some(7));
}
