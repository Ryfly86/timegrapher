"""Open the packaged desktop app on a recording and check it draws (#2430).

  first_frame.py OUT_PNG FILE APP [ARG ...]

Runs APP [ARG ...] FILE with a settings folder of its own, waits for the
app's log to say the window drew its first frame and then that it analysed
the whole recording, lets it draw for --settle seconds, saves a screenshot
of the screen to OUT_PNG and closes the app. Fails if the app exits, hasn't
done both within --timeout seconds, logs an error or leaves a crash record. `--help` passes when the app can't open a
window, load its fonts or read a recording; this does not.

The screenshot is evidence for a person, not a check: if the platform's
screenshot tool fails, that is a warning. On Linux it needs an X display
(Xvfb) and ImageMagick's `import`; macOS uses `screencapture` and Windows
PowerShell, both built in. No hardware, network or release operations.
"""

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

# The line the app logs once it has built a frame (timegrapher-app main.rs),
# and words of the one once an opened recording is read (take_io.rs; a
# note can come before them).
FIRST_FRAME = "The window drew its first frame"
READ_DONE = "Analysed the whole recording."
LOG = Path("logs") / "timegrapher.log"


class FrameError(Exception):
    pass


def log_state(text):
    """(drew a frame and read the recording, the error lines) of the app's
    log text.

    Each line is "<UTC time> <level> <text>" (timegrapher-app applog.rs)."""
    drew = read_done = False
    errors = []
    for line in text.splitlines():
        parts = line.split(None, 2)
        if len(parts) < 3:
            continue
        level, message = parts[1], parts[2]
        if level == "error":
            errors.append(line)
        elif message == FIRST_FRAME:
            drew = True
        elif READ_DONE in message:
            read_done = True
    return drew and read_done, errors


def crash_records(folder):
    """Crash records the app wrote (applog.rs `crash`)."""
    return sorted(p.name for p in Path(folder).glob("crash-*.txt"))


def problems(config, proc_code):
    """What is wrong once the app has drawn and read: [] when nothing is."""
    found = []
    if proc_code is not None:
        found.append(f"the app exited with code {proc_code}")
    _, errors = log_state(read(Path(config) / LOG))
    found += [f"logged: {e}" for e in errors]
    found += [f"crash record: {c}" for c in crash_records(config)]
    return found


def read(path):
    try:
        return Path(path).read_text(encoding="utf-8", errors="replace")
    except FileNotFoundError:
        return ""


def screenshot(out):
    """Save the whole screen to `out`; the tool's complaint, or None."""
    if sys.platform == "darwin":
        cmd = ["screencapture", "-x", str(out)]
    elif os.name == "nt":
        script = (
            "Add-Type -AssemblyName System.Windows.Forms,System.Drawing;"
            "$b=[System.Windows.Forms.SystemInformation]::VirtualScreen;"
            "$i=New-Object System.Drawing.Bitmap $b.Width,$b.Height;"
            "$g=[System.Drawing.Graphics]::FromImage($i);"
            "$g.CopyFromScreen($b.Left,$b.Top,0,0,$i.Size);"
            f"$i.Save('{out}',[System.Drawing.Imaging.ImageFormat]::Png)"
        )
        cmd = ["powershell", "-NoProfile", "-Command", script]
    else:
        cmd = ["import", "-window", "root", str(out)]
    try:
        done = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as e:
        return str(e)
    if done.returncode != 0 or not Path(out).is_file():
        return (done.stderr or done.stdout or f"exit {done.returncode}").strip()
    return None


def tail(path, n=3000):
    return read(path)[-n:]


def run(out, recording, app, timeout, settle):
    with tempfile.TemporaryDirectory(prefix="tg-first-frame-") as config:
        env = dict(os.environ, TIMEGRAPHER_CONFIG_DIR=config)
        err = Path(config) / "stderr.txt"
        with open(err, "w") as stderr:
            proc = subprocess.Popen(list(app) + [str(recording)], env=env,
                                    stdout=subprocess.DEVNULL, stderr=stderr)
        try:
            start = time.monotonic()
            while True:
                done, errors = log_state(read(Path(config) / LOG))
                if done:
                    break
                if errors:
                    raise FrameError("\n".join(f"logged: {e}" for e in errors) + "\n" + tail(err))
                if proc.poll() is not None:
                    raise FrameError(f"the app exited with code {proc.returncode} before "
                                     f"drawing and reading\n{tail(err)}\n{tail(Path(config) / LOG)}")
                if time.monotonic() - start > timeout:
                    raise FrameError(f"not drawn and read within {timeout:.0f} s\n{tail(err)}\n"
                                     f"{tail(Path(config) / LOG)}")
                time.sleep(0.5)
            drew_at = time.monotonic() - start
            print(f"drawn and read after {drew_at:.1f} s; drawing for {settle:.0f} s more",
                  flush=True)
            end = time.monotonic() + settle
            while time.monotonic() < end and proc.poll() is None:
                time.sleep(0.5)
            complaint = screenshot(out)
            if complaint:
                print(f"::warning::no screenshot: {complaint}", flush=True)
            found = problems(config, proc.poll())
            print(tail(Path(config) / LOG), flush=True)
            if found:
                raise FrameError("\n".join(found) + "\n" + tail(err))
        finally:
            if proc.poll() is None:
                proc.kill()
            proc.wait()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("out", type=Path)
    parser.add_argument("recording", type=Path)
    parser.add_argument("app", nargs="+")
    parser.add_argument("--timeout", type=float, default=90)
    parser.add_argument("--settle", type=float, default=5)
    a = parser.parse_args(argv)
    if not a.recording.is_file():
        parser.error(f"no recording at {a.recording}")
    try:
        run(a.out.resolve(), a.recording.resolve(), a.app, a.timeout, a.settle)
    except FrameError as e:
        print(f"first frame check failed: {e}", file=sys.stderr)
        return 1
    print(f"drew and read {a.recording.name}; screenshot {a.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
