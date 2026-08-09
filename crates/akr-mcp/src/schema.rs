//! The tool declarations of `docs/08-mcp.md` §2, with their JSON schemas.
//!
//! The list is **closed for 0.1** — no `knowledge.query`, no `knowledge.build`, no
//! `knowledge.delete`. §2 gives a reason for each absence and they are all the same
//! reason: a tool is a contract, and every tool that exists is one an agent will lean on.
//! The escape valve is `knowledge.search`, which returns records rather than rows (§6).

use akr_core::json::Value;

/// One tool, as `tools/list` reports it.
pub struct Tool {
    /// The tool name.
    pub name: &'static str,
    /// One line, shown to the agent.
    pub description: &'static str,
    /// Whether it writes to `.akr/records/`.
    pub writes: bool,
}

/// The catalogue, in §2's order.
pub const TOOLS: &[Tool] = &[
    Tool {
        name: "knowledge.search",
        description: "Search the ledger. Ranks; never authorises — nothing enters a context \
                      bundle because it matched a query. A missing or stale disposable index \
                      is refreshed from the loaded ledger before results are returned.",
        writes: false,
    },
    Tool {
        name: "knowledge.start",
        description: "Orient a new task to nearby planning records before proposing context.",
        writes: false,
    },
    Tool {
        name: "knowledge.explain",
        description: "Explain a diagnostic code, rule identifier, or record kind.",
        writes: false,
    },
    Tool {
        name: "knowledge.get",
        description: "Retrieve one record by reference. `detail` controls the size: \
                      `summary`, `body` (default), or `canonical` for the raw AKR source \
                      text. Ask for canonical only when you need the syntax itself.",
        writes: false,
    },
    Tool {
        name: "knowledge.context",
        description: "Assemble the deterministic context bundle for a goal: what governs \
                      this work, what it rests on, and what is questionable.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_list",
        description: "List registered source documents and their retention state.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_add",
        description: "Register an immutable source document from a local path.",
        writes: true,
    },
    Tool {
        name: "knowledge.source_search",
        description: "Search the immutable source library — registered outside advice, \
                      audits and reports. Results are NON-AUTHORITATIVE: they say where a \
                      passage is, never that the project adopted it. Punctuation is safe \
                      here; `mode: \"literal\"` verifies an exact substring.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_get",
        description: "Read a passage of a registered source. NON-AUTHORITATIVE. The \
                      default detail is `section`, not the whole document; ask for \
                      `whole` only when the entire report is genuinely wanted.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_verify",
        description: "Verify hashes and retained fragments of every registered source.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_supersede",
        description: "Replace a registered source with a new immutable version.",
        writes: true,
    },
    Tool {
        name: "knowledge.source_status",
        description: "Show one source's availability, references, and retained fragments.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_dependents",
        description: "List exact and lineage record references to a source.",
        writes: false,
    },
    Tool {
        name: "knowledge.source_finalize",
        description: "Retain cited fragments or metadata, then optionally remove the full source.",
        writes: true,
    },
    Tool {
        name: "knowledge.impact",
        description: "What rests on a record, or what a commit range would invalidate. Call \
                      before proposing a supersession.",
        writes: false,
    },
    Tool {
        name: "knowledge.validate",
        description: "Run stages A-D over the ledger as it stands on disk. Call after a \
                      batch of writes and before handing work back to a human.",
        writes: false,
    },
    Tool {
        name: "knowledge.propose",
        description: "Create revision 1 of a new key. An existing key is an error: a \
                      proposal is never silently turned into a revision. Pass `acceptance` \
                      to author its checks — required for a milestone (V-008).",
        writes: true,
    },
    Tool {
        name: "knowledge.revise",
        description: "Create the next revision of an existing key. `base_rev` must equal \
                      the current head, or the call fails with a conflict.",
        writes: true,
    },
    Tool {
        name: "knowledge.supersede",
        description: "Replace a record, disposing of every unfinished part_of child. The \
                      children are listed in the error payload when one is missing.",
        writes: true,
    },
    Tool {
        name: "knowledge.complete",
        description: "Move a milestone or work record to completed. Every acceptance check \
                      must be satisfied by passing evidence — create it first with \
                      knowledge.evidence_add, then cite it here as a D-009 reference, e.g. \
                      {\"checks\": {\"no-placeholder-assets\": \
                      \"@sys.evidence.asset-audit/1\"}}.",
        writes: true,
    },
    Tool {
        name: "knowledge.evidence_add",
        description: "Record what was observed: result, method, and the commit it was \
                      observed at (defaults to HEAD). Deliberately has no field for what \
                      the evidence verifies (D-016) — cite it from a check's verified_by \
                      or from knowledge.complete.",
        writes: true,
    },
    Tool {
        name: "knowledge.papercut",
        description: "Log a small friction hit while working — a tool call that missed \
                      and had to be retried, a confusing setup step, a flaky command, a \
                      stale cache, a misleading error, a non-obvious gotcha. One or two \
                      sentences: what you were doing, what got in the way (a guess at the \
                      cause/fix is a bonus). Do this proactively, in the moment, even \
                      though none of these are blocking — logged together they show where \
                      the project needs sanding down (D-027).",
        writes: true,
    },
];

