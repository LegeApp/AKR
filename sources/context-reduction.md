# Do not abandon AKR based on this screen

The screen is a serious warning about the **current agent interface**, not evidence that the underlying ledger model is a failure.

AKR has already demonstrated the kind of value it was designed for: an agent finished and verified a substantial code change, but the project ledger still described all associated work as merely proposed. A later check caught the divergence, identified the missing state transitions and evidence, and forced the project state back into alignment.  That is a real cross-agent and cross-session benefit. A Markdown roadmap would not reliably detect that its own claims had become stale.

The problem is that AKR currently appears to be providing that benefit through a **context-expensive conversational interface**.

The correct conclusion is:

> Keep AKR as the project-state and verification system. Treat the current MCP usage pattern as a performance bug that needs a hard token budget.

There is also a viable fallback if the MCP layer cannot be made efficient: keep AKR’s ledger, Git coupling, validation, generated views, and CI checks, while replacing most MCP access with one small generated agent briefing. The core system does not depend on an always-on MCP interface.

# What the 45% figure does and does not say

The usage display explicitly calls these “independent characteristics” rather than a breakdown. Therefore:

* `100% subagent-heavy`
* `94% at >150k context`
* `45% MCP server akr`

are overlapping descriptions of the same usage.

They do **not** sum to 100%, and the screen does not establish that 45% of every consumed token was literal AKR output.

The exact attribution algorithm does not appear to be publicly documented, so I would not treat `45%` as an audited token measurement. It may partially mean that AKR was used in sessions responsible for 45% of usage, rather than that AKR text itself occupied 45% of all prompts.

Nevertheless, it is a strong signal because the screen states the specific mechanism:

> MCP tool results stay in context for the rest of the session.

Anthropic also states that current conversation length and tool usage affect usage limits. ([Claude Help Center][1])

The likely interaction is:

1. An AKR call returns several thousand tokens.
2. That output remains in the conversation.
3. The session continues past 150,000 tokens.
4. Every later model request operates over that larger history.
5. Subagents independently make additional requests and may retrieve overlapping AKR material again.

An illustrative example:

```text
4,000-token AKR result
× 20 later model requests
= 80,000 token-turns of context exposure
```

That is not an exact billing calculation because caching and usage-limit accounting complicate it. It shows why the cost of a tool result is not merely the size of the initial result.

The most alarming statistic is therefore arguably **94% at more than 150,000 context**, not 45% AKR by itself. AKR may be one reason those sessions grew and remained large, but the long-session/subagent combination is the multiplier.

Also, the top block showing zero tokens and zero cost appears to describe the newly opened local session, while the 86% bar describes account-level consumption in the current usage window. Those are different accounting scopes.

# Tool schemas are probably not the main problem anymore

Current Claude Code versions enable MCP Tool Search by default. With it enabled:

* Only tool names and server instructions are initially loaded.
* Tool definitions are deferred.
* Only tools actually used enter the context. ([Claude Platform Docs][2])

Therefore, provided that you are using a current Claude Code version with default first-party configuration, the existence of eleven or twenty AKR tools is probably not the dominant cost.

Verify these conditions:

```bash
claude --version
```

```text
ENABLE_TOOL_SEARCH is unset or true
AKR does not have alwaysLoad: true
CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS is not enabled
ANTHROPIC_BASE_URL is not forcing a configuration without Tool Search
```

A reduced tool surface is still useful because it guides agents toward a better workflow, but reducing tool count alone will not solve persistent large results.

The tool **results** are the likely problem. Claude Code currently warns only after an MCP output exceeds 10,000 tokens and permits up to 25,000 tokens by default. Those limits are appropriate for exceptional database or log retrieval, but much too generous for routine project-state operations. ([Claude Platform Docs][2])

An AKR result can be harmful long before it reaches the warning threshold.

# AKR should be a compiler, not a conversational database

Most AKR operations should happen deterministically without asking the model to reason over their output.

## Operations that should normally run outside model context

These belong in CLI commands, hooks, and CI:

```text
akr build
akr check
akr build --check
akr change prepare --staged
akr change verify --staged
akr git commit
akr git verify-range
source hash verification
generated-view freshness checking
Git-to-AKR trailer indexing
```

On success, these should produce either no output or one compact line:

```text
AKR OK ledger=5a0aa895 records=23 checks=18/18
```

Only failures need diagnostics.

The Git coupling you plan to implement is helpful here. It moves a significant amount of coordination from conversational reasoning into deterministic local tooling:

```text
AKR intent
    → staged semantic delta
    → generated commit message
    → hooks
    → Git trailers
    → derived Git index
```

