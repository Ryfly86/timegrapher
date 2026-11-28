"""Offline tests of the Windows walkthrough's answer relay (#2426): the walk
job, where the app under test runs, holds no token that can push; answers
travel as run artifacts to the relay job, which pushes them. No network."""

import base64
import hashlib
import io
import json
import pathlib
import re
import sys
import tempfile
import unittest
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / ".github/walkthrough"))
import answers  # noqa: E402

WORKFLOW = (ROOT / ".github/workflows/windows-walkthrough.yml").read_text(encoding="utf-8")


def token(claims):
    part = base64.urlsafe_b64encode(json.dumps(claims).encode()).decode().rstrip("=")
    return "e30." + part + ".sig"


def jobs():
    body = WORKFLOW.split("\njobs:\n", 1)[1]
    parts = re.split(r"(?m)^  ([\w-]+):\n", body)
    return dict(zip(parts[1::2], parts[2::2]))


class BackendIds(unittest.TestCase):
    def test_the_results_scope_names_run_and_job(self):
        t = token({"scp": "Actions.ExampleScope Actions.Results:run-1:job-2"})
        self.assertEqual(answers.backend_ids(t), ("run-1", "job-2"))

    def test_no_scope_or_no_token_is_an_error(self):
        for bad in (token({"scp": "Actions.Other"}), token({}), "nonsense"):
            with self.assertRaises(ValueError):
                answers.backend_ids(bad)


class Zips(unittest.TestCase):
    def test_answer_files_round_trip(self):
        data = answers.zip_files([("007.png", b"\x89PNG"), ("007.txt", b"app: head running\n")])
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(answers.unzip_to(data, d), ["007.png", "007.txt"])
            self.assertEqual(pathlib.Path(d, "007.txt").read_bytes(), b"app: head running\n")

    def test_only_top_level_files_are_written(self):
        buf = io.BytesIO()
        with zipfile.ZipFile(buf, "w") as z:
            z.writestr("../escape.txt", b"x")
            z.writestr("sub/inner.txt", b"x")
            z.writestr("ok.txt", b"x")
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(answers.unzip_to(buf.getvalue(), d), ["ok.txt"])
            self.assertEqual(sorted(p.name for p in pathlib.Path(d).iterdir()), ["ok.txt"])


class NewAnswers(unittest.TestCase):
    def test_unseen_answers_oldest_first(self):
        listed = [
            {"id": 3, "name": "walk-answer-002", "created_at": "2026-11-28T02:00:20Z"},
            {"id": 1, "name": "walk-apps", "created_at": "2026-11-28T01:50:00Z"},
            {"id": 2, "name": "walk-answer-ready-9", "created_at": "2026-11-28T02:00:00Z"},
            {"id": 4, "name": "walk-answer-003", "created_at": "2026-11-28T02:00:40Z", "expired": True},
        ]
        self.assertEqual(answers.new_answers(listed, set()), [(2, "ready-9"), (3, "002")])
        self.assertEqual(answers.new_answers(listed, {2}), [(3, "002")])


class FakeResponse(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False


class Upload(unittest.TestCase):
    def test_create_put_finalize(self):
        sent = []

        def opener(req, timeout):
            sent.append(req)
            if req.full_url.endswith("CreateArtifact"):
                return FakeResponse(json.dumps({"ok": True, "signed_upload_url": "https://blob/x?sig"}).encode())
            if req.full_url.endswith("FinalizeArtifact"):
                return FakeResponse(json.dumps({"ok": True, "artifact_id": "9"}).encode())
            return FakeResponse(b"")

        t = token({"scp": "Actions.Results:R:J"})
        answers.upload("walk-answer-001", [("001.txt", b"hi\n")], t, "https://results.example/", opener)
        self.assertEqual([r.full_url for r in sent], [
            "https://results.example/" + answers.SERVICE + "CreateArtifact",
            "https://blob/x?sig",
            "https://results.example/" + answers.SERVICE + "FinalizeArtifact",
        ])
        create = json.loads(sent[0].data)
        self.assertEqual(create, {"workflow_run_backend_id": "R", "workflow_job_run_backend_id": "J",
                                  "name": "walk-answer-001", "version": 4})
        self.assertEqual(sent[0].get_header("Authorization"), "Bearer " + t)
        put = sent[1]
        self.assertEqual(put.get_method(), "PUT")
        self.assertEqual(put.get_header("X-ms-blob-type"), "BlockBlob")
        self.assertIsNone(put.get_header("Authorization"))
        final = json.loads(sent[2].data)
        self.assertEqual(final["size"], str(len(put.data)))
        self.assertEqual(final["hash"], "sha256:" + hashlib.sha256(put.data).hexdigest())

    def test_a_refusal_stops_the_upload(self):
        def opener(req, timeout):
            return FakeResponse(json.dumps({"ok": False}).encode())

        with self.assertRaises(RuntimeError):
            answers.upload("walk-answer-001", [], token({"scp": "Actions.Results:R:J"}),
                           "https://results.example", opener)


class Workflow(unittest.TestCase):
    def test_only_the_relay_can_push(self):
        found = jobs()
        self.assertEqual({n for n, t in found.items() if "contents: write" in t}, {"relay"})
        self.assertIn("\npermissions:\n  contents: read\n", WORKFLOW)
        # The walk job never sees the workflow token.
        self.assertNotIn("github.token", found["walk"])

    def test_the_relay_runs_no_code_from_the_branches_walked(self):
        relay = jobs()["relay"]
        self.assertNotIn("inputs.ref", relay)
        self.assertNotIn("inputs.base_ref", relay)
        self.assertNotIn("walk-apps", relay)
        self.assertIn("run: python3 tool/.github/walkthrough/relay.py channel\n", relay)
        # Its scripts come from the workflow's own commit, not the channel.
        self.assertIn("sparse-checkout: .github/walkthrough\n", relay)

    def test_git_never_runs_hooks_where_it_can_push(self):
        # The channel's content is the agent's; a hook in it must not run
        # beside the push token.
        relay = (ROOT / ".github/walkthrough/relay.py").read_text(encoding="utf-8")
        self.assertIn('NO_HOOKS = ["-c", "core.hooksPath=" + os.devnull]\n', relay)
        git = relay.split("def git(", 1)[1].split("\ndef ", 1)[0]
        self.assertIn('["git", "-C", CHANNEL] + NO_HOOKS + AUTH', git)
        self.assertEqual(relay.count("subprocess.run([\"git\""), 1)

    def test_the_docs_say_to_merge_main_into_the_channel(self):
        readme = (ROOT / ".github/walkthrough/README.md").read_text(encoding="utf-8")
        self.assertIn("Merge main into it first, every time", readme)
        self.assertIn("with\n# main merged into the channel first", WORKFLOW)

    def test_a_run_must_come_from_the_channel_branch(self):
        build = jobs()["build"]
        self.assertIn("        if: github.ref != format('refs/heads/{0}', inputs.channel) "
                      "|| github.ref == 'refs/heads/main'\n", build)
        # It is the build job's first step, before any code is checked out.
        self.assertLess(build.index("Run from the channel branch"), build.index("uses: actions/checkout"))

    def test_the_driver_keeps_the_artifact_token_from_the_app(self):
        driver = (ROOT / ".github/walkthrough/driver.py").read_text(encoding="utf-8")
        pops = driver.index('os.environ.pop("WALK_RUNTIME_TOKEN")')
        self.assertLess(pops, driver.index("APP_ENV = dict(os.environ)"))
        self.assertNotIn("WALK_TOKEN", driver)


if __name__ == "__main__":
    unittest.main()
