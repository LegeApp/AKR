//! The four write tools of `docs/08-mcp.md` §4, and the guarantees §5 and §7 attach.
//!
//! The claims under test are the ones an agent would be harmed by if they were false: a
//! refused write leaves nothing behind, `base_rev` catches a moved head, and a supersession
//! missing a disposition names the children in the payload rather than in prose.

mod support;

use akr_core::json::{Value, parse};
use support::{Example, mcp_binary, one_line};

/// One `tools/call`, returning `(payload, is_error)`.
fn call(example: &Example, tool: &str, arguments: &str) -> (Value, bool) {
    use std::io::Write as _;
    use std::process::{Command, Stdio};

    let arguments = one_line(arguments);
    let request = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\
         \"params\":{{\"name\":\"{tool}\",\"arguments\":{arguments}}}}}\n"
    );
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
        .write_all(request.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("akr-mcp exits");
    let line = String::from_utf8_lossy(&output.stdout);
    let response = parse(line.trim()).unwrap_or_else(|error| {
        panic!(
            "no JSON-RPC response: {error}\n{line}\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let result = response.get("result").expect("a result").clone();
    let is_error = result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    (
        result
            .get("structuredContent")
            .cloned()
            .unwrap_or(Value::Null),
        is_error,
    )
}

fn error_class(payload: &Value) -> Option<&str> {
    payload
        .get("error")
        .and_then(|e| e.get("class"))
        .and_then(Value::as_str)
}

fn error_text(payload: &Value) -> String {
    payload.to_pretty()
}

#[test]
fn propose_creates_a_record_and_refuses_the_same_key_twice() {
    let example = Example::materialise("mcp-propose");
    let arguments = r#"{
        "key": "sys.term.day-loop",
        "kind": "term",
        "title": "The day loop",
        "state": "active",
        "scope": ["all"],
        "slots": { "definition": "The repeating structure of one in-game day: wake, work, evening, sleep." }
    }"#;
    let (payload, is_error) = call(&example, "knowledge.propose", arguments);
    assert!(!is_error, "{}", error_text(&payload));
    assert_eq!(
        payload.get("key").and_then(Value::as_str),
        Some("sys.term.day-loop")
    );
    assert_eq!(payload.get("rev").and_then(Value::as_integer), Some(1));
    assert_eq!(payload.get("written").and_then(Value::as_bool), Some(true));
    // Every write stales the lock, and the payload says so rather than leaving the agent
    // to discover it from the next `knowledge.validate` (D-014).
    assert_eq!(
        payload.get("lock_stale").and_then(Value::as_bool),
        Some(true)
    );
    // §4's remaining two fields describe the revision as it landed, not as it was planned,
    // so they are worth asserting: an agent that read a stale `state` back would think its
    // lifecycle move had not taken.
    assert_eq!(
        payload.get("state").and_then(Value::as_str),
        Some("active"),
        "{}",
        error_text(&payload)
    );
    assert!(
        payload
            .get("content_hash")
            .and_then(Value::as_str)
            .is_some_and(|hash| hash.starts_with("sha256:")),
        "{}",
        error_text(&payload)
    );
    assert!(
        example
            .read_file(".akr/records/sys/terms.akr")
            .contains("sys.term.day-loop/1")
    );

    // §4: an existing key is an error. The tool never silently turns a proposal into a
    // revision, because an agent that meant to create something and edited it instead has
    // no way to notice.
    let before = example.sources();
    let (payload, is_error) = call(&example, "knowledge.propose", arguments);
    assert!(is_error);
    assert_eq!(
        error_class(&payload),
        Some("invariant"),
        "{}",
        error_text(&payload)
    );
    assert_eq!(
        before,
        example.sources(),
        "a refused write left something behind"
    );
}

#[test]
fn propose_can_create_a_milestone_with_an_acceptance_block() {
    // V-008 requires a milestone to carry a non-empty `acceptance` block from the moment
    // it exists, and `knowledge.propose` had no field for one — the only way to author a
    // milestone was `akr propose --from` with a hand-written record body. This is the fix:
    // an agent should be able to do the whole thing over MCP.
    let example = Example::materialise("mcp-propose");
    let arguments = r#"{
        "key": "sys.milestone.m2",
        "kind": "milestone",
        "title": "M2",
        "slots": { "intent": "Ship the second milestone." },
        "acceptance": [
            { "id": "full-day-demo", "statement": "A full day loop runs end to end.",
              "method": "manual" }
        ]
    }"#;
    let (payload, is_error) = call(&example, "knowledge.propose", arguments);
    assert!(!is_error, "{}", error_text(&payload));
    assert_eq!(payload.get("rev").and_then(Value::as_integer), Some(1));
    let source = example.read_file(".akr/records/sys/milestones.akr");
    assert!(source.contains("acceptance {"), "{source}");
    assert!(source.contains("check full-day-demo"), "{source}");
}

#[test]
fn a_malformed_payload_is_a_schema_error_and_writes_nothing() {
    let example = Example::materialise("mcp-schema");
    let before = example.sources();
    // A slot the kind does not have. The grammar refuses it, so the class is `schema` and
    // the agent knows to fix the content rather than to retry or to escalate.
    let (payload, is_error) = call(
        &example,
        "knowledge.propose",
        r#"{"key":"sys.term.day-loop","kind":"term","title":"T",
            "slots":{"observed_at":"git:0000000000000000000000000000000000000000"}}"#,
    );
    assert!(is_error);
    assert_eq!(
        error_class(&payload),
        Some("schema"),
        "{}",
        error_text(&payload)
    );
    assert_eq!(before, example.sources());

    // And a record with no body at all: `docs/07-cli.md` §6 says a body source is
    // effectively mandatory, and the tool surface inherits that.
    let (payload, is_error) = call(
        &example,
        "knowledge.propose",
        r#"{"key":"sys.term.day-loop","kind":"term","title":"T"}"#,
    );
    assert!(is_error);
    assert_eq!(before, example.sources());
    assert!(
        error_text(&payload).contains("definition"),
        "{}",
        error_text(&payload)
    );
}

