# Authoring with external LLMs

How to get output from a chat assistant that cannot call AKR's tools — a browser session
at `chatgpt.com`, say — into the ledger without losing the provenance that makes it
auditable. This is the operational companion to [`12-migration.md`](12-migration.md) §6,
which fixes the rule this guide works within: a model may draft structure, but it never
runs inside the write pipeline and never decides authority.

## The problem

An assistant with no tool access answers in prose. You paste that prose somewhere and it
becomes another untyped Markdown file — the exact pile AKR exists to replace. The fix is
to make the assistant's *output* already shaped for the ledger, so the only thing left is
review. Two shapes work, and they trade fidelity against effort. Both keep you as the
reviewer: everything an assistant produces lands non-authoritative until you accept it.

## Option A — import-friendly Markdown

You ask the assistant to emit a Markdown document where each `##` heading is one durable
claim and the paragraph under it is the verbatim rationale, then you run `akr import`. This
is the closest fit to `12-migration.md` §6: the model drafts, `akr import` validates and
writes, you review. The excerpt on every record is a byte-identical slice of the assistant's
own text, so a reviewer can always check the drafting against the source.

- Prompt: [`../prompts/import-markdown.prompt.md`](../prompts/import-markdown.prompt.md)
- Strengths: no knowledge of AKR syntax required; the write pipeline and the verbatim-excerpt
  audit are automatic; a dead link or stale path becomes an ordinary warning you triage.
- Limits: `akr import` keeps only the first paragraph under each heading, and its kind guess
  reaches only six kinds — `question`, `decision`, `requirement`, `track`, `policy`, and the
  `work` default. Everything else you reclassify with `akr revise`.

Run it:

```
# from your project root, with .akr/ already initialised
akr import notes.md --namespace <ns> --dry-run   # read the plan and the warnings
akr import notes.md --namespace <ns>             # write the proposed records + a tracker
akr check                                        # strict validation of the result
```

## Option B — direct `.akr` source

You ask the assistant to emit record source directly, drop it into
`.akr/records/<namespace>/`, and let the compiler catch every mistake. Higher fidelity —
real kinds, scope, relations, acceptance checks — at the cost of a larger prompt and syntax
the assistant can get wrong. It is the right choice when the notes are already
decision-shaped and you want the relation graph, not just an inventory.

- Prompt: [`../prompts/akr-source.prompt.md`](../prompts/akr-source.prompt.md)
- Strengths: the full model is available — `scope`, `implements`, `part_of`, acceptance
  checks — in one pass.
- Limits: the assistant is writing a formal language from a chat window, so treat the output
  as a draft to be *checked*, never trusted. It bypasses the extraction step of `akr import`,
  so you are the one guaranteeing the `source { kind legacy }` provenance is present and the
  excerpt is verbatim.

Run it:

```
# paste the assistant's output into a new file, then:
akr fmt .akr/records/<ns>/imported.akr    # canonicalise; surfaces syntax errors
akr check                                 # strict validation
```

Neither prompt hard-codes the migration tracking record. If you want the disposition
workflow of `12-migration.md` §4 around a batch — one `work` record whose acceptance checks
enumerate what still needs review — `akr import` creates it for you (Option A), or you add it
by hand (Option B).

## Which to reach for

Start with Option A for a long, mixed plan: it turns a wall of prose into a reviewable
inventory in one command, and the rough kinds are cheap to fix. Reach for Option B when the
notes are a handful of real decisions or constraints whose relations to each other are the
point. Trying both on the same source and comparing the review effort is the fastest way to
learn which suits a given assistant's output.

## A note on excerpts

In both options the `excerpt` is the assistant's own words, copied verbatim — never a
paraphrase. Its whole function is to let a reviewer check the structured record against the
text it came from, which a summary would defeat. When the assistant *is* the source, the
excerpt records exactly what it said, which is the honest provenance for a claim you are
choosing to keep.
