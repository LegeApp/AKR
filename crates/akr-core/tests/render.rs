//! The rendered diagnostic form of `spec/diagnostics/README.md` §5.

use akr_core::diagnostics::{
    Diagnostic, FileId, Label, RuleId, SourceMap, Span, Subject, codes, render,
};
use akr_core::syntax::parse;

#[test]
fn a_span_diagnostic_renders_in_the_caret_form() {
    let source = "akr 0.1\nproject fixtures\n\nrecord fx.term.x/1 : term {\n    state active\n}\n";
    let mut sources = SourceMap::new();
    let file = sources.add("fx.akr", source);

    let diagnostic = Diagnostic {
        code: codes::T001,
        severity: akr_core::diagnostics::Severity::Error,
        rule: Some(RuleId(8)),
        message: "term requires slot `definition`".to_owned(),
        primary: Label {
            subject: Subject::Ledger,
            span: Some(Span {
                file,
                start: 26,
                end: 51,
            }),
            message: Some("this record".to_owned()),
        },
        notes: vec![Label {
            subject: Subject::Ledger,
            span: Some(Span {
                file,
                start: 47,
                end: 51,
            }),
            message: Some("the kind is declared here".to_owned()),
        }],
        help: Some("see the kind's table in docs/02 §4".to_owned()),
    };

    let rendered = render(&diagnostic, &sources);
    let expected = "\
error[AKR-T001]: term requires slot `definition`
  --> fx.akr:4:1
   |
 4 | record fx.term.x/1 : term {
   | ^^^^^^^^^^^^^^^^^^^^^^^^^ this record
   |
note: the kind is declared here
  --> fx.akr:4:22
help: see the kind's table in docs/02 \u{a7}4 (see V-008)
";
    assert_eq!(expected, rendered);
}

#[test]
fn parse_diagnostics_carry_usable_spans() {
    let source =
        "akr 0.1\nproject fixtures\n\nrecord fx.term.x/1 : term {\n    created_at 2026-02-30\n}\n";
    let parsed = parse(source, FileId(0));
    let mut sources = SourceMap::new();
    sources.add("fx.akr", source);
    let diagnostic = parsed
        .diagnostics
        .first()
        .expect("an invalid date is reported");
    assert_eq!(diagnostic.code.as_str(), "AKR-P022");

    let rendered = render(diagnostic, &sources);
    assert!(rendered.starts_with("error[AKR-P022]:"), "{rendered}");
    assert!(rendered.contains("--> fx.akr:5:16"), "{rendered}");
    assert!(rendered.contains("^^^^^^^^^^"), "{rendered}");
}

#[test]
fn a_diagnostic_with_no_span_still_renders() {
    let sources = SourceMap::new();
    let diagnostic = Diagnostic::error(
        codes::R001,
        RuleId(12),
        Subject::Ledger,
        "sys.policy.tandem-work has 2 live revisions",
    )
    .help("supersede or withdraw all but one");
    let rendered = render(&diagnostic, &sources);
    assert_eq!(
        rendered,
        "error[AKR-R001]: sys.policy.tandem-work has 2 live revisions\nhelp: supersede or withdraw all but one (see V-012)\n"
    );
}