#[test]
fn revise_requires_base_rev_and_rejects_a_stale_one() {
    let example = Example::materialise("mcp-base-rev");
    let before = example.sources();

    // Missing entirely.
    let (payload, is_error) = call(
        &example,
        "knowledge.revise",
        r#"{"key":"sys.term.playable-day"}"#,
    );
    assert!(is_error);
    assert_eq!(
        error_class(&payload),
        Some("usage"),
        "{}",
        error_text(&payload)
    );

    // Present but stale: the head is at revision 1, so 7 is a claim about a ledger that
    // does not exist. §7 calls this the only concurrency control the surface has, and it
    // is enough because a human is watching the same working tree.
    let (payload, is_error) = call(
        &example,
        "knowledge.revise",
        r#"{"key":"sys.term.playable-day","base_rev":7}"#,
    );
    assert!(is_error);
    assert_eq!(
        error_class(&payload),
        Some("conflict"),
        "{}",
        error_text(&payload)
    );
    assert_eq!(
        payload
            .get("error")
            .and_then(|e| e.get("retryable"))
            .and_then(Value::as_bool),
        Some(true),
        "a conflict is the one class an agent may retry"
    );
    assert_eq!(before, example.sources());
}

#[test]
fn revise_keeps_the_slots_the_payload_does_not_mention() {
    let example = Example::materialise("mcp-revise");
    let (payload, is_error) = call(
        &example,
        "knowledge.revise",
        r#"{"key":"sys.policy.tandem-work","base_rev":1,
            "slots":{"rationale":"Rewritten after the M3 replan."}}"#,
    );
    assert!(!is_error, "{}", error_text(&payload));
    assert_eq!(payload.get("rev").and_then(Value::as_integer), Some(2));

    let source = example.read_file(".akr/records/sys/policies.akr");
    assert!(
        source.contains("Rewritten after the M3 replan."),
        "{source}"
    );
    // An agent that had to resend every slot would eventually drop one, and a dropped
    // claim is knowledge lost silently. The unmentioned ones survive.
    assert!(source.contains("claim lag-bound"), "{source}");
    assert!(source.contains("topic tandem-work"), "{source}");
    assert!(source.contains("exceptions"), "{source}");
    // And the head that was retired says so, in the same write.
    assert!(source.contains("state superseded"), "{source}");
}

