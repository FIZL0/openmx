# S17 — the rider controllers

2026-10-16. Task: `./kit plan S17`.

The four controllers the ground step drives and S3 left as hooks — the low-speed upright hold, the
one-wheel balance, the wheelie-and-stoppie pitch control and the no-hander's roll — are ported in
`crates/game-sim/src/move.rs` and proved against the retail code over 10,000 generated states each,
every word of the five-object graph plus the ordered list of calls each one makes outside itself. Two
of S3's readings are corrected on the way: the four accumulators at `+0xa4c..+0xa6f` belong to the
balance controller, not the no-hander, and `bike+0x650`/`+0x654` are the front and rear wheels' own
brake commands rather than one word written twice — the wheelie's veto reads the rear, the stoppie's
gate the front. Seven of eight deliberate mutations failed where they should; the eighth is a
documented unobservable branch. `./kit check`: 425 tests pass. Nothing waits on the human.
