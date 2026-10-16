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
