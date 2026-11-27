"""Offline tests of CI's light-run and narrow-run rules. Reads the tree; no network or git."""

import pathlib
import re
import sys
import tempfile
import unittest

import changes


ROOT = pathlib.Path(__file__).resolve().parents[2]


class Light(unittest.TestCase):
    def test_edited_or_added_plain_markdown_is_light(self):
        self.assertTrue(changes.light([("M", "docs/app.md"), ("A", "docs/new-page.md"),
                                       ("M", "README.md"), ("M", "fuzz/README.md")]))

    def test_code_or_any_other_file_is_full(self):
        for path in ("crates/timegrapher-core/src/lib.rs", "Cargo.lock", "regress/baseline.json",
                     ".github/workflows/ci.yml", "guide-figures/make.py", "docs/assets/x.png",
                     "guide-figures/data/quick-listen/clip.flac", "guide-figures/README.toml",
                     "docs/user-guide/images/notes.json", "docs/user-guide/images/old/x.png",
                     "docs/user-guide/x.png", "guide-figures/figures/x.py"):
            self.assertFalse(changes.light([("M", "docs/app.md"), ("M", path)]), path)

    def test_a_fixture_two_other_crates_read_is_full(self):
        # The app keeps marks_cases.json, but core's chart_scale.rs tests and
        # the MCP renderer's marks_cases.test.cjs read it too: a rule that
        # ran only the app's checks for an app file would miss them.
        self.assertFalse(changes.light([("M", "crates/timegrapher-app/src/marks_cases.json")]))

    def test_guide_screenshots_and_figure_descriptions_are_light(self):
        # #2356: three guide pages, a screenshot and its figure description
        # ran the whole suite (70 minutes, 182 runner-minutes).
        pr_2356 = [("M", "docs/user-guide/08-charts-and-distributions.md"),
                   ("M", "docs/user-guide/20-positions-test.md"),
                   ("M", "docs/user-guide/22-watches-view.md"),
                   ("M", "docs/user-guide/images/distributions.png"),
                   ("M", "guide-figures/figures/distributions.toml")]
        self.assertTrue(changes.light(pr_2356))
        self.assertTrue(changes.light([("A", "docs/user-guide/images/new.jpg"),
                                       ("A", "docs/user-guide/images/new.svg"),
                                       ("A", "guide-figures/figures/new.toml")]))
        # The screenshots are pages the downloads carry; the descriptions are not.
        self.assertTrue(changes.pages(pr_2356[3:4]))
        self.assertFalse(changes.pages(pr_2356[4:]))

    def test_deleting_or_renaming_a_guide_asset_is_full(self):
        # packaging/test_docs_html.py lists the guide's images, and a page
        # may still show a deleted one.
        self.assertFalse(changes.light([("D", "docs/user-guide/images/scope.png")]))
        self.assertFalse(changes.light([("D", "guide-figures/figures/scope.toml"),
                                        ("A", "guide-figures/figures/scope2.toml")]))

    def test_compiled_in_markdown_is_full(self):
        for path in sorted(changes.MCP_DOCS) + ["skills/watchmaker/SKILL.md",
                                                 "skills/watchmaker/references/long-runs.md",
                                                 "packaging/macos/start-here.md",
                                                 "crates/timegrapher-core/data/README.md"]:
            self.assertFalse(changes.light([("M", path)]), path)

    def test_deleting_or_renaming_markdown_is_full(self):
        # packaging/test_docs_html.py checks that the pages it maps exist.
        self.assertFalse(changes.light([("D", "docs/app.md")]))
        self.assertFalse(changes.light([("D", "docs/app.md"), ("A", "docs/application.md")]))

    def test_no_changes_is_full(self):
        self.assertFalse(changes.light([]))


