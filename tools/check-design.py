#!/usr/bin/env python3
"""Coherence checker for the AKR design set.

Runs eight independent checks over the repository and exits non-zero with a readable
report if any fails. Standard library only, Python 3.8+.

    (a) links          every relative Markdown link and #anchor resolves
    (b) codes          diagnostic-code closure and range ownership
    (c) vocabulary     kind / state / relation names match spec/tables/vocabulary.json
    (d) manifest       MANIFEST.md inventory agrees with the example corpus
    (e) references     every @key[/rev][#anchor] in the corpus resolves
    (f) grammar        shallow lint of .akr files
    (g) fixtures       every V-rule has a fixture or an explicit waiver
    (h) terminology    banned variants from docs/14-glossary.md

Two severities, mirroring AKR's own:

    error    the design set is incoherent — a broken link, a code cited but registered
             nowhere, a code outside its owner's range, a corpus that disagrees with
             the frozen manifest. Exits 1.
    warning  a documentation gap that breaks nothing — most often a registered
             diagnostic code that no specification document cites yet, which
             `spec/diagnostics/README.md` §6 explicitly permits. Reported always;
             fails the run only under --pedantic.

Checks whose inputs are missing report SKIPPED rather than failing, so the tool is
useful while the design set is still being written.

    python3 tools/check-design.py [--root DIR] [--verbose] [--pedantic]
"""

import argparse
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

SKIP_DIRS = {".git", ".akr-cache", "node_modules", "target", "__pycache__"}
TEXT_EXT = {".md", ".akr", ".sql", ".py", ".txt", ".ebnf", ".json", ".lock", ".yml",
            ".expected", ""}

CODE_RE = re.compile(r"AKR-([A-Z])(\d{3})")
REGISTRY_ROW_RE = re.compile(r"^\|\s*`(AKR-[A-Z]\d{3})`\s*\|")
REF_RE = re.compile(r"@([a-z][a-z0-9]*(?:-[a-z0-9]+)*(?:\.[a-z][a-z0-9]*(?:-[a-z0-9]+)*)+)"
                    r"(?:/(\d+))?(?:#([a-z][a-z0-9]*(?:-[a-z0-9]+)*))?")
RECORD_RE = re.compile(r"^record\s+(\S+?)/(\d+)\s*:\s*([a-z]+)\s*\{")
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*$")

failures = []
warnings = []
notes = []
skipped = []


def fail(check, msg):
    failures.append((check, msg))


def warn(check, msg):
    warnings.append((check, msg))


def note(check, msg):
    notes.append((check, msg))


def skip(check, msg):
    skipped.append((check, msg))


def walk(root, exts=None):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS)
        for name in sorted(filenames):
            ext = os.path.splitext(name)[1]
            if exts is None or ext in exts:
                yield os.path.join(dirpath, name)


def read(path):
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read()


def rel(path):
    return os.path.relpath(path, ROOT)


def slug(heading):
    """GitHub-flavoured heading anchor."""
    text = re.sub(r"`([^`]*)`", r"\1", heading)
    text = re.sub(r"\*\*([^*]*)\*\*", r"\1", text)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = text.lower()
    text = "".join(ch for ch in text if ch.isalnum() or ch in " -_")
    return text.strip().replace(" ", "-")


def anchors_of(path):
    out = set()
    in_fence = False
    for line in read(path).splitlines():
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        m = HEADING_RE.match(line)
        if m:
            out.add(slug(m.group(2)))
    return out


# --------------------------------------------------------------------------- (a)

def planned_paths():
    """Paths the repository README's document map promises but that may not be written
    yet. A link to one of these that does not resolve is a warning, not an error."""
    readme = os.path.join(ROOT, "README.md")
    out = set()
    if not os.path.exists(readme):
        return out
    for line in read(readme).splitlines():
        for m in re.finditer(r"`([A-Za-z0-9_./-]+\.(?:md|akr|sql|ebnf|json|lock))`", line):
            out.add(os.path.normpath(os.path.join(ROOT, m.group(1))))
        for m in re.finditer(r"`([A-Za-z0-9_./-]+/)`", line):
            out.add(os.path.normpath(os.path.join(ROOT, m.group(1))))
    return out


