# 26. Claude Desktop

**What it is:** an extension that lets an AI chat app, such as Claude
Desktop, use the timegrapher: read your recordings, record the watch in
each position, and show the readings as charts in the conversation.

**Why you'd use it:** to ask about a take in plain words ("What do these
figures say?", "Is the amplitude lower crown down?") and have the answer
come from what was measured, not a guess. The chat app can also guide a
whole positions test, telling you how to place the watch before each
take. It suits someone who would rather ask than read the tables.

The extension, `timegrapher-mcp`, runs the command line (chapter 25)
with `--json` and hands the chat app what it prints, so the figures in
the chat are the command line's, and the app's on the same sound and
settings.

## Installing

The Mac disk image and the Windows zip each hold `Timegrapher.mcpb` in
their **Claude Desktop** folder. It holds both programs, so nothing else
needs installing.

1. On Windows, extract the whole zip first.
2. Double-click `Timegrapher.mcpb`, or drag it into Claude Desktop's
   window. Or, in Claude Desktop's settings, choose **Extensions**,
   **Advanced settings**, then **Install Extension…** and pick it.
3. Choose **Install**. Claude Desktop says the extension isn't signed;
   that is expected. The timegrapher's tools appear in the chat's tools
   menu.
4. Start a chat with the **Test My Watch** prompt from the same menu, or
   just ask, giving a recording's whole path.