/// The input schema for a tool, or `None` if the name is unknown.
#[must_use]
pub fn input_schema(name: &str) -> Option<Value> {
    let schema = match name {
        "knowledge.search" => object(
            vec![
                ("query", string("What to search for.")),
                ("kinds", string_array("Restrict to these record kinds.")),
                ("states", string_array("Restrict to these states.")),
                (
                    "limit",
                    integer("Maximum results. Default 20, maximum 100."),
                ),
            ],
            &["query"],
        ),
        "knowledge.start" => object(
            vec![
                ("task", string("What the agent should start working on.")),
                ("paths", string_array("Path globs the work will touch.")),
                ("budget_tokens", integer("Approximate token budget.")),
            ],
            &["task"],
        ),
        "knowledge.explain" => object(
            vec![(
                "subject",
                string("Diagnostic code, rule id, or record kind to explain."),
            )],
            &["subject"],
        ),
        "knowledge.get" => object(
            vec![
                (
                    "ref",
                    string("A reference in any of the four forms of D-009."),
                ),
                ("history", boolean("Include every revision of the key.")),
                (
                    "relations",
                    boolean("Include inbound and outbound relations."),
                ),
                (
                    "detail",
                    string(
                        "`summary` (identity, state, scope, relation counts, freshness, \
                         source locators), `body` (the default: adds slots, claims and \
                         full relations) or `canonical` (adds the raw AKR source text). \
                         Ask for `canonical` only when you need the syntax itself.",
                    ),
                ),
            ],
            &["ref"],
        ),
        "knowledge.context" => object(
            vec![
                (
                    "goal",
                    string("The milestone, work item or track to assemble around."),
                ),
                ("paths", string_array("Path globs the work will touch.")),
                ("budget_tokens", integer("Approximate token budget.")),
            ],
            &["goal"],
        ),
        "knowledge.source_list" => object(
            vec![(
                "all_versions",
                boolean("Include superseded source registrations."),
            )],
            &[],
        ),
        "knowledge.source_add" => object(
            vec![
                ("path", string("Local Markdown file to register.")),
                ("id", string("Optional stable source id.")),
                ("title", string("Optional human-readable title.")),
                ("origin", string("external or internal-reference.")),
                ("observed_at", string("Optional observed Git commit.")),
                ("scope", string("Optional project path glob.")),
            ],
            &["path"],
        ),
        "knowledge.source_search" => object(
            vec![
                ("query", string("What to look for. Punctuation is safe.")),
                (
                    "mode",
                    string(
                        "`text` (default) escapes punctuation into ordinary terms, \
                         `literal` verifies an exact substring against the registered \
                         bytes, `fts` passes a raw FTS5 expression through.",
                    ),
                ),
                (
                    "documents",
                    string_array("Restrict to these registered source ids."),
                ),
                (
                    "all_versions",
                    boolean("Include documents a later registration supersedes."),
                ),
                (
                    "limit",
                    integer("Maximum results. Default 10, maximum 100."),
                ),
            ],
            &["query"],
        ),
        "knowledge.source_get" => object(
            vec![
                (
                    "chunk",
                    string("A chunk id from knowledge.source_search. Exclusive with `id`."),
                ),
                (
                    "id",
                    string("A registered source id. Exclusive with `chunk`."),
                ),
                (
                    "detail",
                    string(
                        "`snippet` (the chunk alone), `section` (the chunk and its \
                         neighbours, the default) or `whole` (the entire document).",
                    ),
                ),
                (
                    "lines",
                    string("A line range `a:b` within the document, with `id`."),
                ),
            ],
            &[],
        ),
        "knowledge.source_verify" => object(Vec::new(), &[]),
        "knowledge.source_supersede" => object(
            vec![
                ("old_id", string("Existing source id.")),
                ("new_path", string("Local replacement Markdown file.")),
                ("new_id", string("Optional replacement source id.")),
            ],
            &["old_id", "new_path"],
        ),
        "knowledge.source_status" => object(vec![("id", string("Source id."))], &["id"]),
        "knowledge.source_dependents" => object(vec![("id", string("Source id."))], &["id"]),
        "knowledge.source_finalize" => object(
            vec![
                ("id", string("Source id.")),
                ("retain", string("cited (default) or metadata.")),
                ("context", string("exact or block (default).")),
                (
                    "remove_file",
                    boolean("Remove the full source after durable retention."),
                ),
                (
                    "dry_run",
                    boolean("Report the plan without changing files."),
                ),
            ],
            &["id"],
        ),
        "knowledge.impact" => object(
            vec![
                (
                    "ref",
                    string("A record reference. Exclusive with git_diff."),
                ),
                (
                    "git_diff",
                    string("A commit range `A..B`, full 40-hex on both ends."),
                ),
                (
                    "depth",
                    integer("Maximum propagation depth. Default unbounded."),
                ),
            ],
            &[],
        ),
        "knowledge.validate" => object(
            vec![(
                "review_clean",
                boolean("Also fail when the review queue is not empty."),
            )],
            &[],
        ),
        "knowledge.propose" => object(
            vec![
                (
                    "key",
                    string(
                        "The new logical key, dot-delimited: namespace.topic.slug. The \
                         first segment must be a namespace declared in .akr/project.akr.",
                    ),
                ),
                ("kind", kind_schema()),
                ("title", string("The one-line label.")),
                ("state", string("Override the class's initial state.")),
                ("scope", scope_schema()),
                (
                    "topic",
                    string("The exclusivity handle, normative kinds only."),
                ),
                ("slots", slots_schema()),
                ("claims", claims_schema()),
                ("relations", relations_schema()),
                (
                    "acceptance",
                    acceptance_schema("Required to propose a milestone (V-008)."),
                ),
                ("sources", sources_schema()),
            ],
            &["key", "kind", "title"],
        ),
        "knowledge.revise" => object(
            vec![
                ("key", string("The key to revise.")),
                ("title", string("Replace the title.")),
                ("state", string("Move along the class's lifecycle.")),
                ("scope", scope_schema()),
                ("slots", slots_schema()),
                ("claims", claims_schema()),
                (
                    "retired_claims",
                    string_array("Anchors this revision drops (D-011)."),
                ),
                ("relations", relations_schema()),
                (
                    "acceptance",
                    acceptance_schema(
                        "Replaces the acceptance block. Omit to keep the head's checks.",
                    ),
                ),
                ("sources", sources_schema()),
                (
                    "base_rev",
                    integer(
                        "The revision the edit was made against. Must equal the current \
                         head, or the call fails with a conflict.",
                    ),
                ),
            ],
            &["key", "base_rev"],
        ),
        "knowledge.supersede" => object(
            vec![
                ("old_key", string("The key whose head is retired.")),
                (
                    "new_key",
                    string("The superseding key. Defaults to old_key."),
                ),
                ("slots", slots_schema()),
                ("dispositions", dispositions_schema()),
            ],
            &["old_key"],
        ),
        "knowledge.complete" => object(
            vec![
                ("key", string("The milestone or work record to complete.")),
                (
                    "checks",
                    Value::object(vec![
                        ("type", Value::string("object")),
                        (
                            "description",
                            Value::string(
                                "check id -> evidence reference, in a D-009 form: \
                                 {\"no-placeholder-assets\": \
                                 \"@sys.evidence.asset-audit/1\"}. Create the evidence \
                                 with knowledge.evidence_add first.",
                            ),
                        ),
                        (
                            "additionalProperties",
                            Value::object(vec![("type", Value::string("string"))]),
                        ),
                    ]),
                ),
            ],
            &["key"],
        ),
        "knowledge.evidence_add" => object(
            vec![
                (
                    "key",
                    string(
                        "The new evidence key, dot-delimited: namespace.topic.slug, e.g. \
                         sys.evidence.asset-audit.",
                    ),
                ),
                (
                    "result",
                    enumeration("What was observed.", &["pass", "fail", "inconclusive"]),
                ),
                (
                    "method",
                    enumeration(
                        "How it was observed.",
                        &["manual", "command", "observation"],
                    ),
                ),
                (
                    "command",
                    string("The exact command that was run, for method command."),
                ),
                (
                    "artifact",
                    string("A repository path to the artefact, if one exists."),
                ),
                ("summary", string("One line on what was observed.")),
                (
                    "observed_at",
                    string(
                        "The full 40-hex commit the observation was made at. Defaults to \
                         HEAD.",
                    ),
                ),
                (
                    "title",
                    string("The one-line label. Defaults to the summary or the key."),
                ),
            ],
            &["key", "result", "method"],
        ),
        "knowledge.papercut" => object(
            vec![
                (
                    "message",
                    string(
                        "One or two sentences: what you were doing, what got in the way, \
                         and optionally a guess at the cause or fix.",
                    ),
                ),
                (
                    "agent",
                    string("Who hit it: your model or harness name, e.g. \"claude\"."),
                ),
                (
                    "namespace",
                    string(
                        "Namespace for the key. Needed only when the project declares \
                         several.",
                    ),
                ),
                (
                    "about",
                    string(
                        "What the friction was WITH, when that is not this project — a \
                         tool name such as \"akr\". Leave it out for this project's own \
                         code or setup. Use it whenever the thing that got in your way \
                         was the tooling rather than the repository you are working in: \
                         that is how the report reaches whoever maintains the tool.",
                    ),
                ),
            ],
            &["message", "agent"],
        ),
        _ => return None,
    };
    Some(schema)
}