def is_legacy_fixture(path):
    """A synthetic pre-AKR document under an example's `legacy/` directory.

    These fixtures exist to *be* an unmigrated Markdown pile — the raw material `akr
    import` reads and the migration audit reasons about (`docs/12-migration.md`). Their
    dead links are deliberate: `examples/save-your-skin/docs/legacy/ROADMAP.md` points at
    a `PLAN-v1.md` that is gone on purpose, which is the very condition `AKR-M022` names.
    They are content, not part of the design set's cross-reference graph, so the link
    check skips them rather than flagging fiction as incoherence."""
    parts = os.path.relpath(path, ROOT).split(os.sep)
    return "examples" in parts and "legacy" in parts


def check_links():
    anchor_cache = {}
    planned = planned_paths()
    md_files = [p for p in walk(ROOT, {".md"}) if not is_legacy_fixture(p)]
    checked = 0
    for path in md_files:
        in_fence = False
        for lineno, line in enumerate(read(path).splitlines(), 1):
            if line.lstrip().startswith("```"):
                in_fence = not in_fence
                continue
            if in_fence:
                continue
            for target in LINK_RE.findall(line):
                if re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith("//"):
                    continue
                checked += 1
                filepart, _, anchor = target.partition("#")
                if filepart:
                    dest = os.path.normpath(os.path.join(os.path.dirname(path), filepart))
                else:
                    dest = path
                if not os.path.exists(dest):
                    if os.path.normpath(dest) in planned:
                        warn("links", "%s:%d -> %s (planned in README, not yet written)"
                             % (rel(path), lineno, target))
                    else:
                        fail("links", "%s:%d -> %s (no such file)"
                             % (rel(path), lineno, target))
                    continue
                if os.path.isdir(dest):
                    if anchor:
                        fail("links", "%s:%d -> %s (anchor on a directory)"
                             % (rel(path), lineno, target))
                    continue
                if not anchor:
                    continue
                if dest not in anchor_cache:
                    anchor_cache[dest] = anchors_of(dest)
                if anchor not in anchor_cache[dest]:
                    fail("links", "%s:%d -> %s (no such anchor in %s)"
                         % (rel(path), lineno, target, rel(dest)))
    note("links", "%d links checked across %d Markdown files" % (checked, len(md_files)))


# --------------------------------------------------------------------------- (b)

def parse_registry(path):
    codes = {}
    for lineno, line in enumerate(read(path).splitlines(), 1):
        m = REGISTRY_ROW_RE.match(line)
        if m:
            codes[m.group(1)] = lineno
    return codes