class LightCI(unittest.TestCase):
    """CI's own files that every light run already runs in full."""

    def test_ci_scripts_and_their_tests_are_light(self):
        self.assertTrue(changes.light([("M", ".github/ci/changes.py"),
                                       ("M", ".github/ci/test_changes.py"),
                                       ("M", ".github/watchdog/ci_watchdog.py"),
                                       ("A", ".github/scripts/test_release_ci_gate.py"),
                                       ("M", ".github/workflows/ci-watchdog.yml")]))

    def test_what_only_a_full_run_tries_is_full(self):
        for path in (".github/workflows/ci.yml", ".github/workflows/release.yml",
                     ".github/ci/cut_corpus.py", ".github/ci/apt-install.sh",
                     ".github/ISSUE_TEMPLATE/bug.yml"):
            self.assertFalse(changes.light([("M", path)]), path)
        self.assertFalse(changes.light([("D", ".github/ci/test_reverts.py")]))

    def test_each_light_ci_file_exists(self):
        for path in changes.LIGHT_CI:
            self.assertTrue((ROOT / path).is_file(), path)
        for folder in changes.LIGHT_CI_FOLDERS:
            self.assertTrue((ROOT / folder).is_dir(), folder)

    def test_a_light_ci_file_is_not_used_by_a_full_only_step(self):
        # ci.yml's steps that run only in a full run say so in their `if`;
        # a light CI file they ran would go untried in a light run.
        workflow = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        for step in re.split(r"(?m)^      - ", workflow):
            if "outputs.full != 'false'" in step:
                for path in changes.LIGHT_CI:
                    self.assertNotIn(path, step)


class Narrow(unittest.TestCase):
    APP = "crates/timegrapher-app/src/app/menus.rs"
    MCP = "crates/timegrapher-mcp/src/tools.rs"

    def test_an_app_change_tests_the_app_alone(self):
        self.assertEqual(changes.narrow([("M", self.APP), ("A", "crates/timegrapher-app/src/new.rs"),
                                         ("D", "crates/timegrapher-app/src/old.rs"),
                                         ("M", "docs/app.md")]),
                         (("timegrapher-app",), ("app",)))

    def test_an_mcp_change_builds_the_cli_its_tests_run(self):
        self.assertEqual(changes.narrow([("M", self.MCP)]),
                         (("timegrapher-cli", "timegrapher-mcp"), ("mcp",)))

    def test_both_crates_test_both(self):
        self.assertEqual(changes.narrow([("M", self.APP), ("M", self.MCP)]),
                         (("timegrapher-app", "timegrapher-cli", "timegrapher-mcp"), ("app", "mcp")))

    def test_anything_else_tests_everything(self):
        for path in ("crates/timegrapher-core/src/lib.rs", "crates/timegrapher-cli/src/main.rs",
                     "crates/timegrapher-app/Cargo.toml", "crates/timegrapher-app/build.rs",
                     "crates/timegrapher-mcp/Cargo.toml", "crates/build_info.rs",
                     "crates/timegrapher-app/src/marks_cases.json", "Cargo.lock", "Cargo.toml",
                     "regress/baseline.json", ".github/workflows/ci.yml", ".config/nextest.toml",
                     "packaging/icon.png", ".github/ISSUE_TEMPLATE/bug.yml", "fuzz/Cargo.toml",
                     "docs/reference/series.md"):
            self.assertIsNone(changes.narrow([("M", self.APP), ("M", path)]), path)
        self.assertIsNone(changes.narrow([]))

    def test_plan_prints_packages_and_each_regress_check(self):
        app = [("M", self.APP)]
        self.assertEqual(changes.plan("pull_request", app),
                         {"full": "true", "pages": "false", "packages": "-p timegrapher-app"})
        self.assertEqual(changes.plan("pull_request", app, "app")["full"], "true")
        self.assertEqual(changes.plan("pull_request", app, "engine")["full"], "false")
        self.assertEqual(changes.plan("pull_request", app, "mcp")["full"], "false")
        core = [("M", "crates/timegrapher-core/src/lib.rs")]
        for check in changes.CHECKS:
            self.assertEqual(changes.plan("pull_request", core, check)["full"], "true")
        self.assertEqual(changes.plan("pull_request", core)["packages"], "")
        light = [("M", "docs/app.md")]
        self.assertEqual(changes.plan("pull_request", light, "app")["full"], "false")
        # Main and runs by hand test everything.
        for event in ("push", "workflow_dispatch"):
            self.assertEqual(changes.plan(event, [], "mcp"),
                             {"full": "true", "pages": "false", "packages": ""})

    def test_narrow_crates_are_workspace_members_and_never_core_or_cli(self):
        members = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        for crate, (packages, checks) in changes.NARROW.items():
            self.assertIn(f'"{crate.rstrip("/")}"', members)
            self.assertTrue(set(checks) <= set(changes.CHECKS))
            for package in packages:
                self.assertTrue((ROOT / "crates" / package / "Cargo.toml").is_file(), package)
        self.assertFalse({"crates/timegrapher-core/", "crates/timegrapher-cli/"} & set(changes.NARROW))

    def test_every_file_another_crate_reads_in_a_narrow_crate_is_full(self):
        # A literal path into another crate's folder (`../timegrapher-app/...`)
        # in any crate's Rust or JavaScript. A narrow run tests only the
        # changed crate, so a file another crate reads must be in CROSS_READ.
        found = set()
        for source in (ROOT / "crates").rglob("*"):
            if source.suffix not in {".rs", ".cjs", ".js"} or "target" in source.parts:
                continue
            reader = source.relative_to(ROOT).parts[1]
            text = source.read_text(encoding="utf-8", errors="replace")
            for crate, rest in re.findall(r"\.\./(timegrapher-[a-z]+)/([^\"'\s)]*)", text):
                if crate != reader and f"crates/{crate}/" in changes.NARROW:
                    found.add((source.relative_to(ROOT).as_posix(), f"crates/{crate}/{rest}"))
        self.assertTrue(found)
        for source, path in sorted(found):
            self.assertIn(path, changes.CROSS_READ, f"{source} reads {path}")