/// The output schema for each tool.
pub fn output_schema(name: &str) -> Option<Value> {
    match name {
        "knowledge.search"
        | "knowledge.start"
        | "knowledge.explain"
        | "knowledge.get"
        | "knowledge.context"
        | "knowledge.source_list"
        | "knowledge.source_add"
        | "knowledge.source_search"
        | "knowledge.source_get"
        | "knowledge.source_verify"
        | "knowledge.source_supersede"
        | "knowledge.source_status"
        | "knowledge.source_dependents"
        | "knowledge.source_finalize"
        | "knowledge.impact"
        | "knowledge.validate"
        | "knowledge.propose"
        | "knowledge.revise"
        | "knowledge.supersede"
        | "knowledge.complete"
        | "knowledge.evidence_add"
        | "knowledge.papercut" => Some(Value::object(vec![("type", Value::string("object"))])),
        _ => None,
    }
}

/// `kind`: the closed enumeration of D-001, with each kind's required content slots in
/// the description — generated from the same tables the type-checker reads, so an agent
/// learns what a kind needs before the first `AKR-T001` rather than from it.
fn kind_schema() -> Value {
    let mut description = String::from("The record kind. Required slots per kind: ");
    for (index, kind) in akr_core::model::Kind::ALL.iter().enumerate() {
        if index > 0 {
            description.push_str("; ");
        }
        let required: Vec<&str> = kind
            .content_slots()
            .iter()
            .filter(|spec| spec.required)
            .map(|spec| spec.slot.name())
            .collect();
        description.push_str(kind.name());
        description.push_str(": ");
        if required.is_empty() && !kind.requires_acceptance() {
            description.push_str("(none)");
        } else {
            description.push_str(&required.join(", "));
            if kind.requires_acceptance() {
                if !required.is_empty() {
                    description.push_str(", ");
                }
                description.push_str("acceptance (V-008)");
            }
        }
    }
    description.push('.');
    Value::object(vec![
        ("type", Value::string("string")),
        ("description", Value::string(description)),
        (
            "enum",
            Value::array(
                akr_core::model::Kind::ALL
                    .iter()
                    .map(|kind| Value::string(kind.name()))
                    .collect(),
            ),
        ),
    ])
}