def check_codes():
    scheme = os.path.join(ROOT, "spec", "diagnostics", "README.md")
    lang = os.path.join(ROOT, "spec", "diagnostics", "codes-lang.md")
    runtime = os.path.join(ROOT, "spec", "diagnostics", "codes-runtime.md")

    owner = {}
    if os.path.exists(scheme):
        for line in read(scheme).splitlines():
            m = re.match(r"^\|\s*`([A-Z])`\s*\|[^|]*\|\s*`(spec/diagnostics/codes-\w+\.md)`", line)
            if m:
                owner[m.group(1)] = m.group(2)
    if not owner:
        skip("codes", "spec/diagnostics/README.md ownership table not found")
        return

    registries = {}
    for path, name in ((lang, "spec/diagnostics/codes-lang.md"),
                       (runtime, "spec/diagnostics/codes-runtime.md")):
        if os.path.exists(path):
            registries[name] = parse_registry(path)
        else:
            skip("codes", "%s absent; codes owned by it are not verified" % name)

    registered = {}
    for name, codes in registries.items():
        for code in codes:
            if code in registered:
                fail("codes", "%s registered in both %s and %s" % (code, registered[code], name))
            else:
                registered[code] = name

    for code, name in sorted(registered.items()):
        letter = code[4]
        expected = owner.get(letter)
        if expected and expected != name:
            fail("codes", "%s is in %s but letter %s is owned by %s"
                 % (code, name, letter, expected))

    citations = {}
    for path in walk(ROOT, TEXT_EXT):
        if rel(path).startswith("spec/diagnostics/codes-"):
            continue
        try:
            text = read(path)
        except (UnicodeDecodeError, OSError):
            continue
        for m in CODE_RE.finditer(text):
            citations.setdefault(m.group(0), set()).add(rel(path))

    known_absent = set()
    for letter, name in owner.items():
        if name not in registries:
            known_absent.add(letter)

    for code, where in sorted(citations.items()):
        if code in registered:
            continue
        if code[4] in known_absent:
            continue
        fail("codes", "%s is cited in %s but registered nowhere"
             % (code, ", ".join(sorted(where))))

    for code, name in sorted(registered.items()):
        if code not in citations:
            warn("codes", "%s is registered in %s but cited by no document" % (code, name))

    note("codes", "%d codes registered, %d distinct codes cited"
         % (len(registered), len(citations)))


# --------------------------------------------------------------------------- (c)

BANNED_NAMES = {
    "plan": "there is no `plan` kind; a plan is a `work` record designated plan_of_record (D-001)",
    "goal": "there is no `goal` kind; use milestone, track or work (D-010)",
    "needs-review": "`needs-review` is a derived build fact, never an authored state (D-003)",
    "legacy-source": "legacy provenance is a `source { kind legacy }` block, not a kind (D-022)",
    "task": "not a kind; use `work`",
    "epic": "not a kind; use `milestone` or `track`",
    "in-progress": "not a state; the planning lifecycle uses `active`",
    "done": "not a state; the planning lifecycle uses `completed`",
    "todo": "not a state",
    "obsolete": "not a state; use `superseded` or `withdrawn`",
    "deprecated": "not a state; use `superseded` or `withdrawn`",
}

NEGATION = ("no ", "not ", "never", "instead", "there is", "banned", "|",
            "wrong", "absent", "rather than", "does not", "nonsensical", "illegal",
            "anti-goal", "reject", "silently", "would ", "replace", "caught",
            "cannot", "avoid", "fails")

EXPLANATORY_HEADING = re.compile(
    r"reject|anti-goal|not used|prior art|alternative|why not|considered", re.I)


