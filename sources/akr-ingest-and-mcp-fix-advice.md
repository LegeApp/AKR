# Verdict

Your core idea is sound, but the correct abstraction is:

**Markdown semantic item → review candidate → reviewed disposition → optional AKR operation**

not:

**physical Markdown line → AKR record**

That distinction preserves the project’s existing authority, provenance, lifecycle, and validation rules while making outside-advisor ingest much less ceremonial.

The attached audit shows why the present heading-oriented importer is too coarse. A single heading contains ten independent decoder findings, each of which should be reviewable separately.  It also contains prose recommendations immediately followed by code examples, exactly matching your proposed “claim plus attached implementation example” structure. 

On this file:

* The current heading-based extractor produces about **61 claims**.
* A straightforward semantic-block scanner produced **438 review candidates**: 275 list items, 126 prose paragraphs, 36 table rows, and one blockquote.
* Twelve fenced code blocks attached cleanly to preceding candidates.
* No code block in this document was truly orphaned.

The exact candidate count should not become a golden requirement because table and nested-list policy can change it. The result nevertheless demonstrates that item-oriented extraction is the right level.

## Where the literal version would conflict with AKR

| Literal behavior                                           | Conflict                                                                                       | Compatible behavior                                                                                       |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Every physical line becomes a record                       | AKR explicitly does not treat every agent output or source fragment as durable knowledge       | Every semantic item becomes a **review candidate**, not a record                                          |
| An agent writes `x`, therefore a work record is complete   | AKR completion requires typed acceptance checks and evidence                                   | `x` means “reviewed as apparently satisfied”; canonical completion remains a separate validated operation |
| “Depends on line 17” is stored as the relation             | Line numbers are source coordinates, not stable identity                                       | Accept line-relative shorthand, then immediately resolve it to a stable candidate ID                      |
| A code block is recorded as evidence                       | AKR evidence has stronger empirical semantics: result, method, observation, and provenance     | Store code as **supporting source context** or an implementation example                                  |
| All candidates become proposed records and tracking checks | This would flood the ledger with connective prose, duplicate findings, and non-durable context | Only promoted candidates create or revise records                                                         |
| Keyword parsing chooses the kind                           | This produces accidental kinds from ordinary prose                                             | Extract without classifying; kind selection happens during review or promotion                            |
| Existing `akr import` silently changes behavior            | It has an existing legacy-migration contract                                                   | Keep it and introduce a separate `akr ingest` path                                                        |

The project’s D-020 boundary also remains intact: an agent may extract, draft, compare, and recommend dispositions, but its review does not decide canonical authority or acceptance.

# One template is enough—with one qualification

You do not need separate extraction templates for code advice, planning, observations, constraints, and implementation examples.

You do need to distinguish:

1. The **source review schema**, which can be one generic template.
2. The **canonical AKR operation** resulting from that review.

For example, the audit’s “do not do these things first” bullets are durable constraints or decisions, not ordinary implementation tasks.  The same generic review template can ingest them, but promoting all of them as `work` records would discard useful semantics.

A candidate should therefore answer:

```text
What did the source say?
What is our disposition?
What existing or proposed record does it concern?
What does it depend on?
What supports our disposition?
Has the resulting AKR operation been applied?
```

It should not need to know its final AKR kind during extraction.

# Do not overload “completed”

“Completed” can mean at least four different things here:

1. The candidate has been reviewed.
2. The recommended change appears implemented.
3. The information is already represented in AKR.
4. A canonical work record has passed its acceptance checks.

Those cannot safely share one state.

A one-character review interface still works, provided the character maps to a typed disposition:

| Character | Typed disposition   | Required information            | Canonical effect                                |
| --------: | ------------------- | ------------------------------- | ----------------------------------------------- |
|       `?` | Pending             | None                            | Blocks review closure                           |
|       `+` | Promote             | Create/revise/attach plan       | Produces proposed AKR operations                |
|       `x` | Verified satisfied  | Basis or evidence reference     | Does **not** automatically close canonical work |
|       `=` | Already represented | Existing record reference       | Links candidate to existing knowledge           |
|       `-` | Declined            | Optional reason                 | No record created                               |
|       `~` | Split or partial    | Child candidates or explanation | Blocks promotion until decomposed               |
|       `!` | Contradicted        | Basis and optional target       | Preserves the disagreement in the manifest      |

A candidate is “reviewed” whenever its disposition is not `?`. Whether the underlying recommendation is canonically complete remains governed by normal AKR rules.

Dependencies should not be another disposition character. They are an orthogonal field:

```text
+ c_0042 depends=^
+ c_0043 depends=@c_0017,@akr.work.decoder-api/2
x c_0044 basis=@akr.evidence.decoder-profile/3
= c_0045 target=@akr.decision.parallelism-policy/1
```

`^` can mean “the preceding candidate” at the CLI boundary, but the stored form must contain the resolved candidate ID.

# Recommended ingest model

I would add a new core subsystem rather than extending the current legacy importer until both behaviors are difficult to distinguish:

```text
crates/akr-core/src/ingest/
    mod.rs
    markdown.rs
    manifest.rs
    review.rs
    apply.rs

crates/akr-cli/src/ingest.rs
crates/akr-mcp/src/ingest.rs
```

The existing `crates/akr-core/src/import/mod.rs` remains the curated legacy migration path. Its current behavior—one heading, one first paragraph, keyword classification, legacy provenance—should not become the basis for arbitrary advisor reports.

## Core data structures

A suitable model would look roughly like this:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    Paragraph,
    ListItem,
    TableRow,
    BlockQuote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Pending,
    Promote,
    VerifiedSatisfied,
    AlreadyRepresented,
    Declined,
    Split,
    Contradicted,
}

impl TryFrom<char> for Disposition {
    type Error = ReviewError;

    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '?' => Ok(Self::Pending),
            '+' => Ok(Self::Promote),
            'x' | 'X' => Ok(Self::VerifiedSatisfied),
            '=' => Ok(Self::AlreadyRepresented),
            '-' => Ok(Self::Declined),
            '~' => Ok(Self::Split),
            '!' => Ok(Self::Contradicted),
            other => Err(ReviewError::UnknownDisposition(other)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone)]
pub struct SupportBlock {
    pub span: SourceSpan,
    pub language: Option<String>,
    pub raw_text: String,
}

#[derive(Debug, Clone)]
pub struct IngestCandidate {
    /// Opaque identity assigned when the manifest is created.
    pub id: CandidateId,

    /// Used to match unchanged candidates during a later re-ingest.
    /// This must use a stable digest, never DefaultHasher.
    pub fingerprint: CandidateFingerprint,

    /// Display coordinate only. This is not identity.
    pub ordinal: u32,

    pub source_span: SourceSpan,
    pub section_path: Vec<String>,
    pub parent: Option<CandidateId>,
    pub kind: CandidateKind,

    /// Exact source text.
    pub raw_text: String,

    /// Optional normalized display/search representation.
    pub semantic_text: String,

    pub support: Vec<SupportBlock>,
    pub review: CandidateReview,
}

#[derive(Debug, Clone)]
pub struct CandidateReview {
    pub disposition: Disposition,
    pub promotion: Option<PromotionPlan>,
    pub target: Option<RecordRef>,
    pub basis: Vec<RecordRef>,
    pub relations: Vec<StagedRelation>,
    pub note: Option<String>,

    /// Set by the system after a successful atomic apply.
    pub applied_as: Option<RecordRef>,
}

#[derive(Debug, Clone)]
pub enum PromotionPlan {
    Create {
        kind: RecordKind,
        requested_key: Option<String>,
    },
    Revise {
        target: RecordRef,
    },
    AttachSource {
        target: RecordRef,
    },
}
```

The character is merely an input projection. Internally and over MCP, use the typed disposition names.

Validation should make ambiguous states impossible:

```rust
pub fn validate_candidate_review(
    candidate: &IngestCandidate,
) -> Result<(), ReviewDiagnostic> {
    let review = &candidate.review;

    match review.disposition {
        Disposition::Pending => {}

        Disposition::Promote if review.promotion.is_none() => {
            return Err(ReviewDiagnostic::PromotionPlanRequired {
                candidate: candidate.id.clone(),
            });
        }

        Disposition::VerifiedSatisfied if review.basis.is_empty() => {
            return Err(ReviewDiagnostic::VerificationBasisRequired {
                candidate: candidate.id.clone(),
            });
        }

        Disposition::AlreadyRepresented if review.target.is_none() => {
            return Err(ReviewDiagnostic::ExistingTargetRequired {
                candidate: candidate.id.clone(),
            });
        }

        Disposition::Split if candidate_has_no_children(candidate) => {
            return Err(ReviewDiagnostic::SplitChildrenRequired {
                candidate: candidate.id.clone(),
            });
        }

        _ => {}
    }

    validate_staged_relations(candidate)?;
    Ok(())
}
```

## Manifest storage

Pending candidates should not live in `.akr/records`.

Use a versioned review namespace such as:

```text
.akr/reviews/<ingest-id>/manifest.json
.akr/reviews/<ingest-id>/source.md
```

The source snapshot matters. Otherwise a reviewer can mark candidate 37 against one version of a document while the path now contains different text.

The manifest should contain:

* Source kind, normally `external`.
* Original path or URL as provenance.
* Exact source snapshot or content-addressed reference.
* Source digest.
* Extractor version.
* Manifest version for optimistic concurrency.
* Candidates and dispositions.
* Diagnostics and unresolved relations.
* Applied record references.

This is a genuine format extension and should receive a decision record and schema. It should not be hidden in `.akr/cache`: review progress is durable user state, not reconstructable derived data.

Review-only changes should increment the manifest version without forcing a knowledge-ledger rebuild. Applying promoted candidates changes both the manifest and the normal ledger atomically.

# Markdown extraction rules

The physical-line model needs refinement because Markdown permits arbitrary prose wrapping. These two source fragments are semantically identical:

```markdown
Repair the full-plane allocation before adding
architecture-specific SIMD.
```

```markdown
Repair the full-plane allocation before adding architecture-specific SIMD.
```

Treating them as two claims in the first form would make source formatting affect knowledge semantics.

A deterministic subset parser is sufficient; a complete CommonMark implementation is unnecessary.

## Recommended rules

1. **Headings update section context.**
   ATX and setext headings do not become candidates, but their complete path is retained.

2. **Paragraphs become candidates.**
   Consecutive prose lines are one paragraph candidate. Preserve the raw bytes while also producing a joined semantic form for display and search.

3. **Each list item becomes a candidate.**
   Continuation paragraphs and code belonging to the item remain with it. Nested list items become separate candidates with `parent` relationships.

4. **Code becomes support.**
   Fenced and indented code attaches to the nearest preceding candidate in the same section and list scope. A code block before any candidate produces an `orphan_support` diagnostic rather than silently attaching to an unrelated item.

5. **Tables need an explicit policy.**
   In `rows` mode, the header and delimiter are context while every data row is a candidate. In `support` mode, the whole table attaches to the preceding candidate. I would use `rows` for your strict audit workflow and `support` for ordinary prose imports.

6. **Source task checkboxes are content, not AKR state.**
   An imported `- [x]` must not automatically become disposition `x`.

7. **Thematic breaks, comments, reference definitions, and blank lines are not candidates.**

8. **Unsupported structures produce diagnostics.**
   Embedded HTML, malformed fences, and ambiguous table syntax should not be guessed away.

A scanner can remain hand-written and dependency-free, consistent with the project’s current dependency restriction:

```rust
pub fn extract_markdown_items(
    source: &str,
    options: ExtractOptions,
) -> Extraction {
    let mut scanner = MarkdownScanner::new(source);
    let mut section = SectionPath::default();
    let mut candidates: Vec<IngestCandidate> = Vec::new();
    let mut diagnostics = Vec::new();

    while let Some(block) = scanner.next_block() {
        match block.kind {
            BlockKind::Heading { level, title } => {
                section.set(level, title);
            }

            BlockKind::Paragraph
            | BlockKind::ListItem { .. }
            | BlockKind::BlockQuote => {
                candidates.push(candidate_from_block(
                    block,
                    &section,
                    options.extractor_version,
                ));
            }

            BlockKind::Table { header, rows } => {
                match options.table_mode {
                    TableMode::Rows => {
                        for row in rows {
                            candidates.push(candidate_from_table_row(
                                row,
                                &header,
                                &section,
                                options.extractor_version,
                            ));
                        }
                    }
                    TableMode::Support => {
                        attach_support_or_diagnose(
                            &mut candidates,
                            block.into_support(),
                            &section,
                            &mut diagnostics,
                        );
                    }
                }
            }

            BlockKind::FencedCode { .. }
            | BlockKind::IndentedCode => {
                attach_support_or_diagnose(
                    &mut candidates,
                    block.into_support(),
                    &section,
                    &mut diagnostics,
                );
            }

            BlockKind::Blank
            | BlockKind::ThematicBreak
            | BlockKind::Comment
            | BlockKind::ReferenceDefinition => {}
        }
    }

    assign_stable_ids(&mut candidates);

    Extraction {
        candidates,
        diagnostics,
    }
}
```

The scanner must recognize fences before headings. Otherwise a line beginning with `#` inside a code block can accidentally change section state, which the present lightweight heading parser is vulnerable to.