class Pages(unittest.TestCase):
    def test_a_page_the_downloads_carry_is_checked(self):
        for path in ("README.md", "LICENSE", "docs/app.md", "docs/assets/x.png",
                     "skills/watchmaker/SKILL.md", "skills/watchmaker/references/long-runs.md"):
            self.assertTrue(changes.pages([("M", "Cargo.lock"), ("M", path)]), path)
        self.assertTrue(changes.pages([("D", "docs/app.md")]))

    def test_other_files_are_not(self):
        self.assertFalse(changes.pages([("M", "crates/timegrapher-core/src/lib.rs"),
                                        ("M", "fuzz/README.md"), ("M", "docs.md")]))
        self.assertFalse(changes.pages([]))

    def test_every_file_docs_html_renders_is_covered(self):
        sys.path.insert(0, str(ROOT / "packaging"))
        try:
            import docs_html
        finally:
            sys.path.pop(0)
        for platform in docs_html.PLATFORMS:
            # Release's own pull-request run renders the pages for packaging/.
            for path in docs_html.outputs(platform):
                if not path.startswith("packaging/"):
                    self.assertTrue(changes.pages([("M", path)]), path)


class GuideFigures(unittest.TestCase):
    """What a light run checks of a figure's description, which only
    guide-figures/make.py reads, by hand."""

    def test_each_figure_parses_and_names_a_screenshot_that_exists(self):
        import tomllib
        figures = sorted((ROOT / "guide-figures/figures").glob("*.toml"))
        self.assertGreater(len(figures), 30)
        for figure in figures:
            with self.subTest(figure=figure.name):
                with open(figure, "rb") as f:
                    spec = tomllib.load(f)
                self.assertTrue((ROOT / "docs/user-guide/images" / spec["output"]).is_file())


