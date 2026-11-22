# Contributing

Thank you for helping. Calibre figures, recordings and bug reports go
through the [issue forms](https://github.com/peterhudson/timegrapher/issues/new/choose).
Code and documentation go through pull requests.

## Picking up an issue, with or without an AI agent

The [roadmap](ROADMAP.md) says what is being worked on and what comes next.
Issues labelled [`agent-ready`](https://github.com/peterhudson/timegrapher/labels/agent-ready)
are the ones open to outside contributors. You are welcome to do them
yourself or with an AI coding agent. Please work only on those: other issues
often have work in progress, or a design that still needs deciding.

An issue gets the label only when it is small (about one pull request), says
which files and tests it needs, and does not touch work in progress. Parts of the code
that are being reworked, listed under "Now" in the [roadmap](ROADMAP.md),
get no labelled issues until that work is done.
An issue that changes how a reading is computed gets the label only once it
spells out the method.

**Claiming.** Comment "I'd like to take this" on the issue. A maintainer
assigns it to you, usually within a day. Hold one claimed issue at a time.
If there is no draft pull request or progress comment after 7 days, the
issue is unassigned so someone else can take it; just comment again if you
are still on it. Comment "releasing this" if you stop.

**Your pull request** follows [AGENTS.md](AGENTS.md): one issue, one branch,
one pull request, the body sections it lists, `Closes #N`, and tests that fail
without the fix. It also needs:

- the [CLA](CLA.md) accepted before the first merge;
- all eight required checks green (a maintainer starts CI on a first pull
  request from a fork);
- for a bug fix, one line saying where the bug came from: "Introduced by #N", "Latent" (it
  was always there) or "Design gap" (nothing was meant to handle the case);
- if an AI agent wrote any of it, an `Assisted-by: <tool>` line in the body,
  naming the tool only, with no model or version (for example
  `Assisted-by: Codex`).

You are responsible for the pull request, whoever or whatever typed it: read
the diff, run the checks yourself, and answer review comments. Outside pull
requests go through the same review as the core team's (code review, and the
math, design or walkthrough review when the change needs it). Pull requests
for issues without the label, or that change files the issue does not name,
may be closed with a pointer to this section.

**Ideas** for new features are welcome as issues, through the
[idea form](https://github.com/peterhudson/timegrapher/issues/new/choose).
Please wait for one to be scheduled before writing code for it.

## Licence of contributions

The code is GPL-3.0-or-later, and the public project stays that way.

Before your first pull request is merged, you will be asked to accept the
[Contributor Licence Agreement](CLA.md) by posting the one comment that
section 9 gives. You keep your copyright. The agreement lets the project's
owner also offer the same code under other terms, for example in a closed
App Store edition. One acceptance covers your later pull requests.

Calibre figures and recordings sent through the issue forms are given
under CC0, as each form says, and need no agreement. A recording you
attach to a bug report but keep out of the public corpus stays that way,
even if you later accept the agreement.

## Recording an acceptance

A maintainer adds a line to [docs/developer/cla-acceptances.md](docs/developer/cla-acceptances.md)
before merging: the GitHub account and its numeric id, the date, the pull
request, the agreement version, and any organisation signing.