## Stable identity and changed source files

Candidate identity cannot be its line number.

Use:

* An opaque ID assigned in the manifest.
* A stable fingerprint derived from extractor version, section path, candidate kind, exact semantic text, support digest, and duplicate occurrence.
* Source spans only as audit locators.

When a revised advisor document is ingested:

1. Exact fingerprints carry their previous review state forward.
2. New fingerprints become pending.
3. Missing old candidates remain preserved as unmatched/orphaned; they are not silently deleted.
4. Similar-text matches may be suggested to the reviewer but should never be accepted automatically.
5. The source digest prevents applying reviews against the wrong source snapshot.

There is no perfect identity algorithm for arbitrary rewrites. Preserving old candidates and requiring explicit remapping is safer than pretending fuzzy matching is authoritative.

# Applying the review to AKR

A completed manifest should compile into ordinary AKR operations:

| Disposition         | Apply behavior                                                            |
| ------------------- | ------------------------------------------------------------------------- |
| Promote             | Create or revise a **proposed** record with external provenance           |
| Verified satisfied  | Retain basis and target; do not automatically accept or complete anything |
| Already represented | Link to the existing record                                               |
| Declined            | Preserve disposition only                                                 |
| Split               | Require child candidates before closure                                   |
| Contradicted        | Preserve finding; optionally promote a question, evidence, or decision    |
| Pending             | Prevent final closure                                                     |

Candidate dependencies are resolved during apply:

```text
candidate dependency
    → promoted record created by target candidate
    → existing record named by target candidate
    → error if no canonical target exists
```

The existing graph and cycle validator should run over the complete staged result before anything is written.

The current legacy import creates an acceptance check for every imported claim. Do not repeat that with advisor ingest. A report of this size would produce hundreds of checks before anyone had decided which material was durable.

Use one tracking work record with a few aggregate checks:

```text
[ ] Every candidate has a disposition
[ ] Every promoted candidate has been applied
[ ] Every dependency resolves and the reviewed source digest is final
```

At closure, one evidence record can identify the manifest digest, reviewer, method, date, counts, and unresolved exceptions.

# CLI surface

A narrow CLI would be sufficient:

```text
akr ingest preview report.md --tables rows
akr ingest start report.md --source-kind external
akr ingest show <ingest-id> --pending --limit 50
akr ingest mark <ingest-id> <candidate-id> x --basis @akr.evidence.foo/1
akr ingest mark <ingest-id> <candidate-id> + --depends ^
akr ingest mark <ingest-id> <candidate-id> = --target @akr.work.foo/2
akr ingest apply <ingest-id> --base-version 7
akr ingest close <ingest-id> --base-version 8
```

`apply` can incrementally apply ready `+` candidates while leaving the manifest open. `close` should require no pending candidates, no unresolved split states, and no unapplied promotions.

A generated text projection could support rapid manual editing:

```text
? c_0012 | The public defaults select the slowest shape...
+ c_0013 | Repair irreversible 9/7 allocation behavior... | dep=@c_0012
x c_0014 | Persistent decoder sessions are already used... | basis=@akr.evidence...
= c_0015 | Add decoder telemetry... | target=@akr.work.jpx-telemetry/1
```

The canonical manifest should still store typed fields. Parsing a user-edited projection into typed updates is safer than treating that text file as canonical state.

# MCP design

For MCP, do not use one-character symbols. Explicit enum strings are easier for an agent to interpret and validate, and the token savings from `x` rather than `verified_satisfied` are immaterial.

A batch review call should resemble:

```json
{
  "ingest_id": "ing_01JZP...",
  "base_version": 7,
  "idempotency_key": "review-batch-2026-08-06-03",
  "reviews": [
    {
      "candidate_id": "c_0014",
      "disposition": "verified_satisfied",
      "basis": ["@akr.evidence.decoder-session-audit/1"],
      "note": "Verified against the persistent Jp2Decoder integration."
    },
    {
      "candidate_id": "c_0015",
      "disposition": "promote",
      "promotion": {
        "operation": "create",
        "kind": "work"
      },
      "relations": [
        {
          "kind": "depends_on",
          "target_candidate": "c_0014"
        }
      ]
    }
  ]
}
```

The result should return:

```json
{
  "manifest_version": 8,
  "updated": 2,
  "pending": 173,
  "ready_to_apply": 19,
  "unresolved_dependencies": 2,
  "diagnostics": [],
  "next_pending_candidate": "c_0016"
}
```

I would expose five narrow operations:

```text
knowledge.ingest_preview
knowledge.ingest_start
knowledge.ingest_get
knowledge.ingest_review
knowledge.ingest_apply
```

`ingest_get` should support filtering and cursor pagination rather than returning the whole report repeatedly. Each candidate response should include its section path, immediate neighbors, parent, support blocks, review state, and relevant existing-record matches.

All five should call the same `akr-core` functions as the CLI. That preserves the project’s “one implementation” rule.

## File-access boundary

Do not expose the existing CLI path behavior unchanged to MCP. An agent-facing ingest tool should accept either:

* The source content directly, with a declared provenance path or URL; or
* A workspace-relative path that is canonicalized and proven to remain below the workspace root.

It should not fetch arbitrary URLs itself. The caller can provide the bytes and record the URL as provenance, keeping ingest deterministic.

At minimum:

```rust
pub fn resolve_workspace_file(
    root: &Path,
    requested: &Path,
) -> Result<PathBuf, PathDiagnostic> {
    if requested.is_absolute() {
        return Err(PathDiagnostic::AbsolutePathRejected);
    }

    let canonical_root = root
        .canonicalize()
        .map_err(PathDiagnostic::Root)?;

    let canonical_file = root
        .join(requested)
        .canonicalize()
        .map_err(PathDiagnostic::Source)?;

    if !canonical_file.starts_with(&canonical_root) {
        return Err(PathDiagnostic::OutsideWorkspace);
    }

    Ok(canonical_file)
}
```

Also enforce source-byte, candidate-count, code-block, and individual-line limits before writing a manifest.

# General MCP improvements found in the project

## 1. Add `knowledge.explain`

`AGENTS.md` tells agents to run `akr explain <kind>` when uncertain, but there is no equivalent MCP operation. That forces MCP clients either to guess slot requirements or leave the MCP workflow.

`knowledge.explain` should return:

* Record-kind purpose.
* Required and optional slots.
* Accepted relation types.
* Acceptance and completion rules.
* Small valid examples.
* Common diagnostics.
* Current vocabulary/schema revision.

This is more useful than making the `knowledge.propose` schema enormous with a `oneOf` branch for every record kind.

## 2. Allow provenance in `knowledge.propose` and `knowledge.revise`

The core model supports source blocks, but the current MCP record conversion does not expose them. That makes clean outside-advisor integration impossible through the ordinary MCP path.

Add structured sources:

```json
{
  "sources": [
    {
      "kind": "external",
      "path": "advice/jp2lam-audit.md",
      "excerpt": "Repair the banded 9/7 reconstruction...",
      "ingest_id": "ing_01JZP...",
      "candidate_id": "c_0048"
    }
  ]
}
```

The final two identifiers can remain extension metadata in the manifest initially if changing the record schema is too invasive.

## 3. Make search correct immediately after writes

The current design has an agent workflow gap:

* MCP can write canonical records.
* Search relies on a derived cache.
* MCP has no `knowledge.build`.
* The documentation is inconsistent about whether read operations may rebuild that cache.

The cleanest fix is:

```text
cache revision == ledger revision
    → use persisted index

cache revision != ledger revision
    → perform deterministic current-ledger scan
      or construct a process-local ephemeral index
```

Return the backend in the result:

```json
{
  "backend": "ledger_scan",
  "cache_stale": true,
  "ledger_revision": "...",
  "cache_revision": "..."
}
```

This keeps search correct without allowing read tools to mutate `.akr/cache`, preserving the apparent intent of D-019.

## 4. Add dry-run and idempotency consistently

Every write tool should support:

* `dry_run`
* `base_rev` or manifest `base_version`
* `idempotency_key`
* Planned record/relation/check changes
* Complete diagnostics before mutation

The ingest apply operation especially needs this because one advisor candidate can revise a record while another creates a relation to its resulting revision.

## 5. Preserve actor identity

The MCP write context currently loses meaningful author information. Configure a stable server actor such as:

```text
akr-mcp --actor codex-main
akr-mcp --actor claude-reviewer
```

Do not treat self-declared MCP client metadata as a trusted security identity. It can be retained as auxiliary provenance, but the configured server actor should own the write.

## 6. Add cursor pagination

`knowledge.search`, status reports, validation diagnostics, and ingest candidates will all become too large for single results. Return stable cursors tied to the ledger or manifest revision, and reject cursors after the relevant revision changes.

## 7. Modernize MCP without dropping legacy clients

The server currently hard-codes protocol `2024-11-05`, handles the legacy `initialize` flow, and omits newer result metadata. That is acceptable only as an intentionally legacy-only implementation.

The current official MCP protocol version is `2026-07-28`. Modern servers expose `server/discover` and clients supply protocol metadata per request; current result schemas also include `resultType`. ([Model Context Protocol][1]) Current tool definitions support `outputSchema` and structured results, with deterministic pagination behavior for listings. ([Model Context Protocol][2]) The existing newline-delimited JSON-RPC stdio transport remains valid. ([Model Context Protocol][3])

Do not merely change the constant. Add a compatibility layer:

```rust
const SUPPORTED_PROTOCOLS: &[&str] = &[
    "2026-07-28",
    "2024-11-05",
];

match request.method.as_str() {
    "server/discover" => handle_discover(request),
    "initialize" => handle_legacy_initialize(request),
    _ => dispatch_with_protocol_metadata(request),
}
```

Then add:

* `resultType` to results.
* `outputSchema` to tool descriptions.
* `structuredContent` matching that schema.
* A protocol conformance test matrix for both supported generations.

# Other project issues found during review

## The uploaded archive is not self-contained

This may be an export problem rather than the repository’s normal state, but the ZIP omits directories referenced as required or normative, including `spec/` and `examples/`.

There are also compile-time inclusions of missing files:

* `crates/akr-core/src/store/mod.rs:36` includes `spec/schema/index.sql`.
* `crates/akr-cli/src/commands.rs:1743-1744` includes missing diagnostic-code documents.

`tools/check-design.py` reported seven broken links into the absent examples and skipped several schema/vocabulary checks because their inputs were absent.

I could not run Cargo in this environment because the Rust toolchain was unavailable, but the missing `include_str!` targets independently mean this particular archive cannot compile as supplied.

Add a distribution command that:

1. Packages from the tracked-file list rather than a hand-selected directory.
2. Extracts the resulting archive into a clean temporary directory.
3. Runs `cargo test`.
4. Runs `python tools/check-design.py --strict`.
5. Runs generated-view consistency checks.
6. Fails when normative inputs are missing rather than printing “skipped.”

## Generated views are demonstrably stale

`docs/generated/PAPERCUTS.md` still reports the D-029 completion-definition problem, while D-029 and the corresponding implementation now exist. This is exactly the class of stale-document problem AKR is intended to prevent.

Add:

```text
akr build --check
```

It should regenerate into a temporary location and fail when committed generated outputs differ. CI and the distribution command should run it.

## Small self-description drift

The MCP executable help describes “nine tools,” while the implementation and documentation expose eleven. Derive the count and names from the actual tool registry instead of embedding prose separately.

# Implementation order

## Phase 0: restore trustworthy packaging

* Make the source archive self-contained.
* Make design checks fail closed in CI.
* Add generated-view consistency checking.
* Resolve the cache-rebuild documentation contradiction.

## Phase 1: deterministic ingest core

* Add `IngestCandidate`, manifest, review, and source-snapshot schemas.
* Implement paragraph, list-item, table-row, heading-context, and code-support extraction.
* Disable keyword classification in this path.
* Add golden tests based on the attached audit.
* Preserve the existing legacy importer unchanged.

## Phase 2: review and application

* Add stable candidate IDs and source-version guards.
* Add one-character CLI dispositions.
* Add staged dependencies and resolution.
* Compile promotions into normal `apply_many` operations.
* Keep all resulting records proposed.
* Add aggregate tracking and closure evidence.

## Phase 3: MCP parity

* Add ingest preview/get/review/apply tools.
* Add `knowledge.explain`.
* Add source blocks to propose/revise.
* Add dry-run, idempotency, actor identity, and pagination.
* Make search correct when the persisted cache is stale.

## Phase 4: protocol and review UI

* Add dual legacy/current MCP protocol support.
* Add structured output schemas.
* Build the planned P9 human review interface directly over the same manifest API rather than creating another review representation.

The important correction is narrow: **review every source item, but do not turn every source item into knowledge**. Keep headings as context, code as support, dependencies as typed relations, and one-character marks as a fast interface over an explicit state model. That gives you the seamless external ingest you are after without weakening the parts of AKR that make acceptance, completion, and supersession trustworthy.