class Tree(unittest.TestCase):
    """The rule must keep covering every Markdown file the code builds in or
    the tests read.

    These scan for Markdown paths written out as string literals: in any Rust
    source under crates/ (include_str!, include_bytes!, a doc!-style macro or a
    file a test opens at run time), in the MCP renderer's JavaScript tests,
    and in packaging's and .github's Python, except this folder's own files,
    whose literals are the rule and its test cases. They cannot see a path
    that code assembles from pieces at run time, or a whole folder listed at
    run time; nothing in CI's tests reads Markdown that way today
    (packaging's docs_html.py lists docs/ only to map page names, whose files'
    existence its tests check, so deleting or renaming any file is a full
    run).
    """

    # Literals that name a Markdown file without reading the repository's copy.
    # An entry covers every literal of that path in its file, so a real read
    # of the same path added there later would be missed: remove the entry.
    NOT_READ = {
        # Writes a new README.md into the synthetic take's folder.
        ("crates/timegrapher-cli/src/synth_cmd.rs", "README.md"),
        # Page names and existence checks; the text is read only when the
        # Release workflow renders the pages.
        ("packaging/docs_html.py", "docs/reference/install.md"),
        ("packaging/docs_html.py", "docs/reference/mcp.md"),
        ("packaging/docs_html.py", "docs/reference/agent-interface.md"),
        ("packaging/test_docs_html.py", "README.md"),
    }

    def sources(self):
        paths = [path for path in (ROOT / "crates").rglob("*")
                 if path.suffix in {".rs", ".cjs", ".js", ".py"} and "target" not in path.parts]
        paths += (ROOT / "packaging").rglob("*.py")
        paths += [path for path in (ROOT / ".github").rglob("*.py")
                  if path.parent != pathlib.Path(__file__).resolve().parent]
        return paths

    def named_markdown(self, sources=None):
        """(source, repository path) for each literal naming a Markdown file here."""
        found = set()
        for source in self.sources() if sources is None else sources:
            text = source.read_text(encoding="utf-8", errors="replace")
            for literal in re.findall(r"[\"']([^\"'\s]*\.md)[\"']", text):
                for candidate in (ROOT / literal.lstrip("/"), source.parent / literal,
                                  ROOT / literal.split("../../")[-1].lstrip("/")):
                    candidate = candidate.resolve()
                    if candidate.is_file() and candidate.is_relative_to(ROOT):
                        found.add((source.name if sources else source.relative_to(ROOT).as_posix(),
                                   candidate.relative_to(ROOT).as_posix()))
                        break
        return found

    def test_mcp_docs_match_guidance(self):
        guidance = (ROOT / "crates/timegrapher-mcp/src/guidance.rs").read_text(encoding="utf-8")
        compiled = set(re.findall(r'"(docs/[^"]+\.md)"', guidance))
        self.assertEqual(compiled, changes.MCP_DOCS)

    def test_every_markdown_file_the_code_names_is_full(self):
        found = self.named_markdown()
        # guidance.rs alone names the four docs, the skill and its references.
        self.assertGreater(len(found), 15)
        for source, path in sorted(found - self.NOT_READ):
            self.assertFalse(changes.light([("M", path)]), f"{source} names {path}")

    def test_no_code_or_test_names_a_guide_asset(self):
        # Neither a folder in LIGHT_ASSETS nor a file in one, so a light run
        # of a guide asset cannot skip a test that reads it. packaging's
        # docs_html.py renders the screenshots, which the pages check covers;
        # its test lists the images folder by pieces (GUIDE / "images"), not
        # by this path.
        folders = [folder for folder, _ in changes.LIGHT_ASSETS]
        names = [path.name for folder in folders for path in (ROOT / folder).iterdir()]
        self.assertGreater(len(names), 60)
        for source in self.sources():
            text = source.read_text(encoding="utf-8", errors="replace")
            where = source.relative_to(ROOT).as_posix()
            for folder in folders:
                self.assertNotIn(folder.rstrip("/"), text, where)
            for name in names:
                self.assertIsNone(re.search(r"[\"'/]" + re.escape(name) + r"[\"']", text),
                                  f"{where} names {name}")

    def test_the_not_read_list_is_current(self):
        self.assertLessEqual(self.NOT_READ, self.named_markdown())

    def test_the_scan_sees_each_kind_of_reference(self):
        forms = {"a.rs": 'include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reference/app.md"))',
                 "b.rs": 'const ICON: &[u8] = include_bytes!("/docs/reference/app.md");',
                 "c.rs": 'doc!("timegrapher://docs/app", "app", "App", "x", "docs/reference/app.md")',
                 "d.rs": 'let text = fs::read_to_string("docs/reference/app.md").unwrap();',
                 "e.cjs": "const text = fs.readFileSync('docs/reference/app.md', 'utf8');",
                 "f.py": 'TEXT = (ROOT / "docs/reference/app.md").read_text()'}
        with tempfile.TemporaryDirectory() as tmp:
            sources = []
            for name, line in forms.items():
                (pathlib.Path(tmp) / name).write_text(line + "\n", encoding="utf-8")
                sources.append(pathlib.Path(tmp) / name)
            found = self.named_markdown(sources)
        self.assertEqual(found, {(name, "docs/reference/app.md") for name in forms})
        # docs/reference/app.md is plain Markdown, so a real line like these would fail
        # test_every_markdown_file_the_code_names_is_full.
        self.assertTrue(changes.light([("M", "docs/reference/app.md")]))


if __name__ == "__main__":
    unittest.main()
