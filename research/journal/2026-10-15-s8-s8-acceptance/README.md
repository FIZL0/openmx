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
