"""Decide what a CI run needs: the full build and tests, part of them, or
only the offline tests.

  changes.py EVENT BASE HEAD [--regress CHECK]
      prints full=true or full=false, then pages=true or pages=false, then
      packages= and the nextest/cargo package arguments (blank for the whole
      workspace). With --regress CHECK (engine, app or mcp), full is true only
      when that corpus check is needed.

For a pull request CI checks out GitHub's merge commit, so BASE is HEAD^1
(the base branch) and HEAD is HEAD: their diff is what merging would change.

Everything but a pull request is a full run of every package and check.
A pull request is a light run
only when every file it changes is edited or added in place (never a
deletion or rename) and is either Markdown that nothing compiles in and no
test reads, or a user-guide asset in LIGHT_ASSETS: a screenshot or drawing in
docs/user-guide/images/, or a figure's description in guide-figures/figures/,
which only guide-figures/make.py reads, by hand. The Markdown is never the
watchmaker skill, packaging, core's calibre data folder
(crates/timegrapher-core/data/, whose README a core test reads), or a
document the MCP server compiles in
(MCP_DOCS, kept equal to guidance.rs by the offline tests). In a light run
all eight CI jobs still run and report: the test jobs run only their offline
Python tests, the three regress jobs skip their corpus and engine steps, and mcp-renderer
runs as usual. CI's own scripts and tests that a light run runs in full
(LIGHT_CI) count as Markdown does here. If git fails, this exits with an
error and the job fails.

A pull request that is not light may still be narrow (#2443): when every
file it changes that a light run would not spare is inside one of the crates
in NARROW, the test jobs build and test only those crates' packages, and only
those crates' corpus checks run; the other regress jobs skip their steps and
report, as in a light run. Core and the CLI are never narrow: every other
crate uses them (the app's and the MCP server's corpus checks compare with
`timegrapher session`). A crate's Cargo.toml or build.rs, and any file of a
narrow crate that another crate reads (CROSS_READ, held to the tree by
test_changes.py), make the run full.

pages is true for a pull request, light or full, that changes a file the
downloads render as web pages or link to (PAGE_FILES and PAGE_FOLDERS), so CI runs
Release's render and link check of those pages. Release's own pull-request run
covers packaging/. Everything but a pull request prints pages=false.
"""

import subprocess
import sys


# Markdown under docs/ that crates/timegrapher-mcp/src/guidance.rs compiles in.
MCP_DOCS = {
    "docs/reference/agent-interface.md",
    "docs/reference/sessions.md",
    "docs/reference/series.md",
    "docs/reference/fault-signatures.md",
}
# Markdown here is compiled in (the skill), read by packaging's tests, or
# read by core's calibre tests (data/README.md's confidence list, #2281).
FULL_PREFIXES = ("skills/", "packaging/", "crates/timegrapher-core/data/")
# Guide assets nothing compiles in and no Rust or JavaScript test reads
# (folder, extensions). A screenshot is checked by the pages' render and link
# check below; a figure's description by the offline test that it parses and
# names a screenshot that exists (test_changes.py, GuideFigures).
LIGHT_ASSETS = (
    ("docs/user-guide/images/", (".png", ".jpg", ".svg")),
    ("guide-figures/figures/", (".toml",)),
)
# Files and folders the downloads' pages are rendered from or link to
# (packaging/docs_html.py): a change here re-runs the render and link check.
PAGE_FILES = {"README.md", "LICENSE"}
PAGE_FOLDERS = ("docs/", "skills/watchmaker/")
# CI's own files that every light run already runs in full: offline-tested
# scripts (the publishing gate, the watchdog, this rule and its tests), the
# two checks mcp-renderer always runs and the copied-code baseline, and the
# workflows a CI run never runs. Not ci.yml itself, whose build and test
# steps only a full run tries, nor cut_corpus.py or apt-install.sh, which
# only a full run uses.
LIGHT_CI = {
    ".github/ci/changes.py", ".github/ci/test_changes.py",
    ".github/ci/test_draft_first.py", ".github/ci/test_cut_corpus.py",
    ".github/ci/reverts.py", ".github/ci/test_reverts.py",
    ".github/ci/literal_ratchet.py", ".github/ci/test_literal_ratchet.py",
    ".github/ci/literal_baseline.json",
    ".github/ci/dead_pub.py", ".github/ci/test_dead_pub.py",
    ".github/ci/dead_pub_baseline.json",
    ".github/workflows/ci-watchdog.yml", ".github/workflows/holdout.yml",
}
LIGHT_CI_FOLDERS = (".github/watchdog/", ".github/scripts/")
# Crates a pull request may test on their own: (packages to build and test,
# corpus checks to run). The MCP server's tests run the CLI program this
# build makes (TIMEGRAPHER_MCP_TEST_CLI), so its package is built with them.
NARROW = {
    "crates/timegrapher-app/": (("timegrapher-app",), ("app",)),
    "crates/timegrapher-mcp/": (("timegrapher-mcp", "timegrapher-cli"), ("mcp",)),
}
CHECKS = ("engine", "app", "mcp")
# Files of a narrow crate that another crate's code or tests read: a change
# to one tests the whole workspace. Core's chart_scale.rs reads the app's
# marks cases (the MCP renderer, which always runs, reads them too).
CROSS_READ = {"crates/timegrapher-app/src/marks_cases.json"}
# In a narrow crate, these change how every package builds or what it links.
CRATE_BUILD = ("Cargo.toml", "build.rs")