[1]: https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning "Versioning and Compatibility - Model Context Protocol"
[2]: https://modelcontextprotocol.io/specification/2026-07-28/server/tools?utm_source=chatgpt.com "Tools"
[3]: https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio?utm_source=chatgpt.com "stdio"
## Assessment

Yes. **This particular run probably used at least as many tokens as reading the Markdown directly, and likely more.** An exact count is impossible because the transcript collapses several large outputs, including a 418-line record slice. The attached audit itself is 925 lines and 6,690 words, roughly 9,000–12,000 model tokens depending on tokenization. The agent additionally consumed:

* A 50-entry file-search result from 2,121 matches.
* Git and filesystem orientation output.
* CLI help.
* Project and record listings.
* Search and `get` results.
* An older July Markdown document.
* Hundreds of lines of raw `.akr` records.
* Generated `ACTIVE-WORK.md` content.
* Repeated code-search output containing AKR and generated-document matches.

More importantly, it failed the **planning-coherency** test. The agent encountered:

1. An unresolved AKR goal.
2. A top-level record with no traversable relations.
3. An older July Markdown file.
4. A newer August audit represented only as proposed records.
5. A missing source document.
6. Raw ledger storage and generated views.

That is worse than merely spending extra tokens. It makes it difficult to know which material is current, accepted, complete, or merely outside advice.

The attached audit is not just a short roadmap. Its stated purpose is to rank structural, algorithmic, integration, parallelism, memory, and code-generation gaps by their likely effect on end-to-end rendering.  It also contains implementation-critical details that cannot safely be reduced to one paragraph—for example, it warns that Rayon’s `for_each_init()` does not necessarily allocate once per worker and supplies a concrete coarse-chunk alternative. 

So the agent’s attempt to locate the original Markdown was rational.

## What the trace reveals

| Trace behavior                                                         | What it means                                                                         |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `knowledge.context` apparently failed because the goal did not resolve | The protocol assumes an exact AKR key before the agent has discovered one             |
| `fff.find_files` returned 50 of 2,121 results                          | The agent had no cheap project-orientation path and fell back to broad file discovery |
| `akr search` immediately found one strong result                       | AKR search worked, but it should have been the guided fallback from the initial error |
| `akr get ... --relations` returned no relations                        | The optimization-plan record was a graph dead end                                     |
| The agent searched for `decode-optimization-results-2026-07-20.md`     | It needed implementation detail that the AKR record did not provide                   |
| The newer August source path no longer existed                         | The import was not recoverable from its original source                               |
| The agent read raw `.akr/records/.../work.akr`                         | The supported read interface was insufficient                                         |
| The agent searched `docs/generated/ACTIVE-WORK.md`                     | The generated projection had become a secondary information database                  |
| All imported records were still `proposed`                             | The audit had been imported but not integrated into the project’s accepted plan       |
| It finally inspected current code before editing                       | The agent’s eventual behavior was correct and cautious                                |

The last point matters. I would not blame the agent here. It correctly noticed that the records were proposed, recovered as much context as possible, and then checked the current code rather than blindly applying an old recommendation.

## The biggest AKR defect: it is currently neither source nor map

A useful AKR deployment should provide one of these two things:

1. A sufficiently detailed, authoritative bundle; or
2. A compact authoritative map with direct, stable pointers to detailed sources.

In this case it provided neither.

The import implementation creates a tracking record whose acceptance checks contain sentences such as:

```text
"6. P1: nonzero tile origins unnecessarily disable optimized DWT"
is dispositioned: promoted as <key> or declined with evidence
```

But the relationship between the tracking record and the imported record exists only inside that string. There is no graph edge. Consequently:

* `akr get --relations` cannot show membership.
* `akr context` cannot traverse from the tracker or top-level plan to the imported findings.
* `akr impact` cannot reason about that grouping.
* Generated views have to rediscover records independently.
* An agent is pushed toward raw-file search.

A plan record with no relations, no useful scope, and no structured membership is effectively a Markdown heading stored in a database.

### Do not fix this by making every imported item `part_of` the active plan

That would create the opposite failure. A large outside audit could add dozens or hundreds of proposed items to every context bundle and to `ACTIVE-WORK.md`.

Use a separate relationship or review-manifest membership, for example:

```text
review_item_of -> audit review work record
```

Its semantics should be:

* Domain: any candidate record kind.
* Range: a review/import tracking `work` record.
* It means “this item is awaiting disposition under this review.”
* It does **not** mean the project has adopted the item.
* It does **not** make the item part of the plan of record.
* Context around the tracker may show a compact review summary and optionally page through its candidates.

After review, promoted implementation work should receive ordinary planning structure:

```text
part_of -> accepted decoder optimization plan
scope   -> lege-codecs/jp2lam/**
```

Normative findings should become proper constraints, decisions, or requirements linked through their normal relations. Declined candidates remain only in the review history.

## P0: fix initial task orientation

The generated `AGENTS.md` currently says, in effect:

```text
1. Call knowledge.context with a milestone/work/track.
2. Use knowledge.search while working.
```

This ordering only works when the agent already knows an exact key. A normal task arrives as “continue the jp2lam decoder optimization,” not as:

```text
lege-ecosystem.work.jp2lam-jpeg-2000-decoder-optimization-plan
```

Change the protocol to distinguish known and unknown goals:

```md
**Starting a task**

When the exact planning key is known:
- MCP: `knowledge.context` with that key and the expected paths.
- CLI: `akr context --goal <key> --paths "<glob>" --budget 4000`.

When the key is not known:
- Search only planning kinds first:
  `knowledge.search` with kinds `milestone`, `work`, and `track`.
- Select a live or explicitly relevant proposed result.
- Then call `knowledge.context` with its exact key.

Do not inspect `.akr/records/` or `docs/generated/` as the first fallback.
```

The unresolved-goal error should itself return candidates. This need not make context fuzzy or nondeterministic. Context can continue requiring an exact key; the error simply provides navigation help.

```rust
#[derive(Debug, Clone)]
pub struct GoalCandidate {
    pub id: RevisionId,
    pub title: String,
    pub state: State,
    pub path_overlap: bool,
}

pub enum ContextError {
    GoalUnresolved {
        input: String,
        candidates: Vec<GoalCandidate>,
    },
    // ...
}
```

The MCP error could then contain:

```json
{
  "code": "AKR-X001",
  "summary": "goal does not resolve to a record",
  "candidates": [
    {
      "ref": "@lege-ecosystem.work.jp2lam-jpeg-2000-decoder-optimization-plan/1",
      "title": "jp2lam JPEG 2000 Decoder Optimization Plan",
      "state": "proposed",
      "path_overlap": true
    }
  ],
  "next": {
    "tool": "knowledge.context",
    "arguments": {
      "goal": "lege-ecosystem.work.jp2lam-jpeg-2000-decoder-optimization-plan",
      "paths": ["lege-codecs/jp2lam/**"],
      "budget_tokens": 4000
    }
  }
}
```

The candidate search can be a deterministic lexical scan over planning-record keys and titles. It does not need FTS, embeddings, or authority semantics.

`knowledge.search` should also return its score in structured JSON—the CLI displays it, but the current JSON omits it—and should provide a recommended context call when one result is clearly dominant.

## P0: make MCP context return the actual context

There is a concrete implementation mismatch in the uploaded AKR source.

The CLI constructs both:

```rust
let text = render_text(&bundle, &model, &freshness);
let result = render_json(&bundle, &model);
```

But `akr-mcp/src/tools.rs::run_read()` discards `Output.text` and returns only `Output.result`.

The JSON context representation then stores only record stubs:

```text
key, rev, kind, state, title, via, depth
```

It does not contain record bodies, claims, source provenance, or full relation detail. Therefore the MCP instructions say “read the bundle in full,” but the MCP client does not actually receive that bundle.

The clean repair is to preserve both outputs:

```rust
pub struct ToolPayload {
    pub text: String,
    pub structured: Value,
}

fn run_read(
    root: &Path,
    command: &Command,
) -> Result<ToolPayload, ToolError> {
    let mut session = open(root, false)?;
    let output = commands::run(&mut session, command)
        .map_err(environment)?;

    if output.exit != Exit::Ok {
        return Err(error_from_output(&session, output));
    }

    Ok(ToolPayload {
        text: output.text,
        structured: output.result,
    })
}
```

Then produce an MCP response like:

```rust
fn content(payload: ToolPayload, is_error: bool) -> Value {
    Value::object(vec![
        (
            "content",
            Value::array(vec![Value::object(vec![
                ("type", Value::string("text")),
                ("text", Value::string(payload.text)),
            ])]),
        ),
        ("structuredContent", payload.structured),
        ("isError", Value::bool(is_error)),
    ])
}
```

This has two advantages:

* The model-facing text is the actual readable context bundle.
* The structured form remains available for clients that need keys, sections, and metadata.

The existing implementation instead pretty-prints the same JSON into `content` and also returns it as `structuredContent`. Depending on the MCP host, that can expose essentially the same payload twice. Even where the host deduplicates it, the text form is poor for an agent because it is JSON metadata rather than the designed context narrative.

The `format` argument on `knowledge.context` should probably be removed. MCP can always return readable text plus structured metadata. The current implementation declares `format: json|text` but does not read the argument.

## P0: the token budget currently does not truncate text

The budgeting code marks record IDs as truncated and subtracts a fixed 20-token estimate for each marked record:

```rust
bundle.truncated.push(entry.id().clone());
bundle.estimated_tokens = total_tokens(bundle, model)
    .saturating_sub(bundle.truncated.len().saturating_mul(20));
```

But the renderer emits the complete body first:

```rust
if let Some(body) = body_of(record) {
    out.push_str(&indent(body, 2));
}
```

and only afterward appends:

```text
(prose truncated to fit the budget)
```

So the prose has not actually been truncated. The annotation is false, and `--budget` does not control the model-visible text.

At minimum:

```rust
if let Some(body) = body_of(record) {
    if let Some(limit) = bundle.prose_limit(&record.id) {
        let shortened = truncate_to_estimated_tokens(body, limit);
        out.push_str(&indent(&shortened, 2));
        out.push_str("  (remaining prose omitted to fit the budget)\n");
    } else {
        out.push_str(&indent(body, 2));
    }
}
```

Do not estimate the new total by subtracting a fixed amount. Produce a truncation plan, render it, estimate the actual rendered result, and reduce further until it fits.

Add regression tests that check the rendered output rather than just comparing CLI JSON to MCP JSON:

```rust
#[test]
fn context_budget_removes_prose_from_rendered_text() {
    let text = render_budgeted_fixture(900);

    assert!(
        estimate_tokens(&text) <= 900,
        "rendered context exceeded budget"
    );
    assert!(
        !text.contains("SENTINEL_PROSE_THAT_MUST_BE_TRUNCATED")
    );
    assert!(text.contains("remaining prose omitted"));
}
```

Until this is fixed, AKR cannot credibly claim controlled context size.

## P0: preserve the full source until review is complete

The current importer extracts one claim per heading and retains only the first paragraph of each section. For this audit, that is insufficient.

The source contains:

* Detailed allocation analysis.
* Code excerpts.
* Warnings about plausible but incorrect implementations.
* Benchmark tables.
* Test matrices.
* Ordering constraints.
* A phased roadmap with expected effects. 
* Explicit guidance about optimizations that should not be attempted first. 

Deleting that document while every imported record is still proposed means the reviewer can no longer verify the drafts or retrieve the omitted details. The AKR documentation itself says archiving or deletion should happen only after the tracking record is complete, but the implementation reports a missing source as only `AKR-M022` warning.

That should become an error when all of the following are true:

* The source belongs to an import tracker.
* The tracker is not completed.
* The current path is missing.
* No immutable snapshot or Git blob is recoverable.

The better model is not merely “keep this mutable path.” Record immutable source identity:

```rust
pub struct Source {
    pub kind: SourceKind,
    pub path: Option<String>,

    /// Repository commit at which the source was imported.
    pub observed_at: Option<CommitId>,

    /// Exact source content.
    pub content_sha256: Option<Sha256Digest>,

    /// Content-addressed fallback for uncommitted or external material.
    pub snapshot: Option<SnapshotId>,

    /// Exact source location of this candidate.
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub heading: Option<String>,

    pub excerpt: Option<String>,
}
```

For a committed Markdown file, AKR can retrieve it from Git even after deletion:

```text
git show <observed-commit>:<path>
```

For uncommitted or externally supplied content, save a content-addressed snapshot:

```text
.akr/sources/sha256/<digest>.md
```

That snapshot is not authoritative project knowledge. It is immutable source evidence.

Add a supported command such as:

```text
akr source show @lege-ecosystem-perf.work.p1-nonzero-tile-origins/1
```

or expose the same locator through `knowledge.get`. An agent should never have to parse raw `.akr` files merely to discover where a record came from.

The ordinary text output of `akr get` should show:

```text
sources
  legacy  lege-codecs/jp2lam/decode-fix-plan/...md
          imported at git:<commit>
          lines 283–316
          current path missing; immutable blob available
```

## P0: separate “outside advice” from “plan of record”

The agent says:

> “The newest audit (2026-08-05) defines a phased roadmap.”

