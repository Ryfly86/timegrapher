"""Offline tests of ci.yml's draft rule (#2444): a draft pull request runs
only the quick job, and every other job runs only off a draft. Reads the
workflow as text (no YAML library on every runner); no network or git."""

import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
FULL = "${{ needs.mode.outputs.full == 'true' }}"


def jobs():
    """{job id: its text} for each job under `jobs:`."""
    body = WORKFLOW.split("\njobs:\n", 1)[1]
    parts = re.split(r"(?m)^  ([\w-]+):\n", body)
    return dict(zip(parts[1::2], parts[2::2]))


def job_if(text):
    found = re.search(r"(?m)^    if: (.*)$", text)
    return found.group(1) if found else None


class DraftRule(unittest.TestCase):
    def test_the_draft_state_events_start_a_run(self):
        self.assertIn("types: [opened, synchronize, reopened, ready_for_review]\n", WORKFLOW)
        # A run on converted_to_draft would skip the required checks on a
        # head that may already hold their real results.
        self.assertNotIn("converted_to_draft]", WORKFLOW)

    def test_every_job_but_mode_and_quick_runs_only_off_a_draft(self):
        self.assertIn(f"if: &full-suite {FULL}", WORKFLOW)
        found = jobs()
        self.assertIn("quick", found)
        summaries = {"test-linux-result": "test-linux", "test-macos-result": "test-macos"}
        for name, text in found.items():
            if name in ("mode", "quick"):
                continue
            rule = job_if(text)
            if name in summaries:
                # Its halves ran off a draft, so it runs only when they did:
                # run there, it would fail on their "skipped".
                self.assertEqual(rule, f"${{{{ !cancelled() && needs.{summaries[name]}.result != 'skipped' }}}}")
                continue
            # A job with no `if`, or one that reads the event's draft
            # state, would run on a draft.
            self.assertIn(rule, ("*full-suite", f"&full-suite {FULL}"), name)
            self.assertRegex(text, r"(?m)^    needs: mode$", name)

    def test_mode_reads_the_pull_request_live(self):
        # The event's draft state and head can be a few seconds old (#2368):
        # mode reads both from the API and runs nothing for an older head.
        text = jobs()["mode"]
        self.assertIsNone(job_if(text))
        self.assertNotRegex(text, r"(?m)^    needs: ")
        self.assertIn('gh api "$PR_URL" --jq \'"\\(.draft) \\(.head.sha)"\'', text)
        self.assertIn('if [ "$head" != "$EVENT_HEAD" ]; then', text)
        self.assertNotIn("github.event.pull_request.draft", WORKFLOW.replace(
            "EVENT_DRAFT: ${{ github.event.pull_request.draft }}", ""))

    def test_jobs_made_after_others_are_matrix_jobs(self):
        # A job with `needs:` is made only when what it needs ends. If it
        # were skipped under its own name on the draft run, that "skipped"
        # would stand for it, and read as passing, until then. A skipped
        # matrix job takes its unexpanded name instead. Only quick, which
        # is no required check, may skip under its own name.
        for name, text in jobs().items():
            if name != "quick" and re.search(r"(?m)^    needs: ", text):
                job_name = re.search(r"(?m)^    name: (.*)$", text).group(1)
                self.assertIn("${{ matrix.", job_name, name)
                self.assertRegex(text, r"(?m)^    strategy:\n(      .*\n)*?      matrix:\n", name)

    def test_quick_runs_only_on_a_draft_and_is_no_required_check(self):
        text = jobs()["quick"]
        # Exactly: `!draft` here would run it beside the full suite.
        self.assertEqual(job_if(text), "${{ needs.mode.outputs.quick == 'true' }}")
        name = re.search(r"(?m)^    name: (.*)$", text).group(1)
        gate = (ROOT / ".github/scripts/release_ci_gate.py").read_text(encoding="utf-8")
        self.assertNotIn(f'"{name}"', gate)
        self.assertFalse(name.startswith(("test (", "regress (", "mcp-renderer")), name)


def mode_script():
    """The `run: |` text of mode's "Draft or ready" step, unindented."""
    text = jobs()["mode"]
    block = text.split("        run: |\n", 1)[1]
    lines = []
    for line in block.split("\n"):
        if line and not line.startswith("          "):
            break
        lines.append(line[10:])
    return "\n".join(lines)


# (event, event draft, event head, what `gh api` prints or None if it fails,
#  expected output line)
MODE_CASES = [
    ("push", "", "", "false abc", "full=true quick=false"),
    ("workflow_dispatch", "", "", "true abc", "full=true quick=false"),
    # #2368: the ready_for_review event still named the old head.
    ("pull_request", "false", "old", "false new", "full=false quick=false"),
    # #2368: the push's synchronize event still said draft.
    ("pull_request", "true", "new", "false new", "full=true quick=false"),
    ("pull_request", "true", "abc", "true abc", "full=false quick=true"),
    ("pull_request", "true", "abc", None, "full=false quick=true"),
    ("pull_request", "false", "abc", None, "full=true quick=false"),
    # Marked ready, then back to draft before the run started.
    ("pull_request", "false", "abc", "true abc", "full=false quick=true"),
]


@unittest.skipIf(os.name == "nt" or not shutil.which("bash"), "needs bash")
class ModeDecides(unittest.TestCase):
    """Runs mode's own script from ci.yml with a fake `gh` on PATH."""

    def run_mode(self, event, draft, head, live):
        with tempfile.TemporaryDirectory() as tmp:
            gh = pathlib.Path(tmp, "gh")
            gh.write_text("#!/bin/sh\n" + ("exit 1\n" if live is None else f"echo '{live}'\n"))
            gh.chmod(0o755)
            out = pathlib.Path(tmp, "out")
            env = dict(os.environ, PATH=f"{tmp}{os.pathsep}{os.environ['PATH']}",
                       GITHUB_OUTPUT=str(out), EVENT=event, EVENT_DRAFT=draft,
                       EVENT_HEAD=head, PR_URL="repos/o/r/pulls/1", GH_TOKEN="x")
            subprocess.run(["bash", "-c", mode_script()], env=env, check=True,
                           capture_output=True, text=True)
            return " ".join(out.read_text().split())

    def test_the_eight_cases(self):
        for event, draft, head, live, want in MODE_CASES:
            with self.subTest(event=event, draft=draft, head=head, live=live):
                self.assertEqual(self.run_mode(event, draft, head, live), want)


if __name__ == "__main__":
    unittest.main()