#[test]
fn supersede_lists_the_unfinished_children_in_the_error_payload() {
    let example = Example::materialise("mcp-supersede");
    let path = ".akr/records/sim/work.akr";
    let source = example.read_file(path);
    example.write_file(
        path,
        &source.replace(
            "part_of [ @sys.milestone.m3-playable-day ]",
            "part_of [ @sys.milestone.m3-playable-day/1 ]",
        ),
    );

    let before = example.sources();
    let (payload, is_error) = call(
        &example,
        "knowledge.supersede",
        r#"{"old_key":"sys.milestone.m3-playable-day"}"#,
    );
    assert!(is_error);
    assert_eq!(
        error_class(&payload),
        Some("invariant"),
        "{}",
        error_text(&payload)
    );
    assert_eq!(
        payload
            .get("error")
            .and_then(|e| e.get("wrote"))
            .and_then(Value::as_bool),
        Some(false),
        "an agent never has to guess whether a failed write left something behind"
    );
    assert_eq!(before, example.sources());

    // §4: the children are *in the payload*, so the agent's next message can name them.
    // Structured, not parsed out of a sentence.
    let diagnostics = payload
        .get("error")
        .and_then(|e| e.get("diagnostics"))
        .and_then(Value::as_array)
        .expect("diagnostics");
    let children = diagnostics
        .iter()
        .find_map(|d| d.get("unfinished_children"))
        .and_then(Value::as_array)
        .expect("the children, structurally");
    assert_eq!(children.len(), 1, "{}", error_text(&payload));
    assert_eq!(
        children[0].get("child").and_then(Value::as_str),
        Some("sim.work.rewrite-projection")
    );
    assert_eq!(
        children[0].get("state").and_then(Value::as_str),
        Some("blocked")
    );

    // The same call with a disposition goes through.
    let (payload, is_error) = call(
        &example,
        "knowledge.supersede",
        r#"{"old_key":"sys.milestone.m3-playable-day",
            "dispositions":[{"child":"sim.work.rewrite-projection",
                             "outcome":"intentionally_dropped",
                             "note":"Rewritten under M4 instead."}]}"#,
    );
    assert!(!is_error, "{}", error_text(&payload));
    assert_eq!(payload.get("rev").and_then(Value::as_integer), Some(2));
}

#[test]
fn complete_names_the_unsatisfied_checks_in_the_error_payload() {
    let example = Example::materialise("mcp-complete");
    let before = example.sources();
    let (payload, is_error) = call(
        &example,
        "knowledge.complete",
        r#"{"key":"sys.milestone.m3-playable-day"}"#,
    );
    assert!(is_error);
    assert_eq!(
        error_class(&payload),
        Some("invariant"),
        "{}",
        error_text(&payload)
    );
    assert_eq!(before, example.sources());

    let checks = payload
        .get("error")
        .and_then(|e| e.get("diagnostics"))
        .and_then(Value::as_array)
        .expect("diagnostics")
        .iter()
        .find_map(|d| d.get("unsatisfied_checks"))
        .and_then(Value::as_array)
        .expect("the checks, structurally");
    assert_eq!(
        checks[0].get("id").and_then(Value::as_str),
        Some("no-placeholder-assets")
    );
    assert!(
        checks[0]
            .get("reason")
            .and_then(Value::as_str)
            .is_some_and(|reason| !reason.is_empty()),
        "the reason distinguishes 'no evidence' from 'evidence predates the change'"
    );
}