That is factually true about the source document, but AKR says every corresponding record is still `proposed`. Therefore it is not yet the accepted project roadmap.

The context bundle should make the distinction impossible to miss:

```text
PLAN OF RECORD
  none

PENDING ADVISOR REVIEW
  jp2lam decoder performance audit, 2026-08-05
  61 proposed candidates
  0 promoted
  0 declined
  source snapshot available
```

The agent may still use the audit as technical advice and verify it against code, but it should describe it as an unreviewed recommendation rather than current project intent.

After review, the compact normal context should look more like:

```text
PLAN OF RECORD
  jp2lam decoder optimization plan/2 · active

READY WORK
  persistent decoder integration
  9/7 coarse row scratch
  aligned nonzero-origin DWT dispatch

ACTIVE WORK
  9/7 banded reconstruction

CONSTRAINTS
  do not change 9/7 arithmetic without differential tests
  do not introduce unbounded nested Rayon work

SOURCE ASSESSMENT
  2026-08-05 audit · source snapshot available
```

That is where AKR begins to beat rereading the complete report.

## Add an anchoring validation rule

A live planning record should not be able to become invisible to context assembly.

A useful rule would be:

> Every `ready`, `active`, or `blocked` `work` record must have at least one of:
>
> * A `part_of` planning parent.
> * A valid `plan_of_record` role.
> * A non-empty path or reference scope.
> * An explicitly defined top-level planning role.

Proposed review candidates should be exempt because they live in the review grouping rather than the operational plan.

This would have identified the dead-end records before an agent encountered them.

## Keep review candidates out of ordinary `ACTIVE-WORK.md`

The current projection treats `proposed`, `ready`, `active`, and `blocked` work as live. Since the importer defaults unknown claims to `work`, a large audit can flood `ACTIVE-WORK.md` with unreviewed headings.

That creates two problems:

1. Humans cannot distinguish adopted implementation work from imported suggestions.
2. General code search finds generated AKR prose before source code.

Use a separate projection:

```text
docs/generated/REVIEW-CANDIDATES.md
```

or render one collapsed entry in `ACTIVE-WORK.md`:

```text
Import jp2lam decoder audit
  61 pending candidates · source available · review incomplete
```

Only promoted work should appear as individual operational work items.

This also confirms the earlier ingest design principle: **segment every outside item for review, but do not publish every item into the main knowledge graph before disposition.**

## Reduce MCP’s fixed and repeated token costs

These are secondary to the navigation failures, but still worth addressing.

### Add `detail` levels to `knowledge.get`

The current structured result includes parsed slots and claims plus the entire canonical `source_text`, and the MCP protocol may then duplicate that JSON as text.

Use:

```json
{
  "ref": "@lege-ecosystem.work.foo/1",
  "detail": "summary"
}
```

with:

* `summary`: identity, title, state, scope, relation summaries, freshness, source locators.
* `body`: summary plus content slots, claims, checks, and complete relations.
* `canonical`: body plus canonical AKR source text.

Default to `body`. Canonical syntax should be an explicit request.

### Offer a read-only MCP surface

The tool-schema source for all eleven tools is substantial. Depending on the host, all tool names, descriptions, and schemas may be included in the model context even when an implementation agent only needs three read tools.

A useful deployment option would be:

```text
akr-mcp --surface read
akr-mcp --surface full
```

The read surface could expose:

```text
knowledge.search
knowledge.get
knowledge.context
knowledge.impact
knowledge.validate
knowledge.papercut
```

The full surface adds the record-writing and lifecycle tools. This will not solve the trace by itself, but it removes a fixed tax from every coding session.

### Add pagination where results can be large

Imported review candidates, validation diagnostics, and broad searches should return cursors. A context call should summarize a review set rather than return every candidate.

## The `fff` behavior is separately defective

This call is suspicious:

```text
fff.grep({
  "query": "origin",
  "constraints": "lege-codecs/jp2lam/src/decode/reconstruct.rs"
})
```

Yet the results include:

```text
docs/generated/ACTIVE-WORK.md
.akr/records/...
```

Either:

* `constraints` is not a path filter, but its name/tool description led the agent to think it was; or
* The server is failing to enforce the path restriction.

That is not fundamentally an AKR problem, although AKR-generated duplication makes it worse. It should be logged as an FFF papercut.

For code-oriented searches, the default exclusion set should include:

```text
.akr/**
docs/generated/**
**/target/**
**/fuzz/target/**
.git/**
```

with an explicit override for agents intentionally searching project knowledge.

## What an efficient trace should look like

For the same user request, the target behavior should be roughly:

```text
1. knowledge.search
   query: "jp2lam decoder optimization"
   kinds: ["milestone", "work", "track"]
   limit: 5

   → one strong result
   → state: active/proposed clearly shown
   → recommended context call included
```

```text
2. knowledge.context
   goal: "lege-ecosystem.work.jp2lam-jpeg-2000-decoder-optimization-plan"
   paths: ["lege-codecs/jp2lam/**"]
   budget_tokens: 3500

   → actual readable context text
   → accepted plan and children
   → proposed-review warning
   → source assessment and stable source locator
   → acceptance checks and constraints
```

```text
3. knowledge.get
   ref: "@lege-ecosystem-perf.work.p1-nonzero-tile-origins/1"
   detail: "body"

   → exact recommendation
   → current status
   → relation to plan
   → source lines 283–316
```

Then the agent searches `reconstruct.rs` and starts checking implementation status.

It should not need to run:

```text
which akr
akr --help
ls .akr
sed .akr/records/...
grep docs/generated/ACTIVE-WORK.md
find old planning Markdown
```

## Measure this as an agent-interface regression test

You do not need to understand an agent’s internal perception. Treat the agent as a black box and measure observable behavior.

Use this exact scenario as the first golden evaluation:

```text
Task:
"Continue the current jp2lam JPEG 2000 decoder optimization work.
Identify the highest-leverage unfinished low-risk item and implement it."
```

Suggested pass conditions:

| Metric                                             |                   Target |
| -------------------------------------------------- | -----------------------: |
| AKR calls before finding the exact goal            |                      ≤ 2 |
| AKR output before first relevant source-code read  | ≤ 5,000 estimated tokens |
| Reads of raw `.akr/records`                        |                        0 |
| Reads of `docs/generated` for authoritative detail |                        0 |
| Older superseded Markdown selected as current      |                        0 |
| Correctly distinguishes proposed from accepted     |                      yes |
| Can recover full source or exact source locator    |                      yes |
| Reaches a concrete code target                     |                      yes |

Instrument the MCP server with optional JSONL accounting:

```json
{
  "tool": "knowledge.context",
  "input_bytes": 214,
  "text_output_bytes": 10842,
  "structured_output_bytes": 2941,
  "duplicated_bytes": 0,
  "estimated_output_tokens": 3446,
  "records_returned": 14
}
```

Also record the `tools/list` schema size once per session, because that fixed cost may be significant depending on the client.

The most useful aggregate metric is:

> **Total model-visible bytes/tokens from task start until the first relevant source-code inspection, with the correct authoritative plan identified.**

That captures both token efficiency and navigation quality.

## Priority order

1. **Return the actual text context through MCP.**
2. **Make the context budget really truncate rendered text.**
3. **Guide unresolved goals into planning-record search and return a ready next call.**
4. **Expose structured source provenance and preserve an immutable source blob or Git identity.**
5. **Make imported-review membership traversable without making candidates active plan children.**
6. **Require accepted work to be anchored by graph or scope.**
7. **Separate pending review candidates from ordinary active-work projections.**
8. **Remove MCP payload duplication and add `get` detail levels.**
9. **Offer a reduced read-only MCP surface.**
10. **Fix FFF’s apparent path-filtering problem and default excludes.**

The central conclusion is that **Markdown is the better format for the first complete reading of a rich, newly received technical audit**. AKR should not try to beat it at that moment by fragmenting and discarding detail. AKR should win on the second, fifth, and twentieth session by presenting a compact accepted plan, preserving why it is current, and pointing directly to immutable supporting material.

At present this repository is in the most expensive intermediate state: the Markdown has been fragmented, the fragments have not been reviewed, the graph does not connect them, and the detailed source has disappeared. Fixing that transition will improve both token usage and the more important planning-coherency goal.

# Revised decision

Your recap is mostly right. The agent log exposed that the first design optimized the wrong boundary.

The correction is **not** “never ingest Markdown.” It is:

> **Register external Markdown as immutable source material, but do not translate it into AKR records by default.**

AKR should hold the project’s interpretation of the source:

* What was adopted.
* What was rejected or deferred.
* What work exists.
* What has been completed.
* What evidence verifies completion.
* What constraints now govern the project.

The original Markdown should remain intact because it contains the complete reasoning, examples, caveats, benchmarks, and ordering information. The uploaded audit is a good example: it is explicitly intended to identify and rank several different classes of optimization work, not merely provide a checklist.  It also contains implementation details and warnings that are inseparable from the recommendations—for example, the warning about Rayon’s `for_each_init()` and the alternative coarse-chunk implementation. 

This plan therefore supersedes both previous responses for **external advisor documents**.

It does **not** require removing the existing legacy migration workflow. Legacy migration and external-source intake are different operations and should become explicitly separate.

## What is superseded

| Previous recommendation                                    | New decision                                                                                      |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Turn each semantic Markdown item into a review candidate   | Do not do this by default. Semantic items become **search chunks**, not project state             |
| Track one-character dispositions for every item            | Defer this to an optional exhaustive-review mode                                                  |
| Promote candidates into proposed AKR records               | Create records only for advice the project actually adopts, rejects explicitly, or needs to track |
| Delete or archive the source after review                  | Never delete external advisor sources as part of normal review                                    |
| Use imported records as the primary way to find the report | Search the immutable report directly                                                              |
| Let the original source disappear once excerpts exist      | Preserve exact source bytes permanently                                                           |
| Use headings/lines as record identity                      | Use document hashes and byte ranges; lines are display coordinates                                |
| Bring SinoRAG indexes into AKR immediately                 | Retain SQLite FTS5 initially; borrow selected SinoRAG design ideas                                |
| Treat current command latency as a search-index problem    | Fix repeated parsing, resolution, and Git work first                                              |

Your original segmentation idea still has value. It belongs in the **derived search index**, where imperfect segmentation cannot alter project knowledge.

# Target architecture

AKR should have three distinct layers:

```text
┌─────────────────────────────────────────────────────────────┐
│ IMMUTABLE SOURCE LIBRARY                                    │
│                                                             │
│ sources/external/*.md                                       │
│ Exact outside advice, reports, audits, plans and examples   │
│ Non-authoritative, append-only, content-hashed               │
└──────────────────────────┬──────────────────────────────────┘
                           │ deterministic chunking
                           ▼
┌─────────────────────────────────────────────────────────────┐
│ DERIVED SOURCE INDEX                                        │
│                                                             │
│ .akr/cache/index.sqlite                                     │
│ Heading paths, semantic chunks, symbols, BM25, byte ranges  │
│ Rebuildable and non-authoritative                            │
└──────────────────────────┬──────────────────────────────────┘
                           │ citations / source references
                           ▼
┌─────────────────────────────────────────────────────────────┐
│ AKR LEDGER                                                  │
│                                                             │
│ Accepted decisions, requirements, policies, work, evidence  │
│ Authoritative project interpretation and execution state     │
└─────────────────────────────────────────────────────────────┘
```

The source library says:

> “This is what the advisor said.”

The AKR ledger says:

> “This is what the project currently believes and intends to do about it.”

The index says:

> “This is where the relevant material is.”

Those are different responsibilities. Combining them caused the agent failure you observed.

# 1. Split legacy migration from external-source intake

The existing `akr import` is designed as **legacy migration**. Its current Markdown reader extracts one record proposal per heading and retains only the first paragraph under that heading. That can be appropriate when converting an old project Markdown pile into typed project knowledge, but it is unsuitable for retaining a rich outside technical report.

Rename the workflows conceptually:

```text
akr migrate <legacy-document>
    Convert selected durable claims from old internal documentation.
    The legacy document may eventually be retired.

akr source add <external-document>
    Register an immutable outside source.
    No records are automatically created.
    The source remains permanently available.
```

For compatibility:

```text
akr import ...
```

can remain as a deprecated alias for:

```text
akr migrate legacy ...
```

It should warn when used on a document under `sources/external/`.

This distinction should be recorded in the design:

| Workflow               | Input                                | Default output                            | Can original disappear?    |
| ---------------------- | ------------------------------------ | ----------------------------------------- | -------------------------- |
| Legacy migration       | Old internal project Markdown        | Proposed AKR records                      | Yes, after complete review |
| External-source intake | Advisor report, audit, external plan | Source-catalog entry and index chunks     | No                         |
| Selective adoption     | Chosen external recommendations      | Normal AKR records                        | Source remains             |
| Exhaustive review      | Exceptional compliance/audit case    | Review manifest referencing source ranges | Source remains             |

# 2. Use an immutable source library, not merely a protected folder

A folder plus instructions is not enough. Agents and humans will eventually edit a file accidentally.

Use a top-level source area:

```text
sources/
    catalog.akr
    external/
        2026-08-05-jp2lam-decoder-performance-audit--7a2d3c1e.md
    internal-reference/
        ...
```