That should reduce token use rather than increase it, provided that the hooks do not dump reports into the agent transcript.

## Operations that actually need MCP

MCP should be limited to questions requiring interpretation:

* What current work is relevant to this task?
* What constraints or decisions govern these files?
* What remains incomplete?
* Which source section supports this particular recommendation?
* What AKR changes should accompany the implementation?
* Apply these selected typed updates.

The agent does not need the entire ledger to answer those questions.

# Replace repeated retrieval with one bounded working set

The ideal session should not be:

```text
knowledge.search
knowledge.get
knowledge.context
knowledge.get another record
knowledge.impact
knowledge.validate
source search
source get
repeat after code changes
```

It should normally be:

```text
knowledge.start
    → one compact task packet

work on code

knowledge.update
    → one batched state/evidence update

deterministic local validation and commit
```

## A compact `knowledge.start`

Input:

```json
{
  "task": "continue jp2lam decoder optimization",
  "paths": [
    "lege-codecs/jp2lam/src/decode/**",
    "lege-codecs/jp2lam/src/dwt/**"
  ],
  "budget_tokens": 1400
}
```

Output:

```json
{
  "context_id": "ctx_01K...",
  "ledger_revision": "sha256:5a0aa895...",
  "primary_work": {
    "ref": "@lege-ecosystem.work.jp2lam-decoder-optimization/2",
    "state": "active",
    "title": "jp2lam decoder optimization",
    "intent": "Improve large JPX decode latency and memory use."
  },
  "ready_work": [
    {
      "ref": "@lege-ecosystem.work.aligned-origin-dwt/1",
      "title": "Route aligned nonzero origins through optimized DWT",
      "reason": "Low-risk unfinished Phase 1 item."
    }
  ],
  "constraints": [
    {
      "ref": "@lege-ecosystem.decision.jp2lam-optimization-order/1",
      "text": "Repair structural memory traffic before MQ micro-optimization."
    }
  ],
  "acceptance": [
    "Aligned nonzero tiles use the optimized backend.",
    "Odd-phase tiles remain differential-test correct."
  ],
  "sources": [
    {
      "document": "jp2lam-decoder-performance-audit-2026-08-05",
      "section": "6. P1: nonzero tile origins...",
      "lines": "283-316"
    }
  ]
}
```

No canonical record source. No full Markdown. No complete relation closure. No generated view. No duplicated prose.

The agent can request detail for one item when needed.

## A compact `knowledge.update`

All work transitions, evidence, checks, and relations should be submitted in one batch:

```json
{
  "context_id": "ctx_01K...",
  "base_revision": "sha256:5a0aa895...",
  "operations": [
    {
      "operation": "revise",
      "record": "@lege-ecosystem.work.aligned-origin-dwt/1",
      "state": "completed"
    },
    {
      "operation": "add_evidence",
      "for": "@lege-ecosystem.work.aligned-origin-dwt/2",
      "result": "pass",
      "summary": "OpenJPEG differential matrix passed."
    },
    {
      "operation": "satisfy_check",
      "check": "aligned-origin-differential",
      "evidence": "$previous"
    }
  ]
}
```

Successful response:

```json
{
  "applied": 3,
  "ledger_revision": "sha256:831cdf...",
  "records": [
    "@lege-ecosystem.work.aligned-origin-dwt/2",
    "@lege-ecosystem.evidence.aligned-origin-differential/1"
  ]
}
```

That should be a few hundred tokens, not a rendering of the revised records.

# Establish hard output budgets

Do not rely on Claude Code’s 25,000-token MCP ceiling. Enforce much lower limits inside AKR.

A reasonable initial policy is:

| Operation            | Default output target |  Hard normal limit |
| -------------------- | --------------------: | -----------------: |
| Start/orient task    |      800–1,500 tokens |              2,000 |
| Search               |               300–600 |              1,000 |
| Read one record      |             500–1,000 |              1,500 |
| Read source section  |             700–1,500 |              2,500 |
| Context refresh      |               200–600 |              1,000 |
| Successful write     |               100–300 |                500 |
| Validation success   |             Under 100 |                250 |
| Validation failure   |             500–1,500 |              3,000 |
| Explicit full export |         User-selected | Separate operation |

These are engineering targets, not protocol requirements.

A server-side response guard could be conceptually:

```rust
pub struct OutputBudget {
    pub target_tokens: usize,
    pub hard_tokens: usize,
}

pub fn enforce_budget(
    mut response: AgentResponse,
    budget: OutputBudget,
) -> AgentResponse {
    response.drop_canonical_source();
    response.limit_relations(8);
    response.limit_records(10);
    response.truncate_prose_to(budget.target_tokens);

    if response.estimated_tokens() > budget.hard_tokens {
        response = response.to_summary_with_cursor();
    }

    response
}
```

