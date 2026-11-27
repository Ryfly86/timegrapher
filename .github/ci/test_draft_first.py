"""Offline tests of ci.yml's draft rule (#2444): a draft pull request runs
only the quick job, and every other job runs only off a draft. Reads the
workflow as text (no YAML library on every runner); no network or git."""

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
WORKFLOW = (ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
OFF_DRAFT = "github.event_name != 'pull_request' || !github.event.pull_request.draft"


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
        self.assertIn("types: [opened, synchronize, reopened, ready_for_review, converted_to_draft]",
                      WORKFLOW)

    def test_every_job_but_quick_runs_only_off_a_draft(self):
        self.assertIn(f"if: &full-suite ${{{{ {OFF_DRAFT} }}}}", WORKFLOW)
        found = jobs()
        self.assertIn("quick", found)
        for name, text in found.items():
            if name == "quick":
                continue
            rule = job_if(text)
            # A job with no `if` would run on a draft; a summary that ran
            # there would fail on its halves' "skipped".
            self.assertIsNotNone(rule, name)
            self.assertTrue(rule == "*full-suite" or OFF_DRAFT in rule or "&full-suite" in rule,
                            f"{name}: {rule}")

    def test_quick_runs_only_on_a_draft_and_is_no_required_check(self):
        text = jobs()["quick"]
        self.assertIn("github.event.pull_request.draft", job_if(text))
        self.assertIn("github.event.action != 'converted_to_draft'", job_if(text))
        name = re.search(r"(?m)^    name: (.*)$", text).group(1)
        gate = (ROOT / ".github/scripts/release_ci_gate.py").read_text(encoding="utf-8")
        self.assertNotIn(f'"{name}"', gate)
        self.assertFalse(name.startswith(("test (", "regress (", "mcp-renderer")), name)


if __name__ == "__main__":
    unittest.main()