I would not put this under `docs/`. `docs/` usually implies maintained project documentation. These files are preserved source artifacts.

## Source registration

The registration operation should:

1. Read the exact source bytes.
2. Calculate a stable SHA-256 content hash.
3. Copy the file into `sources/external/`.
4. Add a catalog entry.
5. Index it.
6. Never create work records automatically.

Illustrative catalog syntax:

```akr
source-catalog 0.1

document jp2lam-decoder-performance-audit-2026-08-05 {
    title "jp2lam JPEG 2000 decoder performance audit"
    origin external
    media_type "text/markdown"

    path "sources/external/2026-08-05-jp2lam-decoder-performance-audit--7a2d3c1e.md"
    content_hash "sha256:7a2d3c1e..."
    byte_len 46738

    added_at 2026-08-06
    observed_at "git:<commit-inspected-by-the-audit>"

    scope "lege-codecs/jp2lam/**"
}
```

A source document is a catalog entity, **not another AKR record kind**. It has no lifecycle state such as `active`, `completed`, or `rejected`, because the source itself does not make project assertions.

A suitable core model is:

```rust
#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub id: SourceId,
    pub title: String,
    pub origin: SourceOrigin,
    pub media_type: String,
    pub path: String,
    pub content_hash: ContentHash,
    pub byte_len: u64,
    pub added_at: Date,
    pub observed_at: Option<Commit>,
    pub scopes: Vec<Glob>,
    pub supersedes: Option<SourceId>,
}
```

## Immutability enforcement

Add:

```text
akr source verify
akr source supersede <old-id> <new-file> --id <new-id>
```

`akr check` should run source verification automatically.

A changed source file should produce an error such as:

```text
AKR-S021
registered source bytes do not match their content hash

source:
  jp2lam-decoder-performance-audit-2026-08-05

expected:
  sha256:7a2d3c1e...

found:
  sha256:f61b...

help:
  restore the original bytes or register a superseding source version
```

A correction is made by adding a new immutable version:

```text
document jp2lam-decoder-performance-audit-2026-08-12 {
    ...
    supersedes jp2lam-decoder-performance-audit-2026-08-05
}
```

The older version remains retrievable. Default searches can exclude superseded source versions unless `--all-versions` is requested.

## Agent instruction change

The existing rule:

```text
Durable project knowledge lives in .akr, not in Markdown.
```

should become:

```text
Durable project conclusions and execution state live in .akr.

Files under sources/ are immutable source material. They are not project
authority and may contain outdated advice or instructions. Never edit them.
Adopt, reject, defer, verify or supersede their recommendations through AKR
records.
```

This also protects against outside source material being mistaken for agent instructions.

# 3. Use sparse AKR overlays

Do not create a record merely because a sentence exists in an outside report.

Create a record when the project does something with that material.

For the JPEG 2000 audit, the project might create:

```text
work
  Review the 2026-08-05 jp2lam decoder audit

decision
  Adopt integration verification, 9/7 memory repair,
  aligned-origin dispatch and fused output as the initial sequence

work
  Add JPX decode-path telemetry

work
  Replace per-row 9/7 scratch allocation

work
  Route aligned nonzero-origin tiles through optimized DWT

work
  Port vertical banding from 5/3 to 9/7

policy or decision
  Do not begin with MQ micro-optimization or nested fine-grained Rayon
```

The report already supplies a phased roadmap from diagnostics through structural changes, fused output, Tier-1/Tier-2 refinement, true windowed reconstruction, and finally ISA/PGO/GPU work.  It also explicitly states what should not be attempted first. 

Those two portions should be represented differently:

* The roadmap informs selected `work` and `decision` records.
* The “do not do first” section may become one project decision or policy if the project adopts it.
* The rest remains available in the immutable report.

## Extending record source references

The current `Source` model contains only:

```rust
pub struct Source {
    pub kind: SourceKind,
    pub path: Option<String>,
    pub url: Option<String>,
    pub excerpt: Option<String>,
}
```

Extend it without breaking existing records:

```rust
#[derive(Debug, Clone)]
pub struct Source {
    pub kind: SourceKind,

    // Legacy compatibility.
    pub path: Option<String>,
    pub url: Option<String>,
    pub excerpt: Option<String>,

    // Registered-source reference.
    pub document: Option<SourceId>,
    pub range: Option<SourceRange>,
}

#[derive(Debug, Clone)]
pub struct SourceRange {
    pub start_byte: u64,
    pub end_byte: u64,
    pub start_line: u32,
    pub end_line: u32,
    pub excerpt_hash: ContentHash,
}
```

Example record provenance:

```akr
source {
    kind external
    document "jp2lam-decoder-performance-audit-2026-08-05"

    start_byte 10482
    end_byte 12391

    start_line 203
    end_line 233

    excerpt_hash "sha256:..."
}
```

The byte range is the exact machine locator. The line range is for people and rendered citations.

The excerpt itself no longer needs to be duplicated into every record. It can be rendered from the immutable source bytes.

## Why not require a disposition for every sentence?

Most outside reports contain:

* Explanatory material.
* Benchmarks.
* Examples.
* Alternative implementations.
* Caveats.
* Supporting observations.
* Restatements of earlier conclusions.
* Advice that remains relevant but does not require project action.

Requiring a status for every unit produces administrative work without necessarily improving planning.

The default should therefore be **sparse adoption**.

An optional exhaustive mode can be added later:

```text
akr source review <source-id> --exhaustive
```

That mode could create a review manifest referencing source ranges, but it should:

* Never duplicate the full source text.
* Never create records automatically.
* Never replace or delete the original document.
* Exist only for audits where every recommendation genuinely must receive a disposition.

The one-character review system from the earlier proposal belongs there, not in ordinary source intake.

# 4. Index semantic chunks, not lines

Line numbers are useful coordinates now that the source is immutable. They are still a poor search unit.

A physical line may be:

* Half a wrapped sentence.
* One row of a table.
* One line of a code block.
* A list marker with little independent meaning.
* An empty formatting line.
* A continuation of the preceding paragraph.

The design should distinguish **identity**, **retrieval unit**, and **display location**.

| Purpose              | Representation                       |
| -------------------- | ------------------------------------ |
| Document identity    | Source ID plus SHA-256 content hash  |
| Exact source locator | Byte range plus excerpt hash         |
| Human citation       | Line range and heading path          |
| Search unit          | Semantic chunk                       |
| Search ranking       | BM25 over heading, prose and symbols |

## Chunking policy

Use a deterministic lightweight Markdown block scanner:

1. Headings establish a section path.
2. Paragraphs and list groups are semantic blocks.
3. Fenced code remains intact.
4. Tables remain intact unless exceptionally large.
5. Consecutive blocks under the same heading are packed into approximately 250–700 estimated tokens.
6. No chunk crosses a major heading boundary.
7. No code block is split.
8. No overlap is needed; `source get` can return neighboring chunks.
9. The parser version is stored in the index.
10. Chunking errors can only harm search quality, not project semantics.

A report like the supplied audit should produce several dozen chunks, not 925 line entries and not 438 candidate records.

The initial semantic scanner idea therefore survives in a safer place:

```rust
pub struct SourceChunk {
    pub document: SourceId,
    pub ordinal: u32,
    pub heading_path: Vec<String>,
    pub kind: ChunkKind,

    pub start_byte: u64,
    pub end_byte: u64,
    pub start_line: u32,
    pub end_line: u32,

    pub content_hash: ContentHash,
    pub search_text: String,
    pub symbols: Vec<String>,
}
```

A derived chunk ID can be:

```text
sha256(
    document_content_hash
    || parser_version
    || start_byte
    || end_byte
)
```

Do not store that chunk ID as the permanent citation in records. Index chunk boundaries can change after a parser improvement. Permanent record citations should retain the document ID and exact byte range.

## Index schema

AKR already uses SQLite FTS5 and BM25 for live record search. Extend the existing cache rather than introducing another database:

```sql
CREATE TABLE source_documents (
    id              TEXT PRIMARY KEY,
    title           TEXT NOT NULL,
    path            TEXT NOT NULL UNIQUE,
    content_hash    TEXT NOT NULL,
    origin          TEXT NOT NULL,
    media_type      TEXT NOT NULL,
    byte_len        INTEGER NOT NULL,
    supersedes      TEXT
);

CREATE TABLE source_chunks (
    rowid           INTEGER PRIMARY KEY,
    document_id     TEXT NOT NULL,
    parser_version  INTEGER NOT NULL,
    ordinal         INTEGER NOT NULL,
    heading_path    TEXT NOT NULL,
    kind            TEXT NOT NULL,

    start_byte      INTEGER NOT NULL,
    end_byte        INTEGER NOT NULL,
    start_line      INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,

    raw_text        TEXT NOT NULL,
    search_text     TEXT NOT NULL,
    symbols         TEXT NOT NULL,
    content_hash    TEXT NOT NULL,

    UNIQUE(document_id, parser_version, ordinal)
);

CREATE VIRTUAL TABLE source_chunks_fts USING fts5(
    heading_path,
    search_text,
    symbols,
    content='source_chunks',
    content_rowid='rowid',
    tokenize='unicode61'
);
```

Suggested relative BM25 weights:

```text
heading path   high
symbols        high
normal prose   normal
code/support   lower unless matched as a symbol
```

## Normalize technical symbols separately

Normal word tokenization is poor for queries such as:

```text
DecodeRequest::default()
inverse_97_2d_in_place_at
9/7
src/decode/reconstruct.rs
```

Store a generated symbol field with variants such as:

```text
DecodeRequest::default()
DecodeRequest
default
decode request default

inverse_97_2d_in_place_at
inverse 97 2d in place at

9/7
9 7
irreversible 9 7
```

The exact raw source remains unchanged. Only `search_text` and `symbols` are normalized.

## Search modes

Provide two modes:

```text
akr source search "nonzero tile origins"
akr source search --literal "DecodeRequest::default()"
```

The default mode should treat the query as ordinary text and safely escape FTS5 punctuation. The current record search accepts raw FTS5 syntax, which is awkward for agents and code-related queries. Expert raw syntax can remain available as:

```text
akr source search --fts '<raw FTS5 expression>'
```

Literal search can:

1. Use FTS/symbol search to narrow candidate chunks where possible.
2. Verify the exact substring against `raw_text`.
3. Fall back to a direct byte scan for punctuation-heavy expressions.

For a source corpus of ordinary project size, direct scanning for an exact literal will be fast and far simpler than maintaining a dedicated substring index.

## Source retrieval

Provide:

```text
akr source get <source-id> --whole
akr source get <source-id> --lines 203:233
akr source get <source-id> --section "5.1 Scratch allocation per parallel row"
akr source get <source-id> --chunk <chunk-id> --neighbors 1
```

MCP equivalents:

```text
knowledge.source_search
knowledge.source_get
```

A search result should look like:

```text
4.82  source:jp2lam-decoder-performance-audit-2026-08-05
      external · non-authoritative
      6. P1: nonzero tile origins unnecessarily disable optimized DWT
      lines 283–316
      linked records:
        @lege-ecosystem-perf.work.aligned-nonzero-origin-dwt/1
```

The word **non-authoritative** should not be omitted.

# 5. Use FTS5 first; do not integrate SinoRAG yet

SinoRAG’s indexing work is relevant, but it is solving a larger and somewhat different problem.

Its current TF-IDF index is designed around CJK character n-grams, 8-bit log-quantized weights, mmap-backed document rows, and mmap-backed posting lists. Its default design allows a large feature vocabulary and stores both row and posting representations for similarity and raw-query scoring.

Its current phrase index is also CJK-oriented and uses fixed character grams, a memory-mapped gram table, and hybrid postings: delta-varint for sparse grams and Roaring bitmaps for dense ones.

Those are good large-corpus designs. They are not the first thing AKR needs.

| Method                                            | Value to AKR                                                     | Decision             |
| ------------------------------------------------- | ---------------------------------------------------------------- | -------------------- |
| Existing SQLite FTS5/BM25                         | Ranked title, heading, prose and symbol search                   | Use now              |
| Direct literal byte scan                          | Exact code symbols and punctuation-heavy phrases                 | Use now              |
| FTS5 trigram tokenizer                            | Possible future substring acceleration                           | Measure later        |
| SinoRAG PhraseIndex                               | Excellent candidate generation for large exact-substring corpora | Defer                |
| SinoRAG TF-IDF                                    | Useful alternative lexical ranker and similarity engine          | Defer                |
| Embeddings                                        | Useful for paraphrased semantic discovery                        | Much later, optional |
| SinoRAG document fingerprints and mmap discipline | Directly useful design ideas                                     | Borrow now           |

## Why adding both SinoRAG indexes now would be counterproductive

It would create at least three lexical representations:

1. AKR record FTS.
2. Source TF-IDF rows and postings.
3. Source phrase-gram postings.

That means:

* More invalidation logic.
* More index formats.
* More build commands.
* More schema compatibility.
* More ranking reconciliation.
* More failure modes.
* No improvement to `knowledge.get`, `knowledge.context`, Git freshness, or ledger parsing.

