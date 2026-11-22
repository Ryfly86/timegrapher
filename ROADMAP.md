# Roadmap

What the project is working on, what comes next, and ideas that are not yet
scheduled. Where a line links to an issue, the issue is the live record:
when they differ, the issue wins. Updated 22 November 2026.

If you'd like to help, read [CONTRIBUTING.md](CONTRIBUTING.md) first. Issues
labelled [`agent-ready`](https://github.com/peterhudson/timegrapher/labels/agent-ready)
are ready for someone outside the core team to claim, whether you write the
code yourself or with an AI agent.

## Now: making the app code easier to work on

[#1988](https://github.com/peterhudson/timegrapher/issues/1988) reorganises
the desktop app's code so that each part of its state has one clear owner,
with no change in what the app does. This makes the app easier to change, and
gets it ready for timing several watches at once. It is an internal step, not
a release.

Until it is done, no other change is merged into the desktop app
(`crates/timegrapher-app`). Outside it, issues labelled `agent-ready` are
open to outside contributors now.

Version 1.1.0 is the current release.

## Next: one headline per point release

After the refactor, each point release carries one large feature. These are
the candidates; their order is being decided, and version numbers follow it.

- [#1862](https://github.com/peterhudson/timegrapher/issues/1862) One bench per microphone, so several watches can be timed at once, each in its own tab.
- [#2201](https://github.com/peterhudson/timegrapher/issues/2201) Compare two takes of the same watch, for example before and after a service or a mainspring change.
- [#1866](https://github.com/peterhudson/timegrapher/issues/1866) A watch report as a PDF, with the workshop's own name and logo.
- A multi-day series in the style of chronometer certification.
- [#1513](https://github.com/peterhudson/timegrapher/issues/1513) Translations, first wave of six languages (best last, once the wording is settled).
- [#739](https://github.com/peterhudson/timegrapher/issues/739) Temperature logging alongside a take (waits for the probe hardware).

Every point release also carries smaller work:

- The name "Open Timegrapher" in the app ([#1874](https://github.com/peterhudson/timegrapher/issues/1874)) and replacing the unmaintained `chrono` dependency flagged by a security advisory ([#1856](https://github.com/peterhudson/timegrapher/issues/1856)), both in the first one.
- Bugs found in testing 1.1, gathered in [#2130](https://github.com/peterhudson/timegrapher/issues/2130), plus others filed since.
- Live and saved readings agree: [#1819](https://github.com/peterhudson/timegrapher/issues/1819), [#1817](https://github.com/peterhudson/timegrapher/issues/1817), [#1827](https://github.com/peterhudson/timegrapher/issues/1827), [#1730](https://github.com/peterhudson/timegrapher/issues/1730), [#1917](https://github.com/peterhudson/timegrapher/issues/1917).
- Amplitude edge cases: [#1704](https://github.com/peterhudson/timegrapher/issues/1704), [#1707](https://github.com/peterhudson/timegrapher/issues/1707), [#1709](https://github.com/peterhudson/timegrapher/issues/1709), [#1788](https://github.com/peterhudson/timegrapher/issues/1788), [#1791](https://github.com/peterhudson/timegrapher/issues/1791).
- Error bars (±) and confidence: [#1696](https://github.com/peterhudson/timegrapher/issues/1696), [#1739](https://github.com/peterhudson/timegrapher/issues/1739), [#1802](https://github.com/peterhudson/timegrapher/issues/1802), and ± in the Positions Test ([#2085](https://github.com/peterhudson/timegrapher/issues/2085)).
- Period search: [#1793](https://github.com/peterhudson/timegrapher/issues/1793), [#1795](https://github.com/peterhudson/timegrapher/issues/1795), [#1848](https://github.com/peterhudson/timegrapher/issues/1848), [#1844](https://github.com/peterhudson/timegrapher/issues/1844).
- Hardening from the outside audit: [#1674](https://github.com/peterhudson/timegrapher/issues/1674) to [#1680](https://github.com/peterhudson/timegrapher/issues/1680).
- Calibre photos and spec sheets in the Calibres pane ([#1861](https://github.com/peterhudson/timegrapher/issues/1861)).
- Guided tests: a poise dial ([#1993](https://github.com/peterhudson/timegrapher/issues/1993)) and a choice of position names ([#1994](https://github.com/peterhudson/timegrapher/issues/1994)).
- Stop listening by itself after a stretch with no beat ([#1934](https://github.com/peterhudson/timegrapher/issues/1934)).

## Ideas, not scheduled

These have been suggested but not yet chosen. If one would help you, say why
in a comment on its issue, or file it through the
[idea form](https://github.com/peterhudson/timegrapher/issues/new/choose) if
it has none yet. Please don't start code for one until it is
scheduled: the design usually needs deciding first.

- [#1515](https://github.com/peterhudson/timegrapher/issues/1515) Extend a saved take instead of only replaying it.
- A Positions Test at a third of the power reserve.
- Lift angle measured from slow-motion video.
- A motorised stand, or a stand that reports its own angle.
- [#401](https://github.com/peterhudson/timegrapher/issues/401), [#402](https://github.com/peterhudson/timegrapher/issues/402) A public calibre table website and download.

## Not planned

- Touch and mobile versions ([#1585](https://github.com/peterhudson/timegrapher/issues/1585) records why).
- Escapements other than the Swiss lever.
