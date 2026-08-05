A few concrete things, all from actual friction this session rather than general impressions:

1. --help doesn't specialize per subcommand. akr propose --help, akr evidence add --help, etc. all print the same top-level command list — I just confirmed this again. Every flag requirement I hit (key segment count, required intent slot for milestones, --result/--method for evidence) I discovered by trial-and-error against error messages, one flag at a time. A akr <command> --help that actually describes that command's args would have saved ~5 round trips.

2. Kind-specific required slots aren't discoverable upfront. knowledge_propose for a milestone failed with "milestone requires slot intent" (V-008) — I only learned this after attempting it. There's no akr explain milestone or schema listing that shows "this kind needs these slots" before you write the payload. The tool description says "Planning kinds accept note" but doesn't enumerate what's required per kind.

3. Key format error was cryptic. "sun-visibility-atlas-runtime-switch" is not a key: found 1 segment — it never says segments are dot-delimited, or that the namespace should be the first segment. I guessed from project.akr's namespace saveyourskin line. A one-line hint in the error (expected form: namespace.topic.slug) would remove the guesswork.

4. Evidence is CLI-only, not in the MCP surface. knowledge_complete's checks map wants an evidence reference, but the only way to create evidence is shelling out to akr evidence add via Bash — there's no mcp__akr__evidence_add. For an agent working primarily through MCP tools, that's a context-switch every time a milestone needs closing out. Given knowledge_complete already exists as an MCP tool, evidence creation feels like the obvious missing sibling.

5. The reference format for checks isn't documented in the tool schema. I inferred @key/rev from AGENTS.md's mention of "D-009" and by pattern-matching the revise/propose reference-array conventions — it worked, but the knowledge_complete tool description doesn't show an example.

What worked well: once past setup, propose → revise (for the progress note) → evidence add × 6 → complete → validate was a clean, honest loop, and akr build's generated ROADMAP.md diff is a genuinely good sanity check — seeing "6 of 6 satisfied" with evidence refs and commit hashes was more convincing than any comment I could've written by hand. The AGENTS.md auto-append from akr init was a good move too — it's the reason I knew the propose→revise→complete verbs existed at all.