The current command slowness is likely not caused by BM25 lookup. Static inspection shows repeated workspace and Git work before the search is reached.

## Small index-size sanity check

I ran a rough local prototype against the uploaded audit:

* Source size: 46,738 bytes.
* Heading-based semantic chunks: 57.
* SQLite table plus external-content FTS5 index after `VACUUM`: 147,456 bytes.

That is about three times the source bytes, but only 144 KiB in absolute terms, with SQLite page overhead and repeated heading terms dominating at this tiny scale. It is not a benchmark of final AKR behavior, but it demonstrates that **index disk size is not the immediate risk**.

The immediate risks are duplicated project semantics and repeated command initialization.

## When to reconsider SinoRAG

Keep a narrow backend interface:

```rust
pub trait SourceSearchBackend {
    fn search(
        &self,
        query: &SourceQuery,
    ) -> Result<Vec<SourceHit>, SourceSearchError>;
}
```

Implement only:

```text
SqliteFts5Backend
```

initially.

Evaluate a genericized SinoRAG backend only if measured results show one of these:

* The source corpus grows into tens or hundreds of thousands of chunks.
* Warm FTS queries become materially slow.
* Known-answer retrieval tests show inadequate recall.
* Exact literal scanning becomes a measurable bottleneck.
* Document-similarity discovery becomes a real workflow requirement.

At that point, extract a small generic crate from SinoRAG:

```text
sinorag-lexical
```

with a configurable analyzer. Do not make AKR depend on SinoRAG’s full Parquet/DataFusion/CJK ingestion stack.

# 6. Fix AKR command latency before adding retrieval machinery

Static inspection of the supplied archive identifies a likely primary cause.

`crates/akr-cli/src/session.rs:146–228` shows that `Session::open`:

1. Locates and reads the workspace.
2. Parses and lowers all AKR source files.
3. Opens the Git repository.
4. Computes record last-change information.
5. Collects commits from records.
6. Computes ancestry information.

Then `crates/akr-mcp/src/tools.rs:456–479` opens a fresh `Session` for every MCP read.

Even `akr search`, whose actual query is a small SQLite BM25 operation, pays the common session-opening work first.

A faster index will not fix that.

## Phase the work by command requirements

Introduce explicit load levels:

```rust
#[derive(Debug, Clone, Copy)]
pub enum ReadNeeds {
    IndexOnly,
    Ledger,
    LedgerWithFreshness,
    Write,
}
```

Suggested mapping:

| Command             | Requirement                       |
| ------------------- | --------------------------------- |
| `search`            | Index only                        |
| `source search`     | Index only                        |
| `source get`        | Source catalog and source bytes   |
| `get`               | Ledger                            |
| `context`           | Ledger plus freshness             |
| `review-queue`      | Ledger plus freshness             |
| `impact --git-diff` | Ledger plus Git                   |
| Writes              | Ledger, validation and write lock |

Do not calculate Git ancestry for `search`, `source search`, or ordinary `get`.

## Persistent MCP runtime

The MCP server is long-lived. It should keep a workspace snapshot rather than rebuilding one per tool call.

A suitable shape is:

```rust
pub struct WorkspaceRuntime {
    root: PathBuf,
    snapshot: Option<Snapshot>,
}

pub struct Snapshot {
    stamp: WorkspaceStamp,

    ledger: Arc<Ledger>,
    resolved: Arc<ResolvedData>,

    source_catalog: Arc<SourceCatalog>,
    git_facts: OnceLock<Arc<GitFacts>>,
    freshness: OnceLock<Arc<ReviewQueue>>,
}
```

The current `ResolvedModel<'a>` borrows the ledger. To store a reusable snapshot cleanly, split it into:

```rust
pub struct ResolvedData {
    pub heads: BTreeMap<LogicalKey, RevisionId>,
    pub head_errors: BTreeMap<LogicalKey, HeadError>,
    pub edges: Vec<ResolvedEdge>,
    pub resolutions: Vec<ResolutionEntry>,
    pub supersession: BTreeMap<LogicalKey, Vec<RevisionId>>,
    pub acceptance: Vec<CheckVerdict>,
    pub diagnostics: Vec<Diagnostic>,
    // ...
}

pub struct ResolvedModel<'a> {
    pub ledger: &'a Ledger,
    pub data: &'a ResolvedData,
}
```

This avoids a self-referential cached `Session`.

The runtime should:

1. Load once at server startup.
2. Reuse the snapshot across reads.
3. Invalidate after a successful AKR write.
4. Recompute Git facts only when `HEAD` or relevant ledger inputs change.
5. Detect outside `.akr` edits from file metadata and rehash only changed files.
6. Re-index source documents incrementally.

## Separate cache generations

The current index uses one source-graph hash and fully rebuilds when it differs.

Introduce independent generations:

```text
ledger_graph_hash
source_catalog_hash
source_corpus_hash
git_head
schema_version
source_parser_version
```

Then:

```rust
if schema_changed {
    rebuild_everything();
} else {
    if ledger_hash_changed {
        rebuild_record_tables();
    }

    if source_corpus_hash_changed {
        sync_source_tables();
    }
}
```

Source documents are immutable and append-only, making incremental source indexing straightforward:

* Existing content hashes require no work.
* New hashes are parsed and inserted.
* Superseded documents remain stored but are filtered by default.
* No existing chunk has to be rewritten.

# 7. Fix the current MCP correctness and token problems

These fixes from the previous analysis still stand.

## Return the human-readable context

`commands::Output` already contains:

```rust
pub text: String,
pub result: Value,
```

But MCP `run_read` returns only `output.result`.

That means the MCP consumer does not receive the full readable context produced by `render_text`.

Return both:

```rust
pub struct ToolReadResult {
    pub text: String,
    pub structured: Value,
}
```

The MCP result should contain:

```json
{
  "content": [
    {
      "type": "text",
      "text": "<actual human-readable bundle>"
    }
  ],
  "structuredContent": {
    "...": "machine-readable metadata"
  }
}
```

Do not pretty-print the structured JSON into the text field. That merely duplicates tokens.

## Make budgeting actually remove prose

The current budget implementation marks records as truncated and subtracts a fixed estimate, but the renderer writes the body before adding the “prose truncated” marker.

The first reliable patch is whole-body omission:

```rust
if bundle.truncated.contains(&record.id) {
    out.push_str("  (prose omitted to fit the budget)\n");
} else if let Some(body) = body_of(record) {
    out.push_str(&indent(body, 2));
}
```

Then calculate the token estimate from the text that will actually be rendered.

Partial deterministic prefixes can be added later. Whole-body omission is more honest than falsely claiming truncation.

## Add natural task orientation

The current agent protocol assumes that the agent already knows an exact planning key.

Add:

```text
knowledge.start
```

Input:

```json
{
  "task": "continue the jp2lam decoder optimization work",
  "paths": ["lege-codecs/jp2lam/**"],
  "budget_tokens": 3000
}
```

Output:

```json
{
  "planning_candidates": [
    {
      "ref": "@lege-ecosystem.work.jp2lam-jpeg-2000-decoder-optimization-plan/1",
      "title": "jp2lam JPEG 2000 Decoder Optimization Plan",
      "state": "active",
      "path_overlap": true
    }
  ],
  "external_sources": [
    {
      "id": "jp2lam-decoder-performance-audit-2026-08-05",
      "title": "jp2lam JPEG 2000 decoder performance audit",
      "standing": "non_authoritative",
      "matched_sections": [
        "17. Implementation roadmap"
      ]
    }
  ],
  "recommended_context": {
    "goal": "lege-ecosystem.work.jp2lam-jpeg-2000-decoder-optimization-plan",
    "paths": ["lege-codecs/jp2lam/**"],
    "budget_tokens": 3000
  }
}
```

This tool does not grant authority. It only orients the agent.

Also improve `GoalUnresolved` so it returns the same planning candidates rather than only saying that the goal did not resolve.

The revised protocol becomes:

```text
When an exact goal key is known:
    knowledge.context

When it is not:
    knowledge.start
    then knowledge.context using the selected exact key
```

## Add detail levels

For token control:

```text
knowledge.get detail=summary|body|canonical
knowledge.source_get detail=snippet|section|whole
```

Recommended defaults:

```text
knowledge.get        body
knowledge.source_get section
```

`canonical` should be explicit because raw AKR syntax is rarely what an agent needs.

# 8. Keep source retrieval separate from authority

Do not automatically insert top-ranking source chunks into every context bundle.

`knowledge.context` should include:

```text
SOURCE REFERENCES
  jp2lam decoder audit
    § 5.1 Scratch allocation per parallel row
    lines 203–233
    cited by @...work.row-scratch/1
```

It should not include unrelated source-search hits.

`knowledge.start` may show relevant external sources, but under a separately labelled section:

```text
EXTERNAL REFERENCE MATERIAL — NON-AUTHORITATIVE
```

A source result enters the authoritative plan only through an explicit AKR record and normal lifecycle rules.

This preserves AKR’s existing rule that search ranks but never authorizes.

# 9. Migration plan for the current jp2lam state

The current bulk-imported audit should be converted to the new model.

## Step 1: restore and register the report

Put the exact original report under:

```text
sources/external/
```

Register its hash and source metadata.

Do not edit it to add checkboxes, statuses, or comments.

## Step 2: remove the bulk import from active planning surfaces

The many automatically imported proposed records have not been adopted. They should not remain as ordinary proposed work clutter.

Because they are unaccepted import drafts, use the existing migration rules to either:

* Delete them in one explicit migration commit; or
* Mark them withdrawn if preserving the failed import experiment is valuable.

Record one decision explaining why:

```text
The heading-oriented import was replaced by immutable-source registration
because it fragmented the report, obscured source context, and produced
non-authoritative work entries in normal planning surfaces.
```

## Step 3: create one source-review work record

Use several meaningful acceptance checks, not one per line:

```text
Review the 2026-08-05 jp2lam decoder audit

[ ] Verify the renderer request/session/fallback findings
[ ] Decide the initial optimization sequence
[ ] Create work records for adopted Phase 1 and Phase 2 changes
[ ] Record adopted constraints and explicit deferrals
[ ] Attach source references to every resulting record
```

## Step 4: create the plan of record

A likely decision based on the report would be:

```text
Adopt:
  renderer-path telemetry
  persistent decoder integration
  coarse 9/7 row scratch
  aligned-origin optimized dispatch
  9/7 banded reconstruction
  fused packed output

Defer:
  Tier-1 redesign
  true windowed reconstruction
  architecture-specific SIMD
  GPU backend

Do not begin with:
  MQ instruction-level tuning
  additional fine-grained Rayon nesting
  allocator substitution for structural memory fixes
```

The exact project decision remains yours; the report does not become authoritative merely because it recommended this sequence.

## Step 5: link each adopted record to exact source ranges

For example:

```text
work: coarse 9/7 row scratch
source: audit lines 203–233

work: aligned-origin dispatch
source: audit lines 283–316

decision: initial implementation sequence
source: audit lines 708–777

policy/decision: optimization exclusions
source: audit lines 889–897
```

## Step 6: rerun the agent golden scenario

The agent should now do:

```text
knowledge.start
knowledge.context
knowledge.source_get
source-code search
```

It should not do:

```text
ls .akr
sed .akr/records/...
grep docs/generated/ACTIVE-WORK.md
find missing legacy Markdown
```

# 10. Phased implementation roadmap

## Phase 0 — record the architecture change and establish measurements

Deliverables:

* Add a decision distinguishing legacy migration from external-source intake.
* Mark the earlier line-by-line outside-ingest proposal superseded.
* Add phase timing instrumentation using `Instant`.
* Add the jp2lam agent transcript as a golden workflow test.
* Record cold and warm timings separately.

Instrument:

```text
locate_workspace
read_sources
parse_lower
git_open
git_last_changes
git_ancestry
resolve_validate
freshness
index_open
index_query
render
total
```

Exit conditions:

* The slow phases are measured rather than inferred.
* The agent transcript has machine-checkable expectations.
* No source-library implementation starts before the baseline exists.

## Phase 1 — fix current MCP navigation and latency

Deliverables:

* Persistent MCP workspace snapshot.
* Lazy Git and freshness computation.
* Index-only path for search.
* Actual MCP text output.
* Real prose omission under context budgets.
* `GoalUnresolved` candidate suggestions.
* `knowledge.start`.
* Search scores in structured results.
* Default query escaping rather than raw FTS5 syntax.

Initial performance targets on a representative project fixture:

| Operation                        | Warm p95 target |
| -------------------------------- | --------------: |
| Record search                    |         ≤ 50 ms |
| Record get                       |         ≤ 50 ms |
| Source search                    |         ≤ 50 ms |
| Source section get               |         ≤ 50 ms |
| Context without snapshot refresh |        ≤ 200 ms |

The exact numbers can be adjusted after Phase 0, but the tests should enforce a meaningful warm-session budget.

Exit conditions:

* The jp2lam goal is found within two knowledge calls.
* No Git ancestry work occurs during record or source search.
* No raw ledger or generated-view read is needed.
* The rendered context genuinely fits its declared budget.

## Phase 2 — immutable source catalog