fn object(properties: Vec<(&str, Value)>, required: &[&str]) -> Value {
    Value::object(vec![
        ("type", Value::string("object")),
        (
            "properties",
            Value::Object(
                properties
                    .into_iter()
                    .map(|(name, schema)| (name.to_owned(), schema))
                    .collect(),
            ),
        ),
        (
            "required",
            Value::array(required.iter().map(|name| Value::string(*name)).collect()),
        ),
        ("additionalProperties", Value::bool(false)),
    ])
}

fn string(description: &str) -> Value {
    Value::object(vec![
        ("type", Value::string("string")),
        ("description", Value::string(description)),
    ])
}

fn integer(description: &str) -> Value {
    Value::object(vec![
        ("type", Value::string("integer")),
        ("description", Value::string(description)),
    ])
}

fn boolean(description: &str) -> Value {
    Value::object(vec![
        ("type", Value::string("boolean")),
        ("description", Value::string(description)),
    ])
}

fn enumeration(description: &str, values: &[&str]) -> Value {
    Value::object(vec![
        ("type", Value::string("string")),
        ("description", Value::string(description)),
        (
            "enum",
            Value::array(values.iter().map(|v| Value::string(*v)).collect()),
        ),
    ])
}

fn string_array(description: &str) -> Value {
    Value::object(vec![
        ("type", Value::string("array")),
        ("description", Value::string(description)),
        (
            "items",
            Value::object(vec![("type", Value::string("string"))]),
        ),
    ])
}

