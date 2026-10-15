# I2 — axes

2026-10-15. Task: `./kit plan I2`.

The analogue sticks are ported into `crates/game-sim/src/input.rs` beside I1's controls: six axis slots,
the map that says which report byte each reads (`[6, 7, 4, 0, 0, 5]` — and index 0 means "nothing bound",
which takes 127.0 every frame), the inversion the dispatch applies to the right stick's X and nothing
else, and the curve every consumer's value comes out of. The answers the task wanted: the dead zone is
±0.2 of the output range (report bytes 102..=152 read exactly zero, 101 and 153 are the first that
answer), there is **no clamp** anywhere, and there is **no curve** — two straight segments stretched back
over the full range either side of the dead zone, so outside it the stick is exactly 1.25× steeper than
the byte. Proved twice: 11,524 oracle calls against the game's own `pad_axis_curve` over every byte a
report can hold and 43 ranges, including degenerate ones that put a zero in each divisor; and a 126-frame
recording of the real game's main menu with both sticks swept, replayed row for row — 630 consumer values
bit-exact, each against the range recorded in its own row. Done; see `1-axes-FINAL.md`, whose "Not
verified / not 1:1" list names the unported history poll, pad 1's unproven map, and the value being kept
per axis where the original keeps it per consumer.