Deliverables:

* `sources/catalog.akr`.
* `SourceDocument`, `SourceId`, and source-version model.
* `akr source add`.
* `akr source verify`.
* `akr source list`.
* `akr source supersede`.
* Hash verification in `akr check`.
* Updated `AGENTS.md`.
* New `docs/15-external-sources.md`.
* Rename or document `akr import` as legacy migration only.

Exit conditions:

* Registered bytes round-trip exactly.
* A direct edit produces `AKR-S021`.
* Superseding a source creates a new object and preserves the old one.
* Registering a source creates no project records.
* Source catalog construction is deterministic.

## Phase 3 — source chunking and FTS5 retrieval

Deliverables:

* Deterministic Markdown block scanner.
* Heading-path and byte/line-range calculation.
* Chunk packing by estimated token size.
* Symbol normalization.
* `source_documents`, `source_chunks`, and `source_chunks_fts`.
* Independent `source_corpus_hash`.
* Incremental insertion for new immutable documents.
* `akr source search`.
* `akr source get`.
* `knowledge.source_search`.
* `knowledge.source_get`.

Golden queries for the uploaded audit:

```text
nonzero tile origins
for_each_init worker scratch
DecodeRequest::default()
banded 9/7
what should not be done first
persistent Jp2Decoder session
```

Exit conditions:

* Every query returns the expected section in the top three.
* Exact byte slices reproduce the registered source.
* Index rebuilds are byte-identical.
* A source addition does not rebuild record tables.
* A record write does not rechunk the source corpus.

## Phase 4 — sparse AKR source overlays

Deliverables:

* Extend record `source` blocks with document IDs and ranges.
* Render source references in `get` and `context`.
* Return related AKR records from source retrieval.
* Clearly distinguish unreviewed source, adopted work and accepted policy.
* Add source-reference validation:

  * document exists;
  * digest matches;
  * range is in bounds;
  * excerpt hash matches;
  * lines correspond to the byte range.

Exit conditions:

* An AKR record can cite exact external source material without copying it.
* Moving a source path does not change document identity.
* Changing index chunking does not break record citations.
* Search results cannot affect record state or context membership.
* External source text is always labelled non-authoritative.

## Phase 5 — migrate the jp2lam audit and clean generated views

Deliverables:

* Register the original report.
* Remove or withdraw the bulk proposed import drafts.
* Create the compact review work record.
* Create only adopted plan, work, decision and policy records.
* Add precise source references.
* Regenerate active-work and roadmap views.
* Add the agent workflow regression test.

Exit conditions:

* The report remains fully readable.
* The active plan is compact.
* An agent can tell which recommendations are accepted.
* An agent can retrieve implementation detail without raw ledger inspection.
* `ACTIVE-WORK.md` contains adopted work, not hundreds of source fragments.

## Phase 6 — evaluate advanced lexical indexing

Do not implement this phase merely because SinoRAG exists.

Deliverables:

* Known-answer retrieval benchmark.
* FTS5 latency and index-size measurements.
* Literal-scan measurements.
* Optional `SourceSearchBackend` interface.
* Decision on whether a generic SinoRAG lexical crate is justified.

Possible outcomes:

```text
FTS5 remains sufficient
    → stop

FTS5 ranking recall is insufficient
    → trial TF-IDF alternative ranker

Exact substring search is too slow at scale
    → trial PhraseIndex or FTS5 trigram

Both are needed
    → extract a generic mmap-backed lexical crate from SinoRAG
```

# Final recommendation

Implement the following architecture:

> **Immutable Markdown source library + deterministic semantic search index + sparse authoritative AKR overlay.**

Do not implement default line-by-line or paragraph-by-paragraph record ingestion for outside reports.

Do not integrate SinoRAG’s TF-IDF or phrase index yet. AKR already has the correct first-stage engine in SQLite FTS5. Extend it to source chunks, add literal verification for technical strings, and fix repeated workspace/Git initialization.

The source folder idea should therefore stand, but not as “a folder with a warning.” It should be an append-only, content-addressed, validated source library with stable retrieval and exact AKR citations. That preserves the first-read strength of Markdown while allowing AKR to provide the compact, coherent project state that agents need on every later session.

# Verdict

A tighter coupling would help, but the correct model is not simply “AKR replaces Git as the lead system.”

Use:

> **AKR leads the intent and verification; Git seals the exact snapshot.**

That creates a three-step protocol:

1. **AKR prepares the change**: work, rationale, state transitions, evidence, acceptance checks.
2. **Git records the change**: exact files, diff, parentage, author, commit identity.
3. **AKR indexes the Git result**: which commits implement which work records and whether those commits are reachable from the main branch.

The agent exchange demonstrates the need. The implementation and its validation were already described in enough detail to create a strong commit message, including the two gates, affected code paths, test result, and image metrics.  But the code and ledger had diverged: the implementation existed while the corresponding work remained proposed and had no evidence or completion records. 

The later AGENTS.md changes improve discipline, but they still rely on the agent remembering to coordinate two separate systems manually.  Tooling should make the synchronized path easier than the unsynchronized one.

# Do you need another ledger type?

**Another canonical AKR record kind: no.**

**Another bridge object: yes.**

Do not add a durable `commit` record for every Git commit. That would duplicate Git history and introduce several problems:

* One work record can require many commits.
* One commit can advance several related work records.
* Rebasing and cherry-picking change commit IDs.
* Squashing changes commit boundaries.
* The commit hash cannot be written into a file contained in the same commit without an amendment loop.
* Hundreds of commit records would drown out decisions, work, evidence, and other actual project knowledge.

Instead, add a lightweight **change transaction**—or `ChangeIntent`—that exists only while preparing a Git commit.

It is an AKR-managed object, but not part of the canonical knowledge graph. Store it in the worktree’s Git metadata:

```text
$(git rev-parse --git-path akr/current-change.akr)
```

That makes it:

* Local to the current worktree.
* Naturally associated with Git.
* Safe to discard and reconstruct.
* Absent from normal AKR search and context.
* Free from permanent ledger bloat.

Its durable projection is the resulting Git commit message and its AKR trailers.

# Authority boundary

| Question                                     | Authority                                  |
| -------------------------------------------- | ------------------------------------------ |
| What should be done?                         | AKR work/decision records                  |
| Why should it be done?                       | AKR rationale, sources and decisions       |
| What proves it worked?                       | AKR evidence and acceptance checks         |
| What exact bytes changed?                    | Git tree and diff                          |
| What change was committed together?          | Git commit                                 |
| Which work did that commit advance?          | AKR-generated Git trailers                 |
| Has the change landed on the default branch? | Git reachability                           |
| Is a work item complete?                     | AKR, based on evidence—not merely a commit |
| Which commits relate to a work record?       | Derived AKR Git index                      |

This avoids making either system impersonate the other.

# The change transaction

A transaction needs only the information that cannot be safely inferred from a work record or a Git diff:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeIntent {
    pub id: ChangeId,

    /// HEAD when this transaction began.
    pub base_commit: GitOid,

    /// The main work record this commit advances.
    pub primary_work: Option<RecordRef>,

    /// Other records materially advanced by the same logical change.
    pub related_work: Vec<RecordRef>,

    /// fix, feat, perf, refactor, test, docs, build, chore...
    pub change_kind: ChangeKind,

    /// Human-readable commit scope, such as "tone" or "jp2lam".
    pub scope: Option<String>,

    /// Imperative description of what this particular commit does.
    pub summary: String,

    /// Optional explanation that is specific to this commit.
    pub implementation_note: Option<String>,

    /// Explicit exemption for a material change with no AKR work record.
    pub untracked_reason: Option<String>,
}
```

Example:

```akr
change 0.1

id "chg-01K25R5T4G8F7B2N6JQH"

base_commit "ff74d3b2"

kind fix
scope "tone"
summary "gate reconstructed highlight chroma by uncertainty"

primary_work @raw-autotune.work.slice-6-uncertainty-gated-chroma-limiting-phase/2

related_work {
    @raw-autotune.work.slice-1-diagnostic-dumps-and-clip-state-map/2
    @raw-autotune.work.slice-4-raw-domain-confidence-plumbing-phase-b-4/2
    @raw-autotune.work.slice-8-local-tone-sky-ground-hdr-evaluation/2
}
```

The `summary` field is important. A work title such as:

```text
Slice 6 uncertainty-gated chroma limiting phase
```

is a useful planning name but a poor commit subject. Commit boundaries are finer-grained than work records, so one short per-change summary is unavoidable. It is not redundant documentation: that exact text becomes the commit subject.

# Recommended workflow

## 1. Start from an AKR work record

```bash
akr change begin \
  --primary raw-autotune.work.slice-6-uncertainty-gated-chroma-limiting-phase \
  --kind fix \
  --scope tone \
  --summary "gate reconstructed highlight chroma by uncertainty"
```

This creates the local transaction and records the current `HEAD`.

Optionally:

```bash
akr work start raw-autotune.work.slice-6-uncertainty-gated-chroma-limiting-phase
```

could both revise `proposed → active` and open the transaction.

## 2. Let code and canonical AKR state co-evolve

The agent edits code and makes only meaningful AKR revisions:

```bash
akr revise ...
akr evidence add ...
akr complete ...
akr build
```

A work record should **not** receive a new revision after every implementation commit merely to prove that it remains active. Several commits may reference the same active work record without changing the record itself.

That is one reason the change transaction is needed.

A typical progression is:

```text
Commit 1:
  work proposed → active
  commit references work

Commit 2:
  no work-state change
  commit still references active work

Commit 3:
  evidence added
  work active → completed
  commit references work and evidence
```

The current rule that “if code is dirty, the ledger must be dirty in the same direction” is too broad. Dirty working-tree state is not the correct unit, and it would create meaningless work revisions during incremental implementation.

## 3. Stage the exact logical change

```bash
git add \
  src/tone.rs \
  src/highlight.rs \
  src/color.rs \
  src/pipeline.rs \
  .akr/records/raw-autotune/work.akr \
  .akr/records/raw-autotune/evidence.akr \
  .akr/akr.lock \
  docs/generated/
```

The **Git index**, not the whole working tree, must be the synchronization boundary.

The exchange ended with 18 changed and six untracked files.  A commit tool must not assume every dirty file belongs to the same logical change.

## 4. Prepare from the staged tree

```bash
akr change prepare --staged
```

This command should:

1. Read the base ledger from `HEAD`.
2. Read the proposed ledger from the Git index.
3. Compute a semantic AKR delta.
4. Read the staged non-AKR file changes.
5. Validate the change transaction.
6. Run `akr check` and `akr build --check` against the staged snapshot.
7. Generate the commit message.
8. Record the staged Git tree OID.
9. Refuse preparation if the staged tree changes afterward.

The semantic delta would look like:

```text
Work transitions
  slice-1  proposed → completed
  slice-4  proposed → completed
  slice-6  proposed → completed
  slice-8  proposed → active

Evidence added
  slice-1-diagnostic-dumps-verify
  slice-4-confidence-plumbing-verify
  slice-6-uncertainty-gated-chroma-verify

Code
  src/tone.rs
  src/highlight.rs
  src/color.rs
  src/pipeline.rs
```

Generated files and `akr.lock` should be validated but excluded from the descriptive file summary.

## 5. Commit through the bridge

```bash
akr git commit
```

This invokes Git with the prepared message. It does not implement its own object store or history.

An ordinary `git commit` can still work through hooks, but `akr git commit` should be the preferred agent path.

## 6. Reconcile after Git creates the commit

A post-commit operation records the association in a rebuildable local index:

```text
change ID → commit OID
work refs → commit OID
evidence refs → commit OID
ledger graph hash → commit OID
```

It must not edit canonical AKR records after the commit. That would immediately create another dirty ledger and require another commit.

The durable association is already embedded in the commit trailers, so the local index can always be rebuilt by scanning Git history.

# Commit message generation

The message should be generated from four sources:

1. **Subject**: transaction `summary`.
2. **Why**: primary work intent or rationale.
3. **What happened**: semantic AKR transitions plus optional implementation note.
4. **Verification**: compact evidence results.
5. **Links**: machine-readable Git trailers.

For the attached exchange, a suitable generated message would be:

```text
fix(tone): gate reconstructed highlight chroma by uncertainty

Restore the display-linear near-white proxy and combine it with the
reconstruction uncertainty map so partially clipped highlights are
neutralized without regressing one-channel clips.

Complete diagnostic-dump, confidence-plumbing, and uncertainty-gating
work. Keep local-tone HDR evaluation active.

Verified by 274 release tests and the _DSC1287 image oracle:
- crop strong-magenta rate: 15.0% -> 2.0%
- bright g-deficit >10: 20.4% -> 8.5%
- 2/3-clip median: +59 -> +9

