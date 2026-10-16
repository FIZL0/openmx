# S8 — s8 acceptance

2026-10-15. Task: `./kit plan S8`.

The acceptance run is the whole machinery minus the run itself. S8 asks for several minutes of a human's
freeriding to replay tick-exact against the port; no such recording exists anywhere on this machine (the
human item in `plan/HUMAN.md` and slot 4 are both open), and the port has no bike to replay one against
(`Sim` is still A1's example ball). So what landed is the recipe for the recording
(`watch/acceptance.toml`), the reader that turns a capture of it into a `Fixture` — the input side of A2's
harness, which did not exist before — the scale the harness has to work at (9,000 ticks, a divergence found
and named deep inside one), and three measurements taken from two live captures that would each have made
the first real acceptance run silently wrong: the game ticks at half the recorder's `frame` column, its `dt`
is measured rather than fixed, and its sixteen controls are not libpad's bits. `the_acceptance_run` is
`--ignored` and waits for the file. Marked `[~]`: it needs R7 or the human. Details, and the `dt` paragraph
nobody should write the port's tick without reading, in `1-s8-acceptance-READY.md`.

**Second session (2-s8-acceptance-READY.md).** The run has a run in it now: two captures of the original
from the save state the human's recording is anchored to — its own minute with the bike idling (1,811
ticks) and thirty seconds of a scripted ride through the kit's pad (900 ticks, the bike up to 51 units/s)
— and `the_acceptance_run` out of `--ignored`, building a fixture from the real capture on every
`./kit check` and printing its verdict. Today's verdict is `NoSuchField bike.pos[0]`: the capture is good,
and S3 has not landed a bike. Three findings that each had to come from the data: the shipped watch chain
was aimed at an **AI rider's** vehicle (a freeride area holds four ridden ones; the human's is the eighth
list entry, and the capture now carries the control-source column that proves it), the per-frame `dt` is
the 30 fps clamp on all 2,711 captured ticks, and PCSX2 plays a `.p2m2` back only from its GUI — which an
agent cannot reach here, so the human's own input has still never driven the original. That is the new
`plan/HUMAN.md` ask, ten minutes at the desktop, and it is why the entry is READY.

**Third session (3-s8-acceptance-PART.md).** The recording drove the original. PCSX2 plays a `.p2m2` back
from its GUI only and no keystroke on this desktop reaches its window — but a Qt menu item can be pressed
over the accessibility bus without focus or a pointer, so Tools → Input Recording → Play was pressed that
way, the file dialog filled the same way, and the save-state load caught on the tick it happened
(`frame_counter` back to 2773). The capture is the human's own minute as the original played it: 1,811
ticks with no gap, 1,449 of them with a control held, the bike to 77 units/s over 3,071 units of ground,
two crashes. That settles the tick phase I4 left open and an idle capture never could — **the game reads
the second of a tick's two video fields**, matching RAM byte for byte on all 1,810 overlapping ticks while
the other phase loses it at the first press — and it measures two other tasks' open numbers: S14's wreck
window (ticks 3259–3349 and 4167–4190, reason 5, so the tick-exact window ends at 3258) and S22's
headroom (sideways `g_peak` peaks at 6.12 against a limit of 16.87). The verdict is unchanged and still
honest: `NoSuchField bike.pos[0]`, because S3 has not landed a bike — but the 14,488 checks behind it are
now checks against a minute somebody rode. One casualty, repaired: PCSX2 switches to record mode at the
end of a movie and kept appending to the human's `.p2m2`; a test caught it, four worktree copies restored
it byte for byte, and the file is read-only now. The entry is PART, not READY: nothing in S8 needs a
person any more except a *longer* recording.
