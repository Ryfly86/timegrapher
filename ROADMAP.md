# Roadmap

What the project is working on, what comes next, and ideas that are not yet
scheduled. Where a line links to an issue, the issue is the live record:
when they differ, the issue wins. Updated 26 November 2026.

If you'd like to help, read [CONTRIBUTING.md](CONTRIBUTING.md) first. Issues
labelled [`agent-ready`](https://github.com/peterhudson/timegrapher/labels/agent-ready)
are ready for someone outside the core team to claim, whether you write the
code yourself or with an AI agent.

## Now: fixing what testing found

The desktop app's code reorganisation
([#1988](https://github.com/peterhudson/timegrapher/issues/1988)) is done.
Before any new feature, the project is fixing the bugs found in testing:

- [#2130](https://github.com/peterhudson/timegrapher/issues/2130) gathers the
  bugs found in testing 1.1 and in the shakedown runs since.
- [#2400](https://github.com/peterhudson/timegrapher/issues/2400) tracks the
  findings of an outside audit of the repository.

Version 1.1.0 is the current release. A 1.1.1 point release follows once the
shakedown fixes and the audit's data-loss and wrong-rate fixes are in.

Co-axial escapements are being added as a beta
([#2313](https://github.com/peterhudson/timegrapher/issues/2313)): the rate
and beat error, and an amplitude marked as an estimate.

Issues labelled `agent-ready` are open to outside contributors now.

## Next: one headline per point release

After 1.1.1, each point release carries one large feature, in this
order:

| Release | Headline |
|---|---|
| 1.2 | [#1862](https://github.com/peterhudson/timegrapher/issues/1862) One bench per microphone, so several watches can be timed at once, each in its own tab. |
| 1.3 | [#1866](https://github.com/peterhudson/timegrapher/issues/1866) A watch report as a PDF, with the workshop's own name and logo. |
| 1.4 | [#2201](https://github.com/peterhudson/timegrapher/issues/2201) Compare two takes of the same watch, for example before and after a service or a mainspring change. |
| 1.5 | [#1513](https://github.com/peterhudson/timegrapher/issues/1513) Translations, first wave of six languages. |

1.2 also carries these smaller items, most of them asked for by users:

- [#2236](https://github.com/peterhudson/timegrapher/issues/2236) Listen to the tick: hear the microphone live, switched on and off from the Listen card, with a volume.
- [#2223](https://github.com/peterhudson/timegrapher/issues/2223) Show the measured beat rate in vph, and how far it is from the set rate, with [#2224](https://github.com/peterhudson/timegrapher/issues/2224) typing an exact beat rate, for balances not at a standard rate.

1.3 also carries [#2222](https://github.com/peterhudson/timegrapher/issues/2222), a History list for each watch: notes on takes, and dated events such as a service, with photos that can go in the PDF report. It also carries [#2225](https://github.com/peterhudson/timegrapher/issues/2225), reading the positions against a measured reference rate, to poise and time a free-sprung balance together.

Later, in this order, with no version yet:

- [#2202](https://github.com/peterhudson/timegrapher/issues/2202) A multi-day series in the style of chronometer certification.
- [#739](https://github.com/peterhudson/timegrapher/issues/739) Temperature logging alongside a take (waits for the probe hardware).
- [#2203](https://github.com/peterhudson/timegrapher/issues/2203) Lift angle measured from slow-motion video.

Every point release also carries smaller work:

- The name "Open Timegrapher" in the app ([#1874](https://github.com/peterhudson/timegrapher/issues/1874)) and replacing the unmaintained `chrono` dependency flagged by a security advisory ([#1856](https://github.com/peterhudson/timegrapher/issues/1856)), both in 1.2.
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
- A motorised stand, or a stand that reports its own angle.
- [#401](https://github.com/peterhudson/timegrapher/issues/401), [#402](https://github.com/peterhudson/timegrapher/issues/402) A public calibre table website and download.

## Not planned

- Touch and mobile versions ([#1585](https://github.com/peterhudson/timegrapher/issues/1585) records why).
- Escapements other than the Swiss lever and the co-axial.
