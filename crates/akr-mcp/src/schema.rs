//! The nine tool declarations of `docs/08-mcp.md` §2, with their JSON schemas.
//!
//! The list is **closed for 0.1** — no `knowledge.query`, no `knowledge.build`, no
//! `knowledge.delete`, no `knowledge.evidence_add`. §2 gives a reason for each absence and
//! they are all the same reason: a tool is a contract, and every tool that exists is one
//! an agent will lean on. The escape valve is `knowledge.search`, which returns records
//! rather than rows (§6).

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
                      bundle because it matched a query.",
        writes: false,
    },
    Tool {
        name: "knowledge.get",
        description: "Retrieve one record by reference, with its relations, freshness and \
                      canonical source text.",
        writes: false,
    },
    Tool {
        name: "knowledge.context",
        description: "Assemble the deterministic context bundle for a goal: what governs \
                      this work, what it rests on, and what is questionable.",
        writes: false,
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
                      proposal is never silently turned into a revision.",
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
                      must be satisfied by passing evidence.",
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
                ("limit", integer("Maximum results. Default 20, maximum 100.")),
            ],
            &["query"],
        ),
        "knowledge.get" => object(
            vec![
                ("ref", string("A reference in any of the four forms of D-009.")),
                ("history", boolean("Include every revision of the key.")),
                ("relations", boolean("Include inbound and outbound relations.")),
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
                ("format", enumeration("json or text.", &["json", "text"])),
            ],
            &["goal"],
        ),
        "knowledge.impact" => object(
            vec![
                ("ref", string("A record reference. Exclusive with git_diff.")),
                (
                    "git_diff",
                    string("A commit range `A..B`, full 40-hex on both ends."),
                ),
                ("depth", integer("Maximum propagation depth. Default unbounded.")),
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
                ("key", string("The new logical key.")),
                ("kind", string("The record kind.")),
                ("title", string("The one-line label.")),
                ("state", string("Override the class's initial state.")),
                ("scope", scope_schema()),
                ("topic", string("The exclusivity handle, normative kinds only.")),
                ("slots", slots_schema()),
                ("claims", claims_schema()),
                ("relations", relations_schema()),
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
                ("new_key", string("The superseding key. Defaults to old_key.")),
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
                            Value::string("check id -> evidence reference."),
                        ),
                        ("additionalProperties", Value::object(vec![("type", Value::string("string"))])),
                    ]),
                ),
            ],
            &["key"],
        ),
        _ => return None,
    };
    Some(schema)
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
        ("items", Value::object(vec![("type", Value::string("string"))])),
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
                vec![("anchor", string("The anchor.")), ("text", string("The claim."))],
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
                    ("outcome", string("carried_forward, completed_elsewhere, intentionally_dropped or still_required_separately.")),
                    ("into", string("Where it went, where the outcome needs one.")),
                    ("note", string("Why.")),
                ],
                &["child", "outcome"],
            ),
        ),
    ])
}