When material is omitted, return a cursor:

```json
{
  "truncated": true,
  "continuation": "page_01K...",
  "remaining_records": 14
}
```

Do not return the omitted material automatically.

# Eliminate representation duplication

Audit every MCP result for these forms of duplication:

```text
human-readable text
+ the same content as structured JSON
+ canonical AKR source text
+ generated Markdown projection
+ relation records repeated in both directions
```

The response can contain readable text and structured fields, but the fields should not reproduce the entire readable body.

Bad:

```json
{
  "content": [{"text": "<5,000-token context>"}],
  "structuredContent": {
    "rendered_context": "<same 5,000-token context>",
    "records": ["<complete bodies again>"]
  }
}
```

Better:

```json
{
  "content": [{
    "text": "Primary work: aligned-origin DWT. State: active. Two checks remain."
  }],
  "structuredContent": {
    "primary_ref": "@.../1",
    "state": "active",
    "remaining_check_ids": ["phase-routing", "differential"]
  }
}
```

Instrument actual serialized response size. Do not assume the client deduplicates text and structured content.

# Use deltas after the first call

Once the agent has a working set, later calls should not retransmit it.

```json
{
  "context_id": "ctx_01K...",
  "since_ledger_revision": "sha256:5a0aa895..."
}
```

Response:

```json
{
  "changed": [
    {
      "ref": "@lege-ecosystem.work.aligned-origin-dwt/2",
      "transition": "active -> completed"
    }
  ],
  "removed": [],
  "new_revision": "sha256:831cdf..."
}
```

A task context should be an immutable snapshot plus deltas, not a repeatedly rendered graph closure.

# Do not give every subagent direct AKR access

This is likely a major source of duplication.

The parent agent should make one AKR call and construct a small subagent packet:

```text
Task:
Inspect nonzero-origin DWT dispatch.

Relevant work:
@...aligned-origin-dwt/1

Current state:
active

Constraints:
- Preserve genuinely odd phase handling.
- Validate 5/3 and 9/7 across reductions.

Relevant source:
audit §6, lines 283–316

Return:
code findings and recommended patch only.
Do not query or update AKR.
```

Only the coordinating agent should normally:

* Discover the current plan.
* Decide state transitions.
* Add evidence.
* Update the ledger.
* Prepare the Git change transaction.

Subagents performing code search, benchmarks, or implementation do not need independent copies of the project graph.

Exceptions can be explicit:

```text
AKR reviewer subagent
planning reconciliation subagent
evidence auditor
```

This changes AKR from “every agent repeatedly queries global state” into “one coordinator distributes a bounded task state.”

# Make AKR adaptive to task size

A universal mandatory workflow is wasteful. A one-line typo fix does not need the same context as a cross-platform architectural change.

Use four operating levels.

## Level 0 — mechanical change

Examples:

```text
formatting
comment correction
dependency lock refresh
CI image pin
```

Behavior:

* No AKR planning read.
* Use a change transaction with an explicit maintenance exemption.
* Hooks and CI still validate the ledger.

## Level 1 — exact work key already known

Behavior:

```text
one summary read
implementation
one batched update
```

No search or graph context.

## Level 2 — ambiguous implementation task

Behavior:

```text
knowledge.start with paths and task
one targeted detail read if needed
one batched update
```

## Level 3 — planning, reconciliation, or cross-project work

Behavior:

* Larger bounded context.
* Source search.
* Impact analysis.
* Explicit review.

This is where AKR’s greater context cost is justified.

The core rule should be:

> Consult AKR at task and state-transition boundaries, not continuously after every code edit.

# Keep deterministic checks quiet

The previous agent ran nineteen commands to update and validate the ledger. Even when each output is moderate, this creates context accumulation.

Bundle the workflow:

```bash
akr work finish \
  --work raw-autotune.work.slice-6-uncertainty-gated-chroma-limiting-phase \
  --evidence-file /tmp/slice6-evidence.json \
  --build \
  --check \
  --quiet
```

Successful output:

```text
completed @raw-autotune.work.slice-6.../2 evidence=@.../1
```

Failure:

```text
AKR-C014: check "one-channel-clip-regression" lacks passing evidence
```

Then:

```bash
akr explain AKR-C014
```

only when explanation is needed.

This follows a useful Unix-like principle:

```text
success is quiet
failure is specific
detail is requested
```

# Instrument before making another architectural judgment

The usage display is approximate. Add exact AKR-side telemetry.

Per tool call, log:

