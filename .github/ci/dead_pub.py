"""Find public items in timegrapher-core that nothing outside tests uses (#1522).

  dead_pub.py check     fail if the list differs from the baseline
  dead_pub.py list      print the current list
  dead_pub.py update    write the current list as the baseline

The architecture guide (section 12, "Dead code") says a public item with no
caller outside its tests is removed, judged on all three platforms. The
compiler already fails CI for a private or `pub(crate)` item nothing uses on
the platform it builds for, and the app, CLI and MCP server are programs, so
there even `pub` counts. timegrapher-core is a library, so the compiler never
calls one of its `pub` items unused. This check does.

It reads the source as text, so code for every platform counts at once: an
item called only under `#[cfg(windows)]` is used, wherever this runs. An item
is used when its name appears in code (strings included: format captures and
serde's `skip_serializing_if` name functions) outside test code, other than
where it is defined. Test code is the crates' tests/ folders, anything after
a `#[cfg(test)]` attribute, and modules declared under one. Comments never
count. An item whose name is used for anything else, a local included,
counts as used (`let noted` hides a method `noted`), so the list can miss
an item but never names a used one.

The list must equal the baseline (dead_pub_baseline.json), as the literal
ratchet's counts must: a PR that leaves a new dead item fails, and a PR that
removes or starts calling one drops it from the baseline in the same PR. The
baseline may only shrink: an entry the base commit's baseline (HEAD^1, as CI
checks out a PR's merge commit) lacks fails too.
Offline, standard library only, a few seconds.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINE = Path(__file__).with_name("dead_pub_baseline.json")
ITEM_KINDS = {"fn", "struct", "enum", "const", "static", "trait", "type", "union"}
FN_PREFIXES = {"const", "async", "unsafe", "extern"}

TOKEN = re.compile(r"""
    (?P<line>//[^\n]*)
  | (?P<block>/\*)
  | (?P<raw>b?r\#*")
  | (?P<str>b?"(?:\\.|[^"\\])*")
  | (?P<char>b?'(?:\\.|[^'\\])')
  | (?P<life>'[A-Za-z_][A-Za-z0-9_]*)
  | (?P<word>[A-Za-z_][A-Za-z0-9_]*)
  | (?P<punct>->|=>|::|[{}()\[\];#=!,:<>])
""", re.X | re.S)
IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")


def tokens(text):
    """(kind, text, offset) for each word, string and brace-like mark."""
    out, i, n = [], 0, len(text)
    while i < n:
        m = TOKEN.search(text, i)
        if not m:
            break
        kind = m.lastgroup
        if kind == "block":
            depth, j = 1, m.end()
            while depth and j < n:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            i = j
            continue
        if kind == "raw":
            end = '"' + "#" * m.group().count("#")
            j = text.find(end, m.end())
            j = n if j < 0 else j
            out.append(("str", text[m.end():j], m.start()))
            i = j + len(end)
            continue
        if kind == "str":
            out.append(("str", m.group(), m.start()))
        elif kind in ("word", "punct"):
            out.append((kind, m.group(), m.start()))
        i = m.end()
    return out


def attr_end(toks, i):
    """Index after the attribute starting at toks[i] == '#', or None."""
    j = i + 1
    if j < len(toks) and toks[j][1] == "!":
        j += 1
    if j >= len(toks) or toks[j][1] != "[":
        return None
    depth = 0
    while j < len(toks):
        if toks[j][1] == "[":
            depth += 1
        elif toks[j][1] == "]":
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    return j


def is_test_cfg(toks, start, end):
    """`#[cfg(test)]` or `#[cfg(all(test, ...))]`."""
    words = [t[1] for t in toks[start:end] if t[0] == "word"]
    return words[:2] == ["cfg", "test"] or words[:3] == ["cfg", "all", "test"]


ITEM_WORDS = {"fn", "mod", "impl", "struct", "enum", "trait", "type", "const", "static",
              "union", "use", "extern", "macro_rules", "async", "unsafe"}
OPEN, CLOSE = "([{", ")]}"


def item_end(toks, i):
    """Index after what a `#[cfg(test)]` at toks[i] covers.

    An item (`fn`, `mod`, `impl`, ...) ends at its braces or its `;`. A
    `let` ends at its `;`. A field, in a struct or a struct literal, ends at
    its `,`, and its type's `<...>` are brackets. Anything else (a match arm,
    an expression, a tuple field) ends at a `,` or `;`, or at its braces.
    Nothing runs past the `}`, `)` or `]` that closes what it is in."""
    j = i
    if j < len(toks) and toks[j][1] == "pub":
        j += 1
        if j < len(toks) and toks[j][1] == "(":
            j = item_close(toks, j) or j
    first = toks[j][1] if j < len(toks) else ""
    field = j + 1 < len(toks) and toks[j][0] == "word" and toks[j + 1][1] == ":"
    item = first in ITEM_WORDS and not field
    depth = 0
    for k in range(i, len(toks)):
        mark = toks[k][1]
        if mark in OPEN or (field and mark == "<"):
            depth += 1
        elif mark in CLOSE or (field and mark == ">"):
            if depth == 0:
                return k
            depth -= 1
            if mark == "}" and depth == 0 and first != "let" and not field:
                # The item's braces, or a braced arm or expression.
                return k + 1
        elif depth == 0 and (mark == ";" or (mark == "," and not item and first != "let")):
            return k + 1
    return len(toks)


def item_close(toks, j):
    """Index after the bracket group opening at toks[j], or None."""
    depth = 0
    for k in range(j, len(toks)):
        if toks[k][1] in OPEN:
            depth += 1
        elif toks[k][1] in CLOSE:
            depth -= 1
            if depth == 0:
                return k + 1
    return None


def split_tests(toks):
    """(product tokens, test module files declared here as (name, path attr))."""
    kept, test_mods, i = [], [], 0
    while i < len(toks):
        if toks[i][1] == "#":
            end = attr_end(toks, i)
            if end and is_test_cfg(toks, i, end):
                j, path = end, None
                while j < len(toks) and toks[j][1] == "#":
                    e = attr_end(toks, j) or j + 1
                    words = [t for t in toks[j:e] if t[0] in ("word", "str")]
                    if words and words[0][1] == "path" and len(words) > 1:
                        path = words[1][1].strip('"')
                    j = e
                stop = item_end(toks, j)
                item = [t[1] for t in toks[j:stop]]
                if "mod" in item and item[-1] == ";":
                    name = item[item.index("mod") + 1]
                    test_mods.append((name, path))
                i = stop
                continue
        kept.append(toks[i])
        i += 1
    return kept, test_mods


def mod_decls(toks):
    """Every `mod name;` in a file, with its `#[path]` if it has one."""
    found, path = [], None
    for i, (kind, text, _) in enumerate(toks):
        if text == "path" and i + 2 < len(toks) and toks[i + 1][1] == "=" and toks[i + 2][0] == "str":
            path = toks[i + 2][1].strip('"')
        elif text == "mod" and kind == "word" and i + 2 < len(toks) and toks[i + 2][1] == ";":
            found.append((toks[i + 1][1], path))
            path = None
        elif text in ("{", ";"):
            path = None
    return found


def module_files(path, name, attr):
    """The files `mod name;` in `path` may name."""
    here = path.parent
    if attr:
        return [here / attr]
    base = here if path.name in ("lib.rs", "main.rs", "mod.rs") else here / path.stem
    return [base / f"{name}.rs", base / name / "mod.rs"]


def scan(root=ROOT):
    crates = root / "crates"
    core = crates / "timegrapher-core" / "src"
    files = sorted(p for p in crates.rglob("*.rs") if "target" not in p.relative_to(crates).parts)
    parsed = {p: tokens(p.read_text(encoding="utf-8")) for p in files}
    test_files = {p for p in files if "tests" in p.relative_to(crates).parts[1:2]}
    product = {}
    for p in files:
        product[p], mods = split_tests(parsed[p])
        for name, attr in mods:
            test_files.update(f for f in module_files(p, name, attr) if f.exists())
    # Every module a test file declares is test code too.
    grown = True
    while grown:
        grown = False
        for p in sorted(test_files):
            for name, attr in mod_decls(parsed.get(p, [])):
                for f in module_files(p, name, attr):
                    if f.exists() and f not in test_files:
                        test_files.add(f)
                        grown = True
    uses = {}
    for p in files:
        if p in test_files:
            continue
        for kind, text, _ in product[p]:
            names = IDENT.findall(text) if kind == "str" else [text] if kind == "word" else []
            for name in names:
                uses[name] = uses.get(name, 0) + 1
    found = []
    for p in files:
        if p in test_files or core not in p.parents:
            continue
        toks = product[p]
        for i, (kind, text, offset) in enumerate(toks):
            if text != "pub" or kind != "word" or i + 1 >= len(toks):
                continue
            j = i + 1
            # `const fn`, `async fn`, `unsafe extern "C" fn`, but `const NAME`.
            k = j
            while k < len(toks) and (toks[k][1] in FN_PREFIXES or toks[k][0] == "str"):
                k += 1
            if k < len(toks) and toks[k][1] == "fn":
                j = k
            if j + 1 < len(toks) and toks[j][1] in ITEM_KINDS and toks[j + 1][0] == "word":
                name = toks[j + 1][1]
                if uses.get(name, 0) <= 1:
                    rel = p.relative_to(root).as_posix()
                    found.append(f"{rel}::{name}")
    return sorted(set(found))


def base_baseline(root=ROOT):
    """The baseline at HEAD^1, the base a PR merges into (CI checks out the
    merge commit with its parent), or None when git, that commit or that
    file isn't there."""
    rel = BASELINE.relative_to(ROOT).as_posix()
    try:
        done = subprocess.run(["git", "show", f"HEAD^1:{rel}"], cwd=root,
                              capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if done.returncode != 0:
        return None
    return json.loads(done.stdout)


def check(found, baseline, base):
    """The errors: items not in the baseline, baseline items no longer
    found, and baseline items the base's baseline lacks."""
    rel = BASELINE.relative_to(ROOT).as_posix()
    errors = [f"{f} is public and nothing outside tests uses it: remove it, "
              "or make it pub(crate) if only core needs it" for f in found if f not in baseline]
    errors += [f"{g} is no longer dead (removed or now used): drop it from {rel} "
               "(dead_pub.py update)" for g in baseline if g not in found]
    if base is not None:
        errors += [f"{g} is in {rel} but not in the base's: the list may only shrink, "
                   "so remove the item instead" for g in baseline if g not in base]
    return errors


def main(argv, root=ROOT, baseline_path=BASELINE, base=None):
    mode = argv[1] if len(argv) > 1 else "check"
    found = scan(root)
    if mode == "list":
        print("\n".join(found))
        return 0
    if mode == "update":
        baseline_path.write_text(json.dumps(found, indent=1) + "\n", encoding="utf-8")
        print(f"baseline: {len(found)} items")
        return 0
    if mode != "check":
        print(__doc__, file=sys.stderr)
        return 2
    baseline = json.loads(baseline_path.read_text(encoding="utf-8"))
    if base is None and root == ROOT:
        base = base_baseline(root)
    errors = check(found, baseline, base)
    for e in errors:
        print(f"::error::{e}")
    print(f"{len(found)} dead public items in core; baseline {len(baseline)}")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
