//! Session-head briefing behavior through the real CLI.

mod support;

use akr_core::json::{Value, parse};
use support::Example;

fn start_json(example: &Example, task: &str, extra: &[&str]) -> Value {
    let mut args = vec!["--format", "json", "start", task];
    args.extend_from_slice(extra);
    let run = example.run(&args);
    assert_eq!(run.code, 0, "{}", run.output());
    parse(&run.stdout).expect("start JSON")
}

#[test]
fn start_prepends_a_validated_handoff_and_preserves_orientation() {
    let example = Example::materialise("handoff-basic");
    let run = example.run(&["start", "rewrite projection"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.starts_with("AKR SESSION HEAD\n"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("outstanding"), "{}", run.stdout);

    let json = start_json(&example, "rewrite projection", &[]);
    let result = json.get("result").expect("result");
    assert!(result.get("results").is_some());
    assert_eq!(
        result
            .get("handoff")
            .and_then(|handoff| handoff.get("snapshot"))
            .and_then(|snapshot| snapshot.get("origin"))
            .and_then(Value::as_str),
        Some("working_tree")
    );
    assert!(
        result
            .get("handoff")
            .and_then(|handoff| handoff.get("outstanding"))
            .and_then(|outstanding| outstanding.get("branches"))
            .and_then(Value::as_array)
            .is_some_and(|branches| !branches.is_empty())
    );
    assert!(run.stdout.contains("namespaces"), "{}", run.stdout);
    assert!(
        result
            .get("handoff")
            .and_then(|handoff| handoff.get("namespaces"))
            .and_then(Value::as_array)
            .is_some_and(|namespaces| !namespaces.is_empty()),
        "{:?}",
        result
    );
}

#[test]
fn maintenance_head_does_not_displace_the_latest_akr_work_focus() {
    let example = Example::materialise("handoff-linked");
    example.git(&["add", "-A"]);
    example.git(&[
        "commit",
        "--quiet",
        "-m",
        "feat: checkpoint projection work\n\nAKR-Work: @sim.work.rewrite-projection/1",
    ]);
    example.write_file("MAINTENANCE", "later maintenance\n");
    example.git(&["add", "MAINTENANCE"]);
    example.git(&["commit", "--quiet", "-m", "chore: later maintenance"]);

    let run = example.run(&["start", "projection"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("chore: later maintenance"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("feat: checkpoint projection work"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("@sim.work.rewrite-projection/1"),
        "{}",
        run.stdout
    );
}

#[test]
fn invalid_working_knowledge_falls_back_to_validated_head() {
    let example = Example::materialise("handoff-invalid-overlay");
    example.git(&["add", "-A"]);
    example.git(&["commit", "--quiet", "-m", "build: commit valid knowledge"]);

    let path = example.root().join(".akr/records/sim/work.akr");
    let mut text = std::fs::read_to_string(&path).expect("work records");
    text.push_str("\nthis is not AKR syntax\n");
    std::fs::write(path, text).expect("corrupt working overlay");

    let json = start_json(&example, "projection", &[]);
    let snapshot = json
        .get("result")
        .and_then(|result| result.get("handoff"))
        .and_then(|handoff| handoff.get("snapshot"))
        .expect("snapshot");
    assert_eq!(
        snapshot.get("origin").and_then(Value::as_str),
        Some("head_fallback")
    );
    assert!(
        snapshot
            .get("excluded_working_diagnostics")
            .and_then(Value::as_integer)
            .is_some_and(|count| count > 0)
    );
    assert_eq!(
        json.get("result")
            .and_then(|result| result.get("backend"))
            .and_then(Value::as_str),
        Some("validated_head_fallback")
    );
}

#[test]
fn a_small_budget_reports_deterministic_omissions() {
    let example = Example::materialise("handoff-budget");
    let first = start_json(&example, "projection", &["--budget", "90"]);
    let second = start_json(&example, "projection", &["--budget", "90"]);
    assert_eq!(first.to_pretty(), second.to_pretty());
    let omitted = first
        .get("result")
        .and_then(|result| result.get("handoff"))
        .and_then(|handoff| handoff.get("omitted"))
        .and_then(|omitted| omitted.get("planning"))
        .and_then(Value::as_integer)
        .unwrap_or_default();
    assert!(omitted > 0, "{}", first.to_pretty());
}

#[test]
fn start_still_collates_ledger_state_when_git_is_unavailable() {
    let example = Example::materialise("handoff-no-git");
    let run = example.run_without_git(&["start", "projection"]);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.stdout.contains("(no Git commit)"), "{}", run.stdout);
    assert!(run.stdout.contains("outstanding"), "{}", run.stdout);
}
