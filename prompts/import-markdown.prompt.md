You are helping migrate an assistant's planning output into AKR, a typed project-knowledge
ledger. AKR has a command, `akr import`, that reads a Markdown document and drafts one
`proposed` record per heading, keeping the first paragraph under each heading verbatim as
an auditable excerpt. Your job is to re-shape my raw notes into a Markdown document that
that command imports cleanly. You are drafting for a human reviewer; you are not deciding
anything.

Output **only** the Markdown document — no preamble, no explanation, no closing remarks.

## Rules

1. **One durable claim per `##` heading.** A claim is durable when it is still worth
   knowing in six months. Drop status ("60% done"), notes to a person ("ask Dana"), and
   anything that is only true today. If nothing durable remains, output a single line:
   `<!-- nothing durable to import -->`.

2. **The heading is the claim, as one short statement** — this becomes the record's
   title. Not a topic label ("Signatures") but the claim itself ("Signature status is
   reported as separate facts, never one trust verdict").

3. **Exactly one paragraph under each heading, and put the durable content there.** The
   importer keeps only that first paragraph as the verbatim excerpt; tables, code blocks,
   lists, and later paragraphs under the same heading are dropped. If a section has
   several claims, split it into several `##` headings.

4. **One `#` H1 at the top: the document's title, with no paragraph under it.** Use `##`
   for every claim.

5. **Steer the kind with wording**, because the importer guesses the kind from keywords
   and can only ever propose these six:
   - end the heading with `?` → **question** (import these freely; an open question is
     the most valuable thing in a pile);
   - use *decided*, *decision*, or *we chose* → **decision**;
   - use *must* or *shall* → **requirement**;
   - use *standing* or *ongoing* → **track**;
   - use *always*, *never*, *every*, or *policy* → **policy**;
   - anything else lands as **work**, which is the safe default the reviewer reclassifies.

   You cannot create `term`, `constraint`, `milestone`, `observation`, `evidence`, or
   `assessment` this way — each needs a slot the prose cannot honestly fill (a definition,
   a measured commit, acceptance checks). Leave those as `work` and note in the paragraph
   what kind the reviewer should promote it to.

6. **Do not invent facts, dates, commit hashes, or file paths.** Write only what my notes
   support. The excerpt must be my meaning in my words, not a paraphrase that a reviewer
   could not check against the original.

## Shape to produce

```markdown
# <short document title>

## <claim one, phrased as a statement>

<one self-contained paragraph carrying the durable content of this claim>

## Should we <the open question>?

<one paragraph stating what is unknown and why it matters>
```

Now convert the following notes:

<paste your assistant output here>