#[test]
fn a_write_is_visible_to_the_next_read_through_either_surface() {
    let example = Example::materialise("mcp-visible");
    let (payload, is_error) = call(
        &example,
        "knowledge.propose",
        r#"{"key":"sys.term.day-loop","kind":"term","title":"The day loop","state":"active",
            "scope":["all"],
            "slots":{"definition":"The repeating structure of one in-game day."}}"#,
    );
    assert!(!is_error, "{}", error_text(&payload));

    // Through the tool.
    let (fetched, is_error) = call(&example, "knowledge.get", r#"{"ref":"@sys.term.day-loop"}"#);
    assert!(!is_error, "{}", error_text(&fetched));
    assert_eq!(fetched.get("rev").and_then(Value::as_integer), Some(1));

    // And through the command line, which is the same ledger seen from the other side.
    let run = example.run(&["get", "@sys.term.day-loop"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.stdout.contains("The day loop"), "{}", run.stdout);
}

#[test]
fn no_tool_can_reach_the_sqlite_cache() {
    // D-019, restated in §6: the cache is a private detail of stage E. The assertion that
    // matters is negative and structural — no tool name mentions it, and no tool's schema
    // exposes a query hole.
    for tool in akr_mcp::schema::TOOLS {
        assert!(!tool.name.contains("query"), "{}", tool.name);
        assert!(!tool.name.contains("sql"), "{}", tool.name);
        let schema = akr_mcp::schema::input_schema(tool.name)
            .expect("every declared tool has a schema")
            .to_pretty();
        assert!(!schema.to_lowercase().contains("sqlite"), "{}", tool.name);
        assert!(!schema.to_lowercase().contains("select "), "{}", tool.name);
    }
    assert_eq!(akr_mcp::schema::TOOLS.len(), 13);
}

#[test]
fn evidence_add_creates_a_verified_record_with_head_as_default_commit() {
    // Bug report item 4: closing out a milestone over MCP required shelling out to
    // `akr evidence add`. The tool is the CLI command's twin: same request type, same
    // write pipeline, and — deliberately — no field for what the evidence verifies
    // (D-016).
    let example = Example::materialise("mcp-evidence");
    let arguments = r#"{
        "key": "sys.evidence.asset-audit",
        "result": "pass",
        "method": "command",
        "command": "cargo run -p tools -- audit-assets",
        "summary": "Zero placeholder assets on the day-loop path."
    }"#;
    let (payload, is_error) = call(&example, "knowledge.evidence_add", arguments);
    assert!(!is_error, "{}", error_text(&payload));
    assert_eq!(payload.get("rev").and_then(Value::as_integer), Some(1));
    // Empirical kinds have no proposal state: evidence lands `verified`.
    assert_eq!(
        payload.get("state").and_then(Value::as_str),
        Some("verified"),
        "{}",
        error_text(&payload)
    );
    let source = example.read_file(".akr/records/sys/evidence.akr");
    assert!(source.contains("sys.evidence.asset-audit/1"), "{source}");
    assert!(source.contains("result pass"), "{source}");
    // `observed_at` defaulted to HEAD — a real commit of the materialised repository.
    assert!(source.contains("observed_at"), "{source}");

    // Idempotent by key, like propose: a second add is an error, not a second record.
    let (payload, is_error) = call(&example, "knowledge.evidence_add", arguments);
    assert!(is_error, "{}", error_text(&payload));
}

#[test]
fn papercut_is_one_call_and_lands_in_its_own_view() {
    // D-027: the message is the whole ceremony. The tool allocates the key, fills the
    // slots, and the aggregate appears in PAPERCUTS.md on the next build.
    let example = Example::materialise("mcp-papercut");
    let arguments = r#"{
        "agent": "claude",
        "namespace": "sys",
        "message": "Ran akr search right after a write and got stale results; akr build in between fixed it."
    }"#;
    let (payload, is_error) = call(&example, "knowledge.papercut", arguments);
    assert!(!is_error, "{}", error_text(&payload));
    assert_eq!(payload.get("rev").and_then(Value::as_integer), Some(1));
    assert_eq!(
        payload.get("state").and_then(Value::as_str),
        Some("verified"),
        "{}",
        error_text(&payload)
    );
    let source = example.read_file(".akr/records/sys/papercuts.akr");
    assert!(source.contains(": papercut {"), "{source}");
    assert!(source.contains("author \"claude\""), "{source}");

    // A second papercut with the same message gets its own key, because a log never
    // refuses an entry.
    let (payload, is_error) = call(&example, "knowledge.papercut", arguments);
    assert!(!is_error, "{}", error_text(&payload));
    assert!(
        payload
            .get("key")
            .and_then(Value::as_str)
            .is_some_and(|key| key.ends_with("-2")),
        "{}",
        error_text(&payload)
    );

    // The build emits the view, newest first, with agent and citation.
    let run = example.run(&["build"]);
    assert_eq!(run.code, 0, "{}", run.output());
    let view = example.read_file("docs/generated/PAPERCUTS.md");
    assert!(view.contains("# Papercuts"), "{view}");
    assert!(view.contains("[claude]"), "{view}");
    assert!(view.contains("stale results"), "{view}");
}
