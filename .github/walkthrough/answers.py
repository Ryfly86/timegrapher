"""How a walkthrough answer travels from the Windows runner to the channel
branch without a write token on the machine running the app under test
(#2426).

The walk job uploads each answer (its screenshot and text) as an artifact of
the run named ANSWER_PREFIX + the step's name, with the run's own artifact
credentials. The relay job, on another runner, holds the token that can push:
it downloads each new answer artifact and pushes the files to walk/out/ on
the channel branch.

The upload speaks GitHub's artifact service (version 4, as
actions/upload-artifact does): CreateArtifact, a PUT of the zip to the
signed URL it returns, then FinalizeArtifact with the zip's size and
SHA-256. Standard library only.
"""

import base64
import hashlib
import io
import json
import os
import urllib.request
import zipfile

ANSWER_PREFIX = "walk-answer-"
SERVICE = "twirp/github.actions.results.api.v1.ArtifactService/"


def backend_ids(token):
    """(run, job) backend ids from the runtime token's `scp` claim, which
    holds a scope "Actions.Results:<run>:<job>"."""
    try:
        payload = token.split(".")[1]
        claims = json.loads(base64.urlsafe_b64decode(payload + "=" * (-len(payload) % 4)))
    except (IndexError, ValueError) as e:
        raise ValueError(f"not a runtime token: {e}") from None
    for scope in str(claims.get("scp", "")).split():
        parts = scope.split(":")
        if len(parts) == 3 and parts[0] == "Actions.Results":
            return parts[1], parts[2]
    raise ValueError("the runtime token names no Actions.Results scope")


def zip_files(files):
    """A zip holding each (name, bytes) pair at its top level."""
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
        for name, data in files:
            z.writestr(name, data)
    return buf.getvalue()


def unzip_to(data, folder):
    """Write a zip's files into `folder`, top level only; the names written."""
    names = []
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for info in z.infolist():
            name = os.path.basename(info.filename)
            if info.is_dir() or not name or name != info.filename:
                continue
            with open(os.path.join(folder, name), "wb") as f:
                f.write(z.read(info))
            names.append(name)
    return sorted(names)


def new_answers(artifacts, seen):
    """The answer artifacts not yet relayed, oldest first: (id, step name)."""
    found = [(a["created_at"], a["id"], a["name"][len(ANSWER_PREFIX):])
             for a in artifacts
             if a["name"].startswith(ANSWER_PREFIX) and a["id"] not in seen and not a.get("expired")]
    return [(i, n) for _, i, n in sorted(found)]


def upload(name, files, token, results_url, opener=urllib.request.urlopen):
    """Upload `files` ((name, bytes) pairs) as one artifact `name` of this run."""
    run, job = backend_ids(token)
    base = results_url.rstrip("/") + "/" + SERVICE

    def call(method, body):
        req = urllib.request.Request(
            base + method, data=json.dumps(body).encode(), method="POST",
            headers={"Authorization": "Bearer " + token, "Content-Type": "application/json",
                     "User-Agent": "timegrapher-walkthrough"})
        with opener(req, timeout=60) as r:
            answer = json.load(r)
        if not answer.get("ok"):
            raise RuntimeError(f"{method} refused: {answer}")
        return answer

    ids = {"workflow_run_backend_id": run, "workflow_job_run_backend_id": job, "name": name}
    signed = call("CreateArtifact", dict(ids, version=4))["signed_upload_url"]
    data = zip_files(files)
    put = urllib.request.Request(signed, data=data, method="PUT",
                                 headers={"x-ms-blob-type": "BlockBlob",
                                          "Content-Type": "application/zip"})
    with opener(put, timeout=120):
        pass
    call("FinalizeArtifact", dict(ids, size=str(len(data)),
                                  hash="sha256:" + hashlib.sha256(data).hexdigest()))