/// `slots` accepts any slot of the record's kind, including `note` on planning kinds
/// (D-026). The kind's own table is what validates it, so the schema stays open here and
/// the refusal — `AKR-T002` for a slot the kind does not have — comes from the grammar.
fn slots_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("object")),
        (
            "description",
            Value::string(
                "Content slots for the record's kind. Planning kinds accept `note` (D-026): \
                 operator commentary, rendered in views for terminal records.",
            ),
        ),
        ("additionalProperties", Value::bool(true)),
    ])
}

fn scope_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("array")),
        (
            "description",
            Value::string("Scope terms: \"all\", a path glob, or a \"@key\" reference."),
        ),
        ("items", Value::object(vec![])),
    ])
}

fn claims_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("array")),
        (
            "description",
            Value::string("Addressable claims, each with an anchor and text."),
        ),
        (
            "items",
            object(
                vec![
                    ("anchor", string("The anchor.")),
                    ("text", string("The claim.")),
                ],
                &["anchor", "text"],
            ),
        ),
    ])
}

fn relations_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("object")),
        (
            "description",
            Value::string("relation name -> array of references."),
        ),
        ("additionalProperties", string_array("References.")),
    ])
}

fn source_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("object")),
        (
            "description",
            Value::string("One source attribution for the record."),
        ),
        (
            "properties",
            Value::Object(
                vec![
                    (
                        "kind".to_owned(),
                        enumeration(
                            "legacy, external or internal",
                            &["legacy", "external", "internal"],
                        ),
                    ),
                    (
                        "path".to_owned(),
                        string("Path to the source file this record was authored from."),
                    ),
                    ("url".to_owned(), string("URL to the source material.")),
                    (
                        "excerpt".to_owned(),
                        string("Excerpt of the source material associated with this record."),
                    ),
                    (
                        "document".to_owned(),
                        string("Registered source document id for an exact citation."),
                    ),
                    (
                        "role".to_owned(),
                        enumeration(
                            "How the source contributes to this record.",
                            &["origin", "rationale", "evidence", "constraint", "example"],
                        ),
                    ),
                    (
                        "start_byte".to_owned(),
                        integer("First cited byte, inclusive."),
                    ),
                    (
                        "end_byte".to_owned(),
                        integer("First byte after the cited passage."),
                    ),
                    (
                        "start_line".to_owned(),
                        integer("First cited line, one-based."),
                    ),
                    (
                        "end_line".to_owned(),
                        integer("Last cited line, one-based."),
                    ),
                    (
                        "excerpt_hash".to_owned(),
                        string("Optional sha256 hash of the cited bytes."),
                    ),
                    (
                        "use".to_owned(),
                        string("What the project adopted or retained from this source."),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
        ),
        (
            "required",
            Value::array(vec![Value::string("kind".to_owned())]),
        ),
        ("additionalProperties", Value::bool(false)),
    ])
}

fn sources_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("array")),
        (
            "description",
            Value::string("Source attributions for the record."),
        ),
        ("items", source_schema()),
    ])
}

/// `acceptance`: the checks a milestone or work record must satisfy to complete (D-016).
/// Milestones require a non-empty acceptance block to exist at all (V-008), so this is
/// what lets `knowledge.propose` create one instead of forcing `akr propose --from`.
fn acceptance_schema(description: &str) -> Value {
    Value::object(vec![
        ("type", Value::string("array")),
        ("description", Value::string(description)),
        (
            "items",
            object(
                vec![
                    (
                        "id",
                        string("The check identifier, unique within the record."),
                    ),
                    ("statement", string("The observable outcome.")),
                    ("method", string("manual, command or observation.")),
                    ("command", string("The exact command, for method command.")),
                    (
                        "verified_by",
                        string_array("Evidence references that already satisfy this check."),
                    ),
                ],
                &["id", "statement", "method"],
            ),
        ),
    ])
}

fn dispositions_schema() -> Value {
    Value::object(vec![
        ("type", Value::string("array")),
        (
            "description",
            Value::string(
                "One entry per unfinished part_of child (D-017). A missing one is refused \
                 and the children are listed in the error payload.",
            ),
        ),
        (
            "items",
            object(
                vec![
                    ("child", string("The child's key.")),
                    (
                        "outcome",
                        string(
                            "carried_forward, completed_elsewhere, intentionally_dropped or still_required_separately.",
                        ),
                    ),
                    (
                        "into",
                        string("Where it went, where the outcome needs one."),
                    ),
                    ("note", string("Why.")),
                ],
                &["child", "outcome"],
            ),
        ),
    ])
}