AKR-Change: chg-01K25R5T4G8F7B2N6JQH
AKR-Work: @raw-autotune.work.slice-6-uncertainty-gated-chroma-limiting-phase/2
AKR-Work: @raw-autotune.work.slice-1-diagnostic-dumps-and-clip-state-map/2
AKR-Work: @raw-autotune.work.slice-4-raw-domain-confidence-plumbing-phase-b-4/2
AKR-Work: @raw-autotune.work.slice-8-local-tone-sky-ground-hdr-evaluation/2
AKR-Evidence: @raw-autotune.evidence.slice-6-uncertainty-gated-chroma-verify/1
AKR-Graph: sha256:5a0aa895...
AKR-Tree: 41cd7e...
```

The detailed implementation and verification results already existed in the agent’s output and later AKR updates.  The commit generator should condense that material, not ask the agent to write a second independent history.

Do not put the complete evidence record into the commit message. Git needs a concise historical explanation; AKR retains the full method, artifacts, commands, metrics and acceptance mapping.

# Use standard Git trailers

Use `git interpret-trailers`-compatible fields:

```text
AKR-Change:
AKR-Work:
AKR-Evidence:
AKR-Decision:
AKR-Graph:
AKR-Tree:
```

These are better than storing commit hashes inside AKR records because they:

* Survive ordinary rebases when messages are retained.
* Usually survive cherry-picks.
* Can be collected during squash preparation.
* Are searchable with `git log --grep`.
* Avoid the commit-hash circularity.
* Permit rebuilding all Git-to-AKR links.

Example query:

```bash
git log --all \
  --grep='AKR-Work: @raw-autotune.work.slice-6-' \
  --format='%H %s'
```

AKR should expose this as:

```bash
akr git log \
  raw-autotune.work.slice-6-uncertainty-gated-chroma-limiting-phase
```

# Fix evidence provenance as part of this work

The exchange says evidence was recorded as observed at `ff74d3b`, while the implementation still existed only in a dirty working tree.  If `observed_at` is supposed to identify the tested implementation, that provenance is misleading: `ff74d3b` did not yet contain the code being verified.

You cannot write the future commit ID into evidence contained in that same commit. Nor can you write the complete Git tree ID into a file contained in that tree without creating a hash cycle.

Use a digest over the **implementation portion** of the staged tree, excluding AKR metadata and generated AKR views:

```rust
pub struct ImplementationSnapshot {
    pub base_commit: GitOid,

    /// Hash of sorted path, file mode and staged Git blob OID.
    /// Excludes .akr/** and docs/generated/**.
    pub artifact_digest: Sha256Digest,

    pub included_paths: Vec<String>,
}
```

Conceptually:

```rust
fn implementation_digest(entries: &[IndexEntry]) -> Sha256Digest {
    let mut entries = entries
        .iter()
        .filter(|entry| !is_akr_metadata(&entry.path))
        .collect::<Vec<_>>();

    entries.sort_by_key(|entry| &entry.path);

    let mut hasher = Sha256::new();

    for entry in entries {
        hasher.update(entry.path.as_bytes());
        hasher.update([0]);
        hasher.update(entry.mode.to_le_bytes());
        hasher.update(entry.blob_oid.as_bytes());
        hasher.update([0]);
    }

    hasher.finalize().into()
}
```

Evidence can safely contain this digest because changing the evidence file does not change the implementation digest.

Then `akr change prepare` verifies:

```text
evidence implementation digest
    ==
current staged implementation digest
```

After the commit exists, its OID is connected to the evidence through the trailers and derived Git index.

For high-confidence verification, tests should run against either:

* A working tree with no unstaged changes in the tested scope; or
* A temporary checkout of the Git index.

The second option provides exact staged-snapshot verification:

```bash
tmp="$(mktemp -d)"
git checkout-index --all --prefix="$tmp/"
(cd "$tmp" && cargo test --release)
```

AKR can automate that for acceptance commands that request strict staged verification.

# Hooks and enforcement

Hooks should be guardrails, not the primary architecture.

## `pre-commit`

Run:

```text
akr change verify --staged
akr build --check against staged snapshot
akr check against staged snapshot
```

It should reject:

* A completed work transition without passing evidence.
* Evidence whose implementation digest does not match staged code.
* A staged work transition omitted from the change transaction.
* A material code commit with neither an AKR work reference nor an explicit exemption.
* Stale generated views or lock.
* A prepared transaction whose staged tree changed.

## `prepare-commit-msg`

If an AKR transaction is ready, write the generated message into Git’s commit-message file.

It should not update records or run expensive tests.

## `commit-msg`

Validate:

* Subject length and structure.
* Required AKR trailers.
* Trailer references exist in the staged ledger.
* `AKR-Tree` matches `git write-tree`.
* `AKR-Graph` matches the staged lock.
* Exactly one primary work item was selected when several work records changed.

## `post-commit`

Only:

* Record the commit OID in a local derived cache.
* Clear the current transaction.
* Report whether the commit is reachable from the default branch.

Do not modify the ledger.

## CI or pre-push

Verify the commit range:

```bash
akr git verify-range origin/main..HEAD
```

Checks should include:

* Every material commit has an AKR association or explicit exemption.
* Every trailer reference resolves in that commit’s tree or ancestry.
* Completed records have passing evidence.
* Ledger and generated outputs validate.
* No commit claims an evidence snapshot inconsistent with its implementation tree.
* Multiple unrelated primary work records were not combined without an explicit grouped transaction.

Hooks can be bypassed; CI is the final authority.

# Do not automatically complete work from Git

A passing commit or successful merge does not prove a work item is complete.

Git can show:

* Code exists.
* Tests ran in CI.
* A commit reached `main`.

It cannot decide:

* Whether all acceptance criteria were met.
* Whether output quality is acceptable.
* Whether a benchmark is meaningful.
* Whether a design constraint was satisfied.
* Whether the observed result matches project intent.

Therefore:

```text
commit created       ≠ work completed
commit landed        ≠ acceptance satisfied
tests passed         ≠ all evidence accepted
```

The transaction should consume AKR state transitions, not invent them.

# Handling many-to-many relationships

The bridge must explicitly support these cases.

## One work record, several commits

Each commit contains the same `AKR-Work` trailer. Only commits that change the work’s canonical state revise the record.

```text
abc123  introduce uncertainty propagation
def456  integrate tone-map gating
789abc  add verification and complete work
```

All three relate to the same work record.

## One commit, several related records

Choose one primary work item for the subject and list related records in the body and trailers.

This is what the attached raw-autotune change effectively did: slices 1, 4 and 6 became completed while slice 8 became active. 

## Several unrelated records

Fail preparation and require separate commits.

An explicit `--grouped` transaction may override this when an atomic cross-cutting change is genuinely necessary.

## No work record

Allow it with an explicit category and reason:

```bash
akr change begin \
  --kind chore \
  --scope ci \
  --summary "pin the Windows build image" \
  --untracked-reason "repository maintenance; no project behavior or plan changed"
```

This avoids creating fake work records for formatting, dependency metadata, or mechanical maintenance.

# Squashing, rebasing and merging

Commit IDs are intentionally not canonical AKR fields.

## Rebase

The commit OID changes, but trailers remain. Rebuild the derived Git index.

## Cherry-pick

The new commit remains associated with the same AKR records through its trailers.

## Squash merge

Generate the final message from the union of branch trailers and the final AKR semantic delta:

```bash
akr git prepare-squash origin/main..HEAD
```

It should:

* Select the final primary work item.
* Deduplicate work and evidence references.
* Use final record revisions.
* Summarize the net state transition, not every intermediate commit.
* Generate a new `AKR-Change` ID for the squash result or retain the branch’s primary transaction ID according to policy.

## Merge commit

The merge commit may contain a compact summary and the main AKR work references. Parent commits retain detailed messages.

# Derived Git index

Add a rebuildable cache such as:

```text
.akr/cache/git-links.sqlite
```

Suggested tables:

```sql
CREATE TABLE git_commits (
    oid             TEXT PRIMARY KEY,
    tree_oid        TEXT NOT NULL,
    parent_oids     TEXT NOT NULL,
    subject         TEXT NOT NULL,
    authored_at     INTEGER NOT NULL
);

CREATE TABLE git_akr_links (
    commit_oid      TEXT NOT NULL,
    link_kind       TEXT NOT NULL,
    target_ref      TEXT NOT NULL,
    PRIMARY KEY (commit_oid, link_kind, target_ref)
);

CREATE INDEX git_akr_links_target
    ON git_akr_links(target_ref);
```

This supports:

```bash
akr get <work> --git
akr git log <work>
akr git unlinked
akr git reconcile
akr git landed <work>
```

Example output:

```text
raw-autotune.work.slice-6.../2
  state       completed
  commits
    41cd7e9  fix(tone): gate reconstructed highlight chroma by uncertainty
             current branch: yes
             default branch: no
  evidence
    raw-autotune.evidence.slice-6.../1
```

Git reachability—not a stored Boolean—determines whether the change has landed.

Git notes may cache structured metadata after commit, but they should not be required. Notes are not fetched and pushed by default and are too easy to lose. Commit trailers remain the durable bridge.

# Revised AGENTS.md rule

Replace the overly broad “dirty in the same direction” rule with something closer to:

```md
## AKR ↔ Git change protocol

AKR governs project intent, state, acceptance and evidence. Git governs exact
snapshots and history.

Before a material commit:

1. Associate the change with an AKR work record through `akr change begin`, or
   provide an explicit untracked-change reason.
2. Update canonical AKR records only when intent, scope, state, acceptance or
   evidence actually changes. Active work may span several commits without a
   new work revision.
3. Stage the exact logical code, AKR records and generated AKR outputs.
4. Run `akr change prepare --staged`.
5. Commit through `akr git commit`, or preserve the generated AKR trailers.

The staged Git tree—not the entire dirty working tree—is the synchronization
boundary.

Never mark work completed merely because code exists or a commit was made.
Completion requires the record's acceptance checks and supporting evidence.

Never write a future commit ID into the same ledger revision. Git-to-AKR
associations are carried by commit trailers and a rebuildable derived index.

Before handoff, report:
- `git status --short`
- `akr change status`
- `akr check`
```

# Implementation plan

## Phase 1 — semantic staged-state comparison

Build:

```text
akr diff --staged
```

It should compare:

```text
HEAD ledger
    versus
Git-index ledger
```

and produce typed changes:

* Records added.
* Revisions added.
* State transitions.
* Acceptance checks satisfied.
* Evidence added.
* Relations changed.
* Source graph hash changed.

Do not parse `git diff` text to infer AKR semantics.

Acceptance:

* Generated files do not appear as primary semantic changes.
* A reordered but semantically identical record produces no false transition.
* The command works with unrelated unstaged changes present.

## Phase 2 — change transaction

Implement:

```text
akr change begin
akr change show
akr change add-work
akr change set-summary
akr change prepare --staged
akr change verify --staged
akr change abort
```

Acceptance:

* Transaction is per worktree.
* A changed index invalidates a prepared transaction.
* Multiple work changes require an explicit primary.
* Active work can be referenced without receiving a new revision.
* Transaction files never appear in canonical AKR search.

## Phase 3 — deterministic commit generation

Implement:

```text
akr git message
akr git commit
```

Message generation uses:

```text
transaction summary
+ primary work intent
+ staged AKR semantic delta
+ compact evidence results
+ standard trailers
```

Acceptance:

* Same staged tree and transaction produce the same message except deliberately variable metadata.
* Subject remains under the configured limit.
* No raw record syntax or full evidence payload is dumped into the message.
* The message can be manually edited while trailers remain validated.

## Phase 4 — evidence snapshot binding

Add:

```text
implementation artifact digest
tested paths or scope
base commit
command and result
```

Acceptance:

* Evidence cannot falsely claim an unchanged `HEAD` when testing dirty code.
* Staged preparation detects stale evidence after source changes.
* AKR-only files do not affect the implementation digest.
* Strict mode can run verification against a temporary staged checkout.

## Phase 5 — hooks

Install through:

```bash
akr git install-hooks
```

Hooks should be thin wrappers around the AKR executable:

```sh
#!/bin/sh
exec akr git-hook pre-commit "$@"
```

Acceptance:

* Hooks work in normal and linked worktrees.
* Hooks never rewrite canonical records.
* A user can inspect the exact diagnostic and correction.
* CI catches changes committed with hooks bypassed.

## Phase 6 — Git-link index and agent context

Implement:

```text
akr git reconcile
akr git log <record>
akr git unlinked
akr get <record> --git
```

Add a compact Git section to `knowledge.context`:

```text
IMPLEMENTATION HISTORY

slice-6 uncertainty-gated chroma limiting
  completed
  3 related commits
  latest: 41cd7e9
  reachable from main: yes
```

Do not place full commit diffs into ordinary context.

## Phase 7 — range and squash support

Implement:

```text
akr git verify-range
akr git prepare-squash
```

Acceptance:

* Rebase does not break AKR links.
* Cherry-picks remain discoverable.
* Squash messages include the net work/evidence set.
* No canonical record stores unstable commit OIDs merely for lookup.

# Bottom line

The right architecture is:

> **AKR work record → local change transaction → staged Git tree → generated commit → derived Git linkage**

The additional abstraction should be a **change transaction, not another permanent ledger record type**.

That gives agents an AKR-first workflow without forcing AKR to duplicate Git history. It also solves the main weakness in the current AGENTS.md approach: active work can span several commits without ledger revision spam, while every commit still remains explicitly connected to its intent and evidence.

