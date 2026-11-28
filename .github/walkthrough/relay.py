"""Runs on a Linux runner beside the walk job: pushes each walkthrough answer
the walk job uploads as an artifact to walk/out/ on the channel branch.

  relay.py CHANNEL_DIR

This job alone holds the token that can push (WALK_TOKEN); the app under test
never runs here, and git runs without hooks. It stops once the walk job has
ended and every answer it uploaded is pushed. Needs GH_TOKEN (to read the
run's jobs and artifacts), GITHUB_REPOSITORY, GITHUB_RUN_ID, WALK_CHANNEL.
"""
import base64
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from answers import new_answers, unzip_to  # noqa: E402

CHANNEL = sys.argv[1]
BRANCH = os.environ["WALK_CHANNEL"]
TOKEN = os.environ.pop("WALK_TOKEN")
RUN = "repos/%s/actions/runs/%s" % (os.environ["GITHUB_REPOSITORY"], os.environ["GITHUB_RUN_ID"])
BASIC = base64.b64encode(("x-access-token:" + TOKEN).encode()).decode()
AUTH = ["-c", "http.extraheader=AUTHORIZATION: basic " + BASIC]
NO_HOOKS = ["-c", "core.hooksPath=" + os.devnull]
OUT = os.path.join(CHANNEL, "walk", "out")


def git(*args):
    return subprocess.run(["git", "-C", CHANNEL] + NO_HOOKS + AUTH + list(args),
                          capture_output=True, text=True)


def api(path, raw=False):
    done = subprocess.run(["gh", "api", path] + ([] if raw else ["--paginate", "--slurp"]),
                          capture_output=True)
    if done.returncode != 0:
        raise RuntimeError(done.stderr.decode(errors="replace"))
    return done.stdout if raw else json.loads(done.stdout)


def walk_ended():
    jobs = [j for page in api(RUN + "/jobs?per_page=100") for j in page["jobs"]]
    walk = [j for j in jobs if j["name"] == "walk"]
    return bool(walk) and walk[0]["status"] == "completed"


def push(step):
    git("add", "walk/out")
    git("commit", "-qm", "walkthrough answer " + step)
    for _ in range(5):
        git("pull", "-q", "--rebase", "origin", BRANCH)
        if git("push", "-q", "origin", "HEAD:" + BRANCH).returncode == 0:
            return
        time.sleep(2)
    print("could not push", step, flush=True)


def main():
    git("config", "user.name", "windows-walkthrough")
    git("config", "user.email", "windows-walkthrough@users.noreply.github.com")
    os.makedirs(OUT, exist_ok=True)
    seen, ended = set(), False
    while True:
        try:
            ended = walk_ended()
            artifacts = [a for page in api(RUN + "/artifacts?per_page=100") for a in page["artifacts"]]
            for artifact_id, step in new_answers(artifacts, seen):
                names = unzip_to(api("repos/%s/actions/artifacts/%d/zip" % (
                    os.environ["GITHUB_REPOSITORY"], artifact_id), raw=True), OUT)
                seen.add(artifact_id)
                push(step)
                print("relayed", step, names, flush=True)
        except (RuntimeError, ValueError) as e:  # a failed read is tried again
            print("relay:", e, flush=True)
            ended = False
        # One more sweep after the walk ends catches its last answer.
        if ended:
            return
        time.sleep(3)


if __name__ == "__main__":
    main()
