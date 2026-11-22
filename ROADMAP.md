# Roadmap

What the project is working on, what comes next, and ideas that are not yet
scheduled. Each line links to its issue, and the issue is the live record:
when they differ, the issue wins. Updated 22 November 2026.

If you'd like to help, read [CONTRIBUTING.md](CONTRIBUTING.md) first. Issues
labelled [`agent-ready`](https://github.com/peterhudson/timegrapher/labels/agent-ready)
are ready for someone outside the core team to claim, whether you write the
code yourself or with an AI agent.

## Now: making the app code easier to work on

[#1988](https://github.com/peterhudson/timegrapher/issues/1988) reorganises
the desktop app's code so that each part of its state has one clear owner,
with no change in what the app does. This makes the app easier to change and
gets it ready for one bench per microphone. It is an internal step, not a release. Until it is done, nothing
else lands in the desktop app crate (`crates/timegrapher-app`), so outside
pull requests there will wait.

Version 1.1.0 is the current release.

## Next: version 1.2

Headlines:

- [#1862](https://github.com/peterhudson/timegrapher/issues/1862) One bench per microphone, so several watches can be timed at once, each in its own tab.
- [#739](https://github.com/peterhudson/timegrapher/issues/739) Temperature logging alongside a take.
- [#1866](https://github.com/peterhudson/timegrapher/issues/1866) A watch report as a PDF, with the workshop's own name and logo.
- [#1874](https://github.com/peterhudson/timegrapher/issues/1874) The name "Open Timegrapher" in the app.
- [#1861](https://github.com/peterhudson/timegrapher/issues/1861) Calibre photos and spec sheets in the Calibres pane.

Also in 1.2:

- Hardening from the outside audit: [#1674](https://github.com/peterhudson/timegrapher/issues/1674) to [#1680](https://github.com/peterhudson/timegrapher/issues/1680).
- Live and saved readings agree: [#1819](https://github.com/peterhudson/timegrapher/issues/1819), [#1817](https://github.com/peterhudson/timegrapher/issues/1817), [#1827](https://github.com/peterhudson/timegrapher/issues/1827), [#1730](https://github.com/peterhudson/timegrapher/issues/1730), [#1917](https://github.com/peterhudson/timegrapher/issues/1917).
- Amplitude edge cases: [#1704](https://github.com/peterhudson/timegrapher/issues/1704), [#1707](https://github.com/peterhudson/timegrapher/issues/1707), [#1709](https://github.com/peterhudson/timegrapher/issues/1709), [#1788](https://github.com/peterhudson/timegrapher/issues/1788), [#1791](https://github.com/peterhudson/timegrapher/issues/1791).
- Error bars (±) and confidence: [#1696](https://github.com/peterhudson/timegrapher/issues/1696), [#1739](https://github.com/peterhudson/timegrapher/issues/1739), [#1802](https://github.com/peterhudson/timegrapher/issues/1802), and ± in the Positions Test ([#2085](https://github.com/peterhudson/timegrapher/issues/2085)).
- Guided tests: a poise dial ([#1993](https://github.com/peterhudson/timegrapher/issues/1993)) and a choice of position names ([#1994](https://github.com/peterhudson/timegrapher/issues/1994)).
- Stop listening by itself after a stretch with no beat ([#1934](https://github.com/peterhudson/timegrapher/issues/1934)).
- Bugs found in testing 1.1, gathered in [#2130](https://github.com/peterhudson/timegrapher/issues/2130).

## Later: version 1.3

- [#1513](https://github.com/peterhudson/timegrapher/issues/1513) Translations, first wave of six languages.

## Ideas, not scheduled

These have been suggested but not yet chosen. A comment on the issue saying
why it would help you is welcome. Please don't start code for one until it is
scheduled: the design usually needs deciding first.

- Compare two takes of the same watch, for example before and after a service or a mainspring change.
- [#1515](https://github.com/peterhudson/timegrapher/issues/1515) Extend a saved take instead of only replaying it.
- A Positions Test at a third of the power reserve.
- A multi-day series in the style of chronometer certification.
- Lift angle measured from slow-motion video.
- A motorised stand, or a stand that reports its own angle.
- [#401](https://github.com/peterhudson/timegrapher/issues/401), [#402](https://github.com/peterhudson/timegrapher/issues/402) A public calibre table website and download.

## Not planned

Touch and mobile versions ([#1585](https://github.com/peterhudson/timegrapher/issues/1585)
records why). Escapements other than the Swiss lever.