def prose_lines(path):
    """Yield (lineno, line) for prose outside code fences and outside explanatory
    passages — sections and paragraphs whose job is to discuss a banned term in order
    to ban it."""
    in_fence = False
    heading = ""
    explanatory_para = False
    for lineno, line in enumerate(read(path).splitlines(), 1):
        if line.lstrip().startswith("```"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        m = HEADING_RE.match(line)
        if m:
            heading = m.group(2)
            explanatory_para = False
            continue
        if not line.strip():
            explanatory_para = False
            continue
        if re.match(r"\s*\*\*(Why|Rationale|Resolution|Question|Not used)", line):
            explanatory_para = True
        if explanatory_para or EXPLANATORY_HEADING.search(heading):
            continue
        yield lineno, line


def check_vocabulary():
    vocab_path = os.path.join(ROOT, "spec", "tables", "vocabulary.json")
    if not os.path.exists(vocab_path):
        skip("vocabulary", "spec/tables/vocabulary.json absent")
        return
    vocab = json.loads(read(vocab_path))
    kinds = set(vocab["kinds"])
    states = set()
    for lc in vocab["lifecycles"].values():
        states.update(lc["states"])
    relations = set(vocab["relations"])
    legal = kinds | states | relations

    for name in BANNED_NAMES:
        if name in legal:
            fail("vocabulary", "banned name %r is actually in vocabulary.json" % name)

    scanned = 0
    for path in walk(os.path.join(ROOT, "docs"), {".md"}):
        base = os.path.basename(path)
        if base in ("DECISIONS.md", "14-glossary.md"):
            continue
        scanned += 1
        for lineno, line in prose_lines(path):
            lower = line.lower()
            if any(marker in lower for marker in NEGATION):
                continue
            if not re.search(r"\bkind\b|\bstate\b|\blifecycle\b", lower):
                continue
            for token in re.findall(r"`([a-z][a-z0-9_-]*)`", line):
                if token in BANNED_NAMES:
                    fail("vocabulary", "%s:%d uses `%s` — %s"
                         % (rel(path), lineno, token, BANNED_NAMES[token]))
    note("vocabulary", "%d kinds, %d states, %d relations; %d documents scanned"
         % (len(kinds), len(states), len(relations), scanned))


# --------------------------------------------------------------------------- corpus

def load_corpus(akr_root):
    """Return (records, files, namespaces) for one .akr tree."""
    records = {}          # key -> {rev: {kind, state, anchors, retired, file}}
    files = {}            # key -> set(paths)
    for path in walk(akr_root, {".akr"}):
        if os.path.basename(path) == "akr.lock":
            continue
        text = read(path)
        current = None
        depth = 0
        in_prose = False
        for line in text.splitlines():
            if line.count('"""') == 1:
                in_prose = not in_prose
                continue
            if in_prose:
                continue
            m = RECORD_RE.match(line)
            if m:
                key, revno, kind = m.group(1), int(m.group(2)), m.group(3)
                current = records.setdefault(key, {}).setdefault(
                    revno, {"kind": kind, "state": None, "anchors": set(),
                            "retired": set(), "file": rel(path)})
                files.setdefault(key, set()).add(rel(path))
                depth = 1
                continue
            if current is None:
                continue
            stripped = line.strip()
            m = re.match(r"^(claim|check)\s+([a-z][a-z0-9-]*)\s*\{", stripped)
            if m:
                current["anchors"].add(m.group(2))
            m = re.match(r"^state\s+([a-z][a-z0-9-]*)\s*$", stripped)
            if m and current["state"] is None:
                current["state"] = m.group(1)
            m = re.match(r"^retired_claims\s*\[(.*)\]", stripped)
            if m:
                for a in m.group(1).split(","):
                    a = a.strip()
                    if a:
                        current["retired"].add(a)
            depth += line.count("{") - line.count("}")
            if depth <= 0:
                current = None
    namespaces = set()
    project = os.path.join(akr_root, "project.akr")
    if os.path.exists(project):
        for line in read(project).splitlines():
            m = re.match(r"^namespace\s+([a-z][a-z0-9-]*)\b", line.strip())
            if m:
                namespaces.add(m.group(1))
    return records, files, namespaces


# --------------------------------------------------------------------------- (d)

MANIFEST_ROW_RE = re.compile(
    r"^\|\s*`([a-z][a-z0-9.-]+)`\s*\|\s*(\w+)\s*\|\s*(\d+)\s*\|\s*([a-z-]+)\s*\|\s*`([^`]+)`\s*\|")


def check_manifest():
    manifest = os.path.join(ROOT, "examples", "save-your-skin", "MANIFEST.md")
    akr_root = os.path.join(ROOT, "examples", "save-your-skin", ".akr")
    if not os.path.exists(manifest):
        skip("manifest", "examples/save-your-skin/MANIFEST.md absent")
        return
    if not os.path.isdir(akr_root):
        skip("manifest", "examples/save-your-skin/.akr absent (Writer A's files)")
        return

    declared = {}
    for line in read(manifest).splitlines():
        m = MANIFEST_ROW_RE.match(line)
        if m and "." in m.group(1):
            declared[m.group(1)] = (m.group(2), int(m.group(3)), m.group(4), m.group(5))

    if not declared:
        skip("manifest", "no inventory rows parsed from MANIFEST.md §5")
        return

    records, files, _ = load_corpus(akr_root)

    live = {
        "proposed", "active", "verified", "ready", "blocked", "open", "deferred",
    }

    for key, (kind, revs, head_state, filename) in sorted(declared.items()):
        if key not in records:
            fail("manifest", "%s is in MANIFEST but not in the corpus" % key)
            continue
        got = records[key]
        if len(got) != revs:
            fail("manifest", "%s: MANIFEST says %d revision(s), corpus has %d"
                 % (key, revs, len(got)))
        kinds = {r["kind"] for r in got.values()}
        if kinds != {kind}:
            fail("manifest", "%s: MANIFEST says kind %s, corpus has %s"
                 % (key, kind, ", ".join(sorted(kinds))))
        heads = [r for r in got.values() if r["state"] == head_state]
        if not heads:
            fail("manifest", "%s: MANIFEST says head state %s, corpus has %s"
                 % (key, head_state,
                    ", ".join(sorted(str(r["state"]) for r in got.values()))))
        live_revs = [n for n, r in got.items() if r["state"] in live]
        if len(live_revs) > 1:
            fail("manifest", "%s: %d live revisions (V-012)" % (key, len(live_revs)))
        for path in sorted(files.get(key, ())):
            # Windows walks yield backslash paths; MANIFEST names are always /-separated.
            if not path.replace("\\", "/").endswith(filename):
                fail("manifest", "%s: MANIFEST says %s, corpus has %s"
                     % (key, filename, path))
        if len(files.get(key, ())) > 1:
            fail("manifest", "%s: revisions split across %s (V-003)"
                 % (key, ", ".join(sorted(files[key]))))

    for key in sorted(records):
        if key not in declared:
            fail("manifest", "%s is in the corpus but not in MANIFEST" % key)

    total_revs = sum(len(v) for v in records.values())
    note("manifest", "%d keys / %d revisions agree with MANIFEST"
         % (len(records), total_revs))


# --------------------------------------------------------------------------- (e)

def check_references():
    akr_root = os.path.join(ROOT, "examples", "save-your-skin", ".akr")
    if not os.path.isdir(akr_root):
        skip("references", "examples/save-your-skin/.akr absent (Writer A's files)")
        return
    records, files, namespaces = load_corpus(akr_root)
    if not namespaces:
        skip("references", ".akr/project.akr declares no namespaces")
        return

    def head_of(key):
        revs = records[key]
        live = {"proposed", "active", "verified", "ready", "blocked", "open", "deferred"}
        for n, r in sorted(revs.items()):
            if r["state"] in live:
                return n
        return max(revs)

    count = 0
    for path in walk(akr_root, {".akr"}):
        text = read(path)
        for lineno, line in enumerate(text.splitlines(), 1):
            if line.strip().startswith("#"):
                continue
            for m in REF_RE.finditer(line):
                key, revno, anchor = m.group(1), m.group(2), m.group(3)
                count += 1
                ns = key.split(".")[0]
                if ns not in namespaces:
                    fail("references", "%s:%d @%s — namespace %r not declared (V-002)"
                         % (rel(path), lineno, key, ns))
                    continue
                if key not in records:
                    fail("references", "%s:%d @%s — no such key (V-001)"
                         % (rel(path), lineno, key))
                    continue
                target = int(revno) if revno else head_of(key)
                if target not in records[key]:
                    fail("references", "%s:%d @%s/%d — no such revision (V-001)"
                         % (rel(path), lineno, key, target))
                    continue
                if anchor:
                    rec = records[key][target]
                    if anchor not in rec["anchors"] and anchor not in rec["retired"]:
                        fail("references", "%s:%d @%s/%d#%s — no such anchor (V-004)"
                             % (rel(path), lineno, key, target, anchor))

    for key, paths in sorted(files.items()):
        if len(paths) > 1:
            fail("references", "%s: revisions in %s (V-003 requires one file)"
                 % (key, ", ".join(sorted(paths))))

    note("references", "%d references resolved across %d keys" % (count, len(records)))


# --------------------------------------------------------------------------- (f)

KEY_SEG = r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*"
KEY_RE = re.compile(r"^%s(?:\.%s){1,7}$" % (KEY_SEG, KEY_SEG))
SLOT_RE = re.compile(r"^[a-z][a-z0-9_]*$")
COMMIT_RE = re.compile(r"\bgit:([0-9a-fA-F]*)")
TS_RE = re.compile(r"\b\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\S*)")


def check_grammar():
    roots = [os.path.join(ROOT, "examples"), os.path.join(ROOT, "fixtures"),
             os.path.join(ROOT, "spec")]
    paths = []
    for r in roots:
        if os.path.isdir(r):
            paths.extend(walk(r, {".akr"}))
    lock = os.path.join(ROOT, "examples", "save-your-skin", ".akr", "akr.lock")
    if os.path.exists(lock):
        paths.append(lock)
    if not paths:
        skip("grammar", "no .akr files found")
        return

    skipped_err = 0
    for path in paths:
        if re.search(r"fixtures/(parse|validate|format)/err/", rel(path).replace(os.sep, "/")):
            skipped_err += 1
            continue
        with open(path, "rb") as fh:
            raw = fh.read()
        if b"\r" in raw:
            fail("grammar", "%s contains CR (LF-only required)" % rel(path))
        if raw and not raw.endswith(b"\n"):
            fail("grammar", "%s does not end with a newline" % rel(path))
        if raw.endswith(b"\n\n"):
            fail("grammar", "%s ends with a blank line" % rel(path))
        if raw.startswith(b"\xef\xbb\xbf"):
            fail("grammar", "%s starts with a UTF-8 BOM" % rel(path))

        text = raw.decode("utf-8")
        fences = text.count('"""')
        if fences % 2:
            fail("grammar", "%s has an unbalanced triple-quote fence (%d)" % (rel(path), fences))

        depth = 0
        in_prose = False
        for lineno, line in enumerate(text.splitlines(), 1):
            if line.count('"""') == 1:
                in_prose = not in_prose
                continue
            if in_prose:
                continue
            code = "" if line.lstrip().startswith("#") else re.sub(r"(^|\s)#.*$", "", line)
            depth += code.count("{") + code.count("[") - code.count("}") - code.count("]")
            if depth < 0:
                fail("grammar", "%s:%d closes more brackets than it opens" % (rel(path), lineno))
                depth = 0
            if line.strip() and not line.lstrip().startswith("#"):
                indent = len(line) - len(line.lstrip(" "))
                if indent % 4:
                    fail("grammar", "%s:%d indent %d is not a multiple of 4"
                         % (rel(path), lineno, indent))
                if line.lstrip(" ") != line.lstrip():
                    fail("grammar", "%s:%d indented with a tab" % (rel(path), lineno))
            if line.rstrip() != line:
                fail("grammar", "%s:%d has trailing whitespace" % (rel(path), lineno))
            m = RECORD_RE.match(line)
            if m and not KEY_RE.match(m.group(1)):
                fail("grammar", "%s:%d key %r is not in key-segment form (D-005)"
                     % (rel(path), lineno, m.group(1)))
            for hexpart in COMMIT_RE.findall(code):
                if len(hexpart) != 40 or not re.match(r"^[0-9a-f]{40}$", hexpart):
                    fail("grammar", "%s:%d git:%s is not 40 lowercase hex digits (D-008)"
                         % (rel(path), lineno, hexpart))
            for suffix in TS_RE.findall(code):
                if suffix != "Z":
                    fail("grammar", "%s:%d timestamp must end in Z (D-008)" % (rel(path), lineno))
            m = re.match(r"^([a-z][a-z0-9_-]*)\s", line.strip())
            if m and m.group(1) not in ("record", "namespace", "project", "akr",
                                        "akr-lock", "claim", "check", "source",
                                        "disposition", "acceptance", "defaults",
                                        "build", "resolution", "sealed", "path"):
                if not SLOT_RE.match(m.group(1)):
                    fail("grammar", "%s:%d slot name %r must be snake_case (D-005)"
                         % (rel(path), lineno, m.group(1)))
        if depth != 0:
            fail("grammar", "%s has %d unclosed bracket(s)" % (rel(path), depth))

    note("grammar", "%d .akr files linted, %d deliberate-failure fixtures skipped"
         % (len(paths) - skipped_err, skipped_err))


# --------------------------------------------------------------------------- (g)

def check_fixtures():
    rules_doc = os.path.join(ROOT, "docs", "05-validation-rules.md")
    fix_root = os.path.join(ROOT, "fixtures")
    fix_readme = os.path.join(fix_root, "README.md")
    if not os.path.exists(rules_doc):
        skip("fixtures", "docs/05-validation-rules.md absent (Writer A's file)")
        return
    if not os.path.isdir(fix_root):
        skip("fixtures", "fixtures/ absent (Writer A's files)")
        return

    rules = set(re.findall(r"\bV-(\d{3})\b", read(rules_doc)))
    rules = {r for r in rules if r.startswith("0")}
    if not rules:
        skip("fixtures", "no V-rule ids found in docs/05-validation-rules.md")
        return

    covered = set()
    expected_files = []
    for path in walk(os.path.join(fix_root, "validate"), None) if \
            os.path.isdir(os.path.join(fix_root, "validate")) else []:
        base = os.path.basename(path)
        m = re.search(r"\bv(\d{3})\b", rel(path))
        if m:
            covered.add(m.group(1))
        if base.endswith(".expected") or base == "expected":
            expected_files.append(path)

    waived = set()
    if os.path.exists(fix_readme):
        text = read(fix_readme)
        for m in re.finditer(r"V-(\d{3})[^\n]*\b(waiv|no fixture|not fixture-testable)", text, re.I):
            waived.add(m.group(1))

    for rule in sorted(rules - covered - waived):
        fail("fixtures", "V-%s has no validate fixture and no waiver in fixtures/README.md"
             % rule)

    registered = set()
    for name in ("codes-lang.md", "codes-runtime.md"):
        path = os.path.join(ROOT, "spec", "diagnostics", name)
        if os.path.exists(path):
            registered.update(parse_registry(path))
    if registered:
        for path in expected_files:
            for line in read(path).splitlines():
                line = line.strip()
                if not line or line.startswith("#"):
                    continue
                m = CODE_RE.match(line)
                if not m:
                    fail("fixtures", "%s: %r does not begin with a diagnostic code"
                         % (rel(path), line))
                elif m.group(0) not in registered:
                    fail("fixtures", "%s names unregistered code %s" % (rel(path), m.group(0)))

    note("fixtures", "%d V-rules, %d covered, %d waived, %d .expected files"
         % (len(rules), len(rules & covered), len(waived), len(expected_files)))


# --------------------------------------------------------------------------- (h)

def check_terminology():
    glossary = os.path.join(ROOT, "docs", "14-glossary.md")
    if not os.path.exists(glossary):
        skip("terminology", "docs/14-glossary.md absent")
        return
    banned = {}
    for line in read(glossary).splitlines():
        m = re.match(r"^\|\s*(?:a\s+)?`?([^|`]+?)`?(?:\s+kind)?\s*\|\s*([^|]+?)\s*\|\s*$", line)
        if m and m.group(1) not in ("Not used", "---"):
            banned[m.group(1).strip().strip('"')] = m.group(2).strip()

    phrases = {
        "newest wins": 'use supersession, disposition, `topic` or `acknowledged` (D-004)',
        "newest-wins": 'use supersession, disposition, `topic` or `acknowledged` (D-004)',
        "frontmatter": "AKR uses typed slots, not frontmatter",
        "needs-review state": "`needs-review` is derived, never a state (D-003)",
        "plan kind": "there is no `plan` kind (D-001)",
        "goal kind": "there is no `goal` kind (D-010)",
    }

    scanned = 0
    for path in walk(os.path.join(ROOT, "docs"), {".md"}):
        base = os.path.basename(path)
        if base in ("DECISIONS.md", "14-glossary.md"):
            continue
        scanned += 1
        for lineno, line in prose_lines(path):
            lower = line.lower()
            if any(marker in lower for marker in NEGATION):
                continue
            for phrase, why in phrases.items():
                if phrase in lower:
                    fail("terminology", "%s:%d uses %r — %s"
                         % (rel(path), lineno, phrase, why))
    note("terminology", "%d banned variants from the glossary, %d documents scanned"
         % (len(banned), scanned))


# ---------------------------------------------------------------------------

CHECKS = [
    ("links", check_links),
    ("codes", check_codes),
    ("vocabulary", check_vocabulary),
    ("manifest", check_manifest),
    ("references", check_references),
    ("grammar", check_grammar),
    ("fixtures", check_fixtures),
    ("terminology", check_terminology),
]


def main():
    global ROOT
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--root", default=ROOT)
    ap.add_argument("--verbose", "-v", action="store_true")
    ap.add_argument("--pedantic", action="store_true",
                    help="treat warnings as errors")
    args = ap.parse_args()
    ROOT = os.path.abspath(args.root)

    for name, fn in CHECKS:
        try:
            fn()
        except Exception as exc:  # a checker bug must not look like a design fault
            fail(name, "checker raised %s: %s" % (type(exc).__name__, exc))

    if args.pedantic:
        failures.extend(warnings)
        del warnings[:]

    by_check = {}
    for name, msg in failures:
        by_check.setdefault(name, []).append(msg)
    warn_by_check = {}
    for name, msg in warnings:
        warn_by_check.setdefault(name, []).append(msg)

    print("check-design.py — AKR design set coherence")
    print("root: %s" % ROOT)
    print()
    for name, _ in CHECKS:
        problems = by_check.get(name, [])
        cautions = warn_by_check.get(name, [])
        skips = [m for c, m in skipped if c == name]
        infos = [m for c, m in notes if c == name]
        if problems:
            status = "FAIL (%d error%s%s)" % (
                len(problems), "" if len(problems) == 1 else "s",
                ", %d warning" % len(cautions) if cautions else "")
        elif cautions:
            status = "ok (%d warning%s)" % (len(cautions), "" if len(cautions) == 1 else "s")
        elif skips:
            status = "SKIPPED"
        else:
            status = "ok"
        print("  %-12s %s" % (name, status))
        for m in skips:
            print("      - skipped: %s" % m)
        if args.verbose:
            for m in infos:
                print("      . %s" % m)
        for m in problems:
            print("      ! %s" % m)
        if args.verbose or args.pedantic:
            for m in cautions:
                print("      ? %s" % m)
        elif cautions:
            print("      ? %d warning(s); rerun with --verbose to list, "
                  "--pedantic to fail on them" % len(cautions))
    print()
    if failures:
        print("%d error(s) in %d check(s)." % (len(failures), len(by_check)))
        return 1
    print("All checks passed%s%s."
          % (" (%d warning%s)" % (len(warnings), "" if len(warnings) == 1 else "s")
             if warnings else "",
             " (%d skipped)" % len(skipped) if skipped else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