```json
{
  "tool": "knowledge.start",
  "pid": 23145,
  "sequence": 3,
  "input_chars": 218,
  "output_text_chars": 4210,
  "output_structured_chars": 1180,
  "estimated_output_tokens": 1348,
  "records_considered": 84,
  "records_returned": 7,
  "duration_ms": 42,
  "truncated": false
}
```

Aggregate:

```text
calls by tool
median and p95 output tokens
largest result
total result tokens
records returned per result
duplicate text/JSON bytes
calls per session
calls per subagent
```

Add:

```bash
akr mcp stats --since 24h
```

Desired report:

```text
AKR MCP — last 24 hours

calls                         47
output tokens             31,240
median/call                   410
p95/call                    1,720
largest call                4,811

knowledge.context
  calls                        12
  tokens                   21,600
  share                       69%

duplicated text/json        8,940
subagent calls                 19
```

That will tell you whether the problem is:

* One oversized context tool.
* Too many small calls.
* Duplicate structured/text output.
* Subagent repetition.
* Full record bodies.
* Source retrieval.
* Validation chatter.

Without this instrumentation, changing the search index or storage engine would be guesswork.

# Run a controlled comparison

Do not compare AKR to Markdown through impressions. Use the same tasks in fresh sessions.

Test three configurations:

```text
A. Markdown planning folder, AKR disabled
B. Current AKR workflow
C. Compact AKR workflow
```

Use representative tasks:

1. Small known bug.
2. Multi-module implementation.
3. Continuation of work started by another agent.
4. Cross-OS handoff.
5. A task where an old plan conflicts with current code.
6. A task containing completed code but stale project state.

Measure:

```text
input and cache-read tokens
output tokens
tool result tokens
context size at first code edit
context size at completion
number of tool calls
time to correct work selection
incorrect or obsolete plan selected
duplicate work performed
ledger/code divergence at handoff
human corrections required
```

## Suggested success criteria

For ordinary known-work tasks:

```text
AKR token overhead versus Markdown: <= 10%
MCP calls before code inspection: <= 2
AKR result tokens before code inspection: <= 2,500
```

For cross-agent or cross-session tasks:

```text
total usage no worse than Markdown
or
measurably fewer wrong-plan selections, duplicated changes, or stale handoffs
```

Across the full test set:

```text
no raw ledger reads
no generated-view reads as authority
no repeated full-context calls
no ordinary MCP result above 3,000 tokens
```

These thresholds are design choices, but they force an honest decision.

# The kill criterion should apply to the MCP layer, not immediately to AKR

After implementing compact responses, subagent isolation, delta refreshes, and quiet deterministic commands:

* If AKR still adds more than roughly 20–25% total token use on representative work;
* And it does not measurably reduce rework, wrong decisions, or stale handoffs;

then disable the AKR MCP server by default.

Do **not** necessarily remove:

* The canonical ledger.
* Acceptance checks.
* Evidence.
* Source citations.
* Git change transactions.
* Commit trailers.
* Hooks.
* CI validation.
* Generated project-state views.

Instead, generate one compact file:

```text
.akr/generated/AGENT-BRIEF.md
```

approximately 1,000–2,000 tokens:

```markdown
# Current project state

## Active work
...

## Ready work
...

## Governing constraints
...

## Acceptance checks
...

## Recent changes
...
```

Agents read it once at task start. They update AKR through concise CLI commands. Git hooks ensure synchronization.

That architecture would still preserve most of AKR’s value while removing persistent MCP retrieval from ordinary sessions.

# My assessment

AKR is not yet a failed experiment.

The evidence you have points to a more specific diagnosis:

> **AKR’s state model is useful; its conversational delivery is currently overused and insufficiently bounded.**

It has already caught a consequential class of failure: code that was implemented and tested while the project’s durable state still said that nothing had progressed.  The subsequent reconciliation recorded completed and active work, attached evidence, regenerated the views, and restored a clean AKR validation state. 

That is precisely the problem a folder of Markdown usually does not solve.

But a planning system that consumes nearly half of the usage associated with your recent sessions is not acceptable merely because it is conceptually sound. The next development pass should be treated as a **token-performance optimization pass** with hard budgets and measured regressions.

The target is not “AKR costs no tokens.” It is:

> **One small AKR packet should replace a larger amount of repository exploration, stale-plan reading, repeated explanation, and corrective work.**

Until that is true, AKR should remain valuable infrastructure under test—not mandatory conversational overhead on every agent and every turn.

[1]: https://support.anthropic.com/en/articles/9797557-usage-limit-best-practices "Usage limit best practices | Claude Help Center"
[2]: https://docs.anthropic.com/id/docs/claude-code/mcp "Hubungkan Claude Code ke alat melalui MCP - Claude Code Docs"