def light(changes):
    """True when every (status, path) change spares the build and tests."""
    return bool(changes) and all(spared(status, path) for status, path in changes)


def spared(status, path):
    """True when a light run's checks are all this change needs."""
    if status not in {"M", "A"}:
        return False
    if asset(path) or path in LIGHT_CI or path.startswith(LIGHT_CI_FOLDERS):
        return True
    return path.endswith(".md") and path not in MCP_DOCS and not path.startswith(FULL_PREFIXES)


def narrow(changes):
    """(packages, corpus checks) a pull request that is not light needs,
    or None for every package and check."""
    packages, checks = set(), set()
    for status, path in changes:
        if spared(status, path):
            continue
        crate = next((c for c in NARROW if path.startswith(c)), None)
        if crate is None or path in CROSS_READ or path[len(crate):] in CRATE_BUILD:
            return None
        packages.update(NARROW[crate][0])
        checks.update(NARROW[crate][1])
    return (tuple(sorted(packages)), tuple(c for c in CHECKS if c in checks)) if packages else None


def asset(path):
    """True for a guide asset in LIGHT_ASSETS: in its folder, not below it."""
    return any(path.startswith(folder) and "/" not in path[len(folder):]
               and path.endswith(ends) for folder, ends in LIGHT_ASSETS)


def pages(changes):
    """True when a (status, path) change touches a page the downloads carry."""
    return any(path in PAGE_FILES or path.startswith(PAGE_FOLDERS) for _, path in changes)


def changed(base, head):
    out = subprocess.check_output(
        ["git", "diff", "--no-renames", "--name-status", base, head], text=True)
    rows = []
    for line in out.splitlines():
        status, _, path = line.partition("\t")
        rows.append((status[:1], path))
    return rows


def plan(event, changes, check=None):
    """The lines changes.py prints, as {name: value}."""
    full = event != "pull_request" or not light(changes)
    scope = narrow(changes) if full and event == "pull_request" else None
    if check is not None and scope is not None:
        full = check in scope[1]
    packages = " ".join(f"-p {p}" for p in scope[0]) if scope else ""
    return {"full": "true" if full else "false",
            "pages": "true" if pages(changes) else "false",
            "packages": packages}


def main(argv):
    args = argv[1:]
    check = None
    if len(args) == 5 and args[3] == "--regress" and args[4] in CHECKS:
        check = args[4]
        args = args[:3]
    if len(args) != 3:
        raise SystemExit(__doc__)
    event, base, head = args
    changes = changed(base, head) if event == "pull_request" else []
    for name, value in plan(event, changes, check).items():
        print(f"{name}={value}")


if __name__ == "__main__":
    main(sys.argv)