The Linux zip has no extension. Any chat app that runs local MCP
(Model Context Protocol) servers (VS Code, Goose, LM Studio and others) can use
`timegrapher-mcp` from the **Command Line** folder: give it the
program's full path as the command, with no arguments.
[The MCP page](../reference/mcp.md#install) gives the steps, and how to add it to
Claude Desktop by hand.

## What It Can Do

The chat app asks you before it runs each of these.

| It can | Runs | Why you'd ask for it |
| --- | --- | --- |
| Read one recording | `analyze` | The rate, amplitude and beat error of one position, and whether a regular beat was found. |
| Read a positions test | `session` | Each position's readings and the figures that sum them up (M8), with a report page. |
| Read how the readings move | `series` | Steadiness: trends, level changes, cycles and how sure each is (chapter 9). |
| Read a long take | `long` | Rate and amplitude over hours, and the cycles that point at a wheel. |
| Show the Tick and Tock | `profile`, `shape` | When the amplitude or beat error seems wrong (chapter 7). |
| Record one position | `record` | A positions test, one take at a time. **Uses the microphone.** |
| Check the microphone | `doctor` | The level, clipping, automatic gain and noise. **Uses the microphone**, changes nothing. |
| Check a recording | `doctor --file` | The same, for a file. |
| Look up a calibre | `calibres` | Its beat rate, lift angle and wheels, your own additions included. |
| List your watches | `watches` | Each watch and its takes, with each take's folder, so you can name a watch instead of a path. |
| List the sound inputs | `devices` | To pick the microphone. |
| Make a problem report | `report` | A zip to attach to an issue (chapter 27). |
| Read the guide | | The watchmaker's guide the chat app works from. |

It reads your watches while the app is open. Recording into a watch's
takes is refused while the app has the Watches folder open; close the
app first.

## The Test My Watch Prompt

The prompt, in the chat's tools menu, starts the test an experienced
watchmaker would run: the calibre, the microphone, then each of the six
positions, recorded into one folder, and the result read as a positions
test. Before each take the chat app says how to place the watch and
waits for you. Each take is kept; when no regular beat was heard, or
the sound clipped, the chat app is told so.

## Working with the Assistant at the Bench

The watchmaker skill provides an iterative, hypothesis-driven way to test and diagnose a watch:

### 1. Start with a Recording or a Symptom (Triage & Hypotheses)

Record a take in the Timegrapher app (or describe a problem) and ask naturally:

- "Analyze my last [Watch Name] recording."
- "My watch loses time when the chrono is on—what are the hypotheses and how do we test it?"
- "I just ran a 3-position test on my [Watch Name]. What looks off?"

What the agent will do:

1. Pull the latest take, verify the calibre and lift angle, and rule out setup or handling issues first (clipping, missed beats from a loose clamp, or temporary settling torque after winding).
2. Form ranked hypotheses from what the program measured (e.g., symmetric vs. one-sided amplitude drop, periodogram wheel matches, or positional variations).
3. Propose the cheapest case-closed change at the bench first (usually a 1- to 5-minute check) to confirm or eliminate candidate causes.

### 2. Run Differential Tests at the Bench

As you try each suggested change in the Timegrapher app, reply with what you did (for example, "waited 15 minutes", "re-clamped the watch", "turned chronograph on"). Guided by [bench-hypothesis-testing.md](../../skills/watchmaker/references/bench-hypothesis-testing.md), the agent judges which case-closed test fits the symptoms:

- **Settling check:** Wait 10–15 min after winding to rule out temporary mainspring bridle torque peaks.
- **Clamp & acoustic contact:** Re-seat the watch when a `beats_missing` caveat or poor signal appears in only one position.
- **Targeted positional pairs:** Compare DU vs. DD (endstones and endshake) and horizontal vs. verticals (`dvh_amplitude` for pivot friction, rate divergence for poise).
- **Complication load:** Compare chronograph ON vs. OFF to separate normal friction from excessive coupling drag.
- **Hand phase-shift:** Advance the hands 4–6 hours to separate hand-to-hand rubbing from dial/crystal contact or a wheel defect.
- **Date-change & wind progression:** Check date wheel engagement across midnight, or wind incrementally from stop to test bridle slip.

While an extended run is recording, ask "status and isochronism?" or "checkpoint" anytime for a live readout without stopping the take.

### 3. Open-Case Inspection Guidance

When testing has confirmed an internal fault and the watch needs service, ask:

- "What should a watchmaker inspect once the case is open?"
- "What does this rate and amplitude drop mean for service?"

What the agent will do:

The agent translates the measured figures and caveats directly into specific mechanical inspection points for the watchmaker—such as verifying hand clearances before uncasing, inspecting hairspring centering between regulator pins, checking chronograph coupling depth, or letting down power to observe going-train spin-down at a flagged wheel (see [diagnosis.md](../../skills/watchmaker/references/diagnosis.md#once-the-case-is-open-what-to-inspect)).

### 4. Other Built-In Modes

- **Full Guided Test from Scratch:** "Walk me through a full 6-position test on a new watch."
- **Microphone / Stand Troubleshooting:** "Check my microphone levels with doctor."
- **Seller QC Photo / Video Check:** Attach a photo or video of another timegrapher screen and ask "What do you make of this QC video?"

## Charts

In chat apps that show MCP Apps (Claude on the web and desktop among
them), five things show as charts in the conversation, drawn as the app
draws them:

| Chart | What it shows |
| --- | --- |
| One recording | The rate, amplitude and beat error, those three over time, and the Tick and Tock. |
| A positions test | Each position's readings and the figures that sum them up, with no pass or fail. |
| The Tick and Tock | Their profiles with the unlock and drop. |
| Recording | A **Live** view while it records: the readings, the last 30 seconds of the strip and the readings over time, about once a second, then what the check found. |
| Steadiness | The app's Steadiness views, with each one's headline and statistics, then the Isochronism walk and the readings' distributions. |

One recording, a positions test, Recording and Steadiness list what limits
the readings, when anything does, under **What Limits These Readings**.
Chat apps without charts get the same figures as text.

## What It Never Does

- **Upload anything.** A problem report is a file on your computer that
  you attach yourself.
- **Change the computer's sound settings.** It reports what it measured;
  any change is yours.
- **Use the microphone** except to record a position or check the
  microphone, and only for the seconds asked.
- **Grade the watch.** It passes on what was measured; any judgement is
  the chat app's, from the guide it reads, and yours.

## When It Doesn't Work

| What you see | What to do |
| --- | --- |
| "could not run timegrapher" | `timegrapher` isn't beside `timegrapher-mcp`. Keep the two together, as the extension does. |
| "is not a full path" | The chat app passed a bare file name. Give it the whole path. |
| No timegrapher tools in the chat | Quit the chat app completely and open it again. For a server added by hand, check the path in its settings file. |

## What It Can't Tell You

- **More than the command line does.** Every figure comes from it; the
  chat app's explanation is its own.
- **That the chat app read the figures right.** Its words are a model's;
  the charts and the command line's text are what was measured.

---

Previous: [The Command Line](25-command-line.md) ·
Next: [Problems and Reports](27-problems-and-reports.md)
