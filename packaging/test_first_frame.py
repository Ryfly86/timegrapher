"""Offline tests of first_frame.py: its reading of the app's log, and runs
against a stand-in app (a Python script), so no display or build is needed."""

from pathlib import Path
import sys
import tempfile
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import first_frame  # noqa: E402

DREW = "2026-11-28T01:00:00Z info  The window drew its first frame"
READ = "2026-11-28T01:00:01Z info  Analysed the whole recording. Drag the strip to look back."
BOTH = (f"log('info', {first_frame.FIRST_FRAME!r})\n"
        "log('info', 'Analysed the whole recording. Drag the strip to look back.')\n")


def fake_app(folder, body):
    """A stand-in app: Python running `body` with LOG the log's path."""
    script = Path(folder, "app.py")
    script.write_text(
        "import os, sys, time\n"
        "d = os.path.join(os.environ['TIMEGRAPHER_CONFIG_DIR'], 'logs')\n"
        "os.makedirs(d, exist_ok=True)\n"
        "LOG = os.path.join(d, 'timegrapher.log')\n"
        "def log(level, text):\n"
        "    open(LOG, 'a').write('2026-11-28T01:00:00Z %-5s %s\\n' % (level, text))\n"
        + body)
    return [sys.executable, str(script)]


class LogState(unittest.TestCase):
    def test_the_first_frame_line_and_errors(self):
        text = "\n".join([
            "2026-11-28T01:00:00Z info  timegrapher-app 1.1.1",
            DREW,
            READ,
            "2026-11-28T01:00:01Z warn  something odd",
            "2026-11-28T01:00:02Z error Can't read the recording",
            "",
            "short",
        ])
        drew, errors = first_frame.log_state(text)
        self.assertTrue(drew)
        self.assertEqual(errors, ["2026-11-28T01:00:02Z error Can't read the recording"])

    def test_drawn_but_not_yet_read(self):
        self.assertEqual(first_frame.log_state(DREW + "\n"), (False, []))
        self.assertEqual(first_frame.log_state(READ + "\n"), (False, []))

    def test_no_frame_yet(self):
        self.assertEqual(first_frame.log_state("2026-11-28T01:00:00Z info  Drawing with x\n"),
                         (False, []))
        # The words inside another line are no first frame.
        self.assertFalse(first_frame.log_state(
            "2026-11-28T01:00:00Z info  quoted: The window drew its first frame\n")[0])

    def test_crash_records_and_exit_are_problems(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(first_frame.problems(d, None), [])
            Path(d, "crash-2026-11-28T01-00-00Z.txt").write_text("boom")
            found = first_frame.problems(d, 101)
            self.assertEqual(found, ["the app exited with code 101",
                                     "crash record: crash-2026-11-28T01-00-00Z.txt"])


class Runs(unittest.TestCase):
    """Whole runs against a stand-in app; the screenshot may warn."""

    def check(self, body, timeout=10):
        with tempfile.TemporaryDirectory() as d:
            recording = Path(d, "take.flac")
            recording.write_bytes(b"")
            return first_frame.main([str(Path(d, "shot.png")), str(recording),
                                     *fake_app(d, body), "--timeout", str(timeout),
                                     "--settle", "1"])

    def test_an_app_that_draws_and_keeps_going_passes(self):
        self.assertEqual(self.check(BOTH + "time.sleep(60)\n"), 0)

    def test_an_app_that_draws_but_never_reads_fails(self):
        self.assertEqual(self.check(f"log('info', {first_frame.FIRST_FRAME!r})\n"
                                    "time.sleep(60)\n", timeout=2), 1)

    def test_an_app_that_never_draws_fails(self):
        self.assertEqual(self.check("time.sleep(60)\n", timeout=2), 1)

    def test_an_app_that_exits_before_drawing_fails(self):
        self.assertEqual(self.check("sys.exit(3)\n"), 1)

    def test_an_app_that_exits_after_drawing_fails(self):
        self.assertEqual(self.check(BOTH + "sys.exit(0)\n"), 1)

    def test_an_app_that_logs_an_error_fails(self):
        self.assertEqual(self.check(BOTH + "log('error', 'Can not read the recording')\n"
                                    "time.sleep(60)\n"), 1)

    def test_an_error_before_the_reading_fails_at_once(self):
        # As the app does on a file it can't read: it draws, logs the error
        # and never says it analysed the recording.
        start = time.monotonic()
        self.assertEqual(self.check(f"log('info', {first_frame.FIRST_FRAME!r})\n"
                                    "log('error', 'Can not read take.flac')\n"
                                    "time.sleep(60)\n", timeout=30), 1)
        self.assertLess(time.monotonic() - start, 20)

    def test_an_app_that_leaves_a_crash_record_fails(self):
        self.assertEqual(self.check(
            BOTH + "open(os.path.join(os.environ['TIMEGRAPHER_CONFIG_DIR'], "
            "'crash-2026-11-28T01-00-00Z.txt'), 'w').write('x')\n"
            "time.sleep(60)\n"), 1)


class TheAppsWords(unittest.TestCase):
    """The two log lines waited for are still the app's own."""

    def test_the_app_logs_them(self):
        app = Path(__file__).resolve().parents[1] / "crates/timegrapher-app/src"
        main = (app / "main.rs").read_text(encoding="utf-8")
        self.assertIn(f'applog::info("{first_frame.FIRST_FRAME}")', main)
        take_io = (app / "app/take_io.rs").read_text(encoding="utf-8")
        self.assertIn(f'"{first_frame.READ_DONE} ', take_io)


class InTheRelease(unittest.TestCase):
    """Every platform's build runs the check on a take that exists."""

    def test_each_platform_runs_it(self):
        root = Path(__file__).resolve().parents[1]
        text = (root / ".github/workflows/release.yml").read_text(encoding="utf-8")
        step = text.split("      - name: First frame\n", 1)[1].split("\n      - ", 1)[0]
        take = step.split("TAKE: ", 1)[1].split("\n", 1)[0]
        self.assertTrue((root / take).is_file(), take)
        for os_name in ("Linux)", "macOS)", "Windows)"):
            branch = step.split(os_name, 1)[1].split(";;", 1)[0]
            self.assertIn('packaging/first_frame.py "$SHOT" "$TAKE" ', branch, os_name)
        self.assertIn("xvfb-run", step)


if __name__ == "__main__":
    unittest.main()
