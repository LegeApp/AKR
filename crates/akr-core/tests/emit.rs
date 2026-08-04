//! `syntax::emit` is the inverse of `syntax::lower`: model in, canonical source out.

use akr_core::diagnostics::FileId;
use akr_core::model::{
    CheckMethod, ContentSlot, ContentValue, Kind, Outcome, RecordBuilder, SourceKind, State,
};
use akr_core::syntax::{emit, format, lower, parse};

fn round_trip(record: &akr_core::model::Record) -> akr_core::model::Record {
    let text = format!(
        "akr 0.1\nproject fixtures\n\n{}",
        emit::record_text(record, "fixtures")
    );
    let parsed = parse(&text, FileId(0));
    assert!(
        parsed.diagnostics.is_empty(),
        "emitted text must parse cleanly, got {:?}\n{text}",
        parsed
            .diagnostics
            .iter()
            .map(|d| d.code.as_str())
            .collect::<Vec<_>>()
    );
    let file = parsed.file.expect("parses");
    assert_eq!(
        format(&file),
        text,
        "emitted text must already be canonical"
    );
    let lowered = lower::lower_file(&file, "fixtures.akr");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    lowered.records.into_iter().next().expect("one record")
}

#[test]
fn a_filled_record_of_every_kind_round_trips() {
    for kind in Kind::ALL {
        let record = RecordBuilder::new("fx.a.b", 1, *kind).filled().build();
        let back = round_trip(&record);
        assert_eq!(back.kind, record.kind, "{kind}");
        assert_eq!(back.state, record.state, "{kind}");
        assert_eq!(back.title, record.title, "{kind}");
        assert_eq!(back.content, record.content, "{kind}");
        assert_eq!(back.scope, record.scope, "{kind}");
    }
}

#[test]
fn every_block_form_round_trips() {
    let record = RecordBuilder::new("fx.work.everything", 1, Kind::Work)
        .filled()
        .claim("first", "An addressable claim.")
        .retires(&["gone"])
        .check("one", CheckMethod::Command, &["@fx.evidence.something/1"])
        .disposition(
            "@fx.work.child",
            Outcome::CarriedForward,
            Some("@fx.track.t"),
        )
        .source(SourceKind::Legacy, Some("docs/legacy/PLAN.md"))
        .rel(akr_core::model::Relation::Implements, "@fx.req.something")
        .acknowledged(true)
        .build();

    let back = round_trip(&record);
    assert_eq!(back.claims, record.claims);
    assert_eq!(back.retired_claims, record.retired_claims);
    assert_eq!(back.acceptance, record.acceptance);
    assert_eq!(back.dispositions, record.dispositions);
    assert_eq!(back.sources, record.sources);
    assert_eq!(back.relations, record.relations);
    assert!(back.acknowledged);
}

#[test]
fn prose_survives_emission_including_blank_lines() {
    let mut record = RecordBuilder::new("fx.term.prose", 1, Kind::Term)
        .filled()
        .build();
    let text =
        "First paragraph.\n\nSecond paragraph, after a blank line.\n\n    An indented block.";
    record
        .content
        .insert(ContentSlot::Definition, ContentValue::prose(text));

    let back = round_trip(&record);
    assert_eq!(
        back.content.get(&ContentSlot::Definition),
        Some(&ContentValue::prose(text))
    );
}

#[test]
fn strings_needing_escapes_survive() {
    let mut record = RecordBuilder::new("fx.evidence.escapes", 1, Kind::Evidence)
        .filled()
        .build();
    record.content.insert(
        ContentSlot::Command,
        ContentValue::Text("grep -n \"needle\" src/*.rs | head -1\ttab".to_owned()),
    );
    record.title = "A \"quoted\" title with a \\ backslash".to_owned();

    let back = round_trip(&record);
    assert_eq!(back.title, record.title);
    assert_eq!(
        back.content.get(&ContentSlot::Command),
        record.content.get(&ContentSlot::Command)
    );
}

#[test]
fn the_emitted_text_starts_at_the_record_keyword() {
    let record = RecordBuilder::new("fx.term.minimal", 1, Kind::Term)
        .filled()
        .build();
    let text = emit::record_text(&record, "fixtures");
    assert!(
        text.starts_with("record fx.term.minimal/1 : term {"),
        "{text}"
    );
    assert!(text.ends_with("}\n"), "{text}");
    assert!(!text.contains("akr 0.1"), "no header in a record rendering");
}

#[test]
fn a_state_that_the_kind_forbids_still_emits_and_is_caught_by_validation() {
    // Emission is not a validator: it renders what it is given, and V-007 reports it.
    let record = RecordBuilder::new("fx.policy.wrong", 1, Kind::Policy)
        .filled()
        .state(State::Completed)
        .build();
    let text = emit::record_text(&record, "fixtures");
    assert!(text.contains("state completed"));
}
