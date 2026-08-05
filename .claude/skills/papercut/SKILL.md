---
name: papercut
description: Mine the current session for papercuts — small frictions hit while working — and log each one to the AKR ledger via `akr papercut`. User-triggered only; never run this review unprompted.
---

# /papercut — mine the session for frictions

Review the current session's work and log every papercut to the AKR ledger. A papercut
is a small friction that did not block anything: a tool call that missed and had to be
retried, a confusing or undocumented setup step, a flaky command, a stale cache, a
misleading error, a non-obvious gotcha.

## Steps

1. Re-read the session so far and list every friction you (or a subagent) actually hit.
   Genuine frictions only — not mistakes in your own reasoning, and not things that
   worked on the first try. If a friction was already logged this session, skip it.
2. For each one, compose one or two sentences: what you were doing → what got in the
   way. A guess at the cause or fix is a bonus. Keep it concrete enough that a reader
   could reproduce or investigate it.
3. Log each with the CLI (or the `knowledge.papercut` MCP tool if connected):

   ```
   akr papercut -m <your-model-or-harness-name> "<message>"
   ```

   Use `--namespace <ns>` only if the command reports the project declares several.
4. Report to the user how many were logged, with the one-line messages. If none were
   found, say so — do not invent frictions to have something to log.

This is distinct from durable records (knowledge worth proposing) and from scratch
notes. When a papercut turns out to be a real bug, propose a `work` or `question`
record too and mention the papercut key in it.
