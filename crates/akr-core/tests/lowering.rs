//! Type constraints enforced while lowering source into the model.

use akr_core::diagnostics::{FileId, SlotRef, Subject, codes as c};
use akr_core::model::ContentSlot;
use akr_core::syntax::{lower_file, parse};

#[test]
fn constrained_content_enums_name_the_record_and_invalid_value() {
    let cases = [
        (
            "fx.obs.invalid-method",
            "observation",
            ContentSlot::Method,
            "automated",
        ),
        (
            "fx.evidence.invalid-result",
            "evidence",
            ContentSlot::Result,
            "success",
        ),
        (
            "fx.evidence.invalid-method",
            "evidence",
            ContentSlot::Method,
            "instrumented",
        ),
        (
            "fx.assessment.invalid-confidence",
            "assessment",
            ContentSlot::Confidence,
            "certain",
        ),
    ];

    for (key, kind, slot, invalid) in cases {
        let source = format!(
            "akr 0.1\nproject fixtures\n\nrecord {key}/1 : {kind} {{\n    {} {invalid}\n}}\n",
            slot.name()
        );
        let parsed = parse(&source, FileId(0));
        assert!(
            parsed.diagnostics.is_empty(),
            "{key}: {:?}",
            parsed.diagnostics
        );
        let lowered = lower_file(parsed.file.as_ref().expect("source parses"), "fixture.akr");
        assert_eq!(
            lowered.diagnostics.len(),
            1,
            "{key}: {:?}",
            lowered.diagnostics
        );

        let diagnostic = &lowered.diagnostics[0];
        assert_eq!(diagnostic.code, c::T012);
        assert!(
            diagnostic.message.contains(&format!("{key}/1")),
            "{}",
            diagnostic.message
        );
        assert!(
            diagnostic.message.contains(&format!("`{invalid}`")),
            "{}",
            diagnostic.message
        );
        assert!(matches!(
            &diagnostic.primary.subject,
            Subject::Slot(id, SlotRef::Content(found)) if id.to_string() == format!("{key}/1") && *found == slot
        ));
    }
}

#[test]
fn legacy_observation_method_remains_readable() {
    let source = "akr 0.1\nproject fixtures\n\nrecord fx.obs.legacy-method/1 : observation {\n    method observation\n}\n";
    let parsed = parse(source, FileId(0));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);

    let lowered = lower_file(parsed.file.as_ref().expect("source parses"), "fixture.akr");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
}
