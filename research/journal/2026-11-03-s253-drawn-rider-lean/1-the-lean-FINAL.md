# 1 — the lean (FINAL)

Task S253. 2026-11-03.

## How it was found

The tool is `the_drawn_rider_leans_as_the_original_through_the_turns`
(`crates/game-sim/tests/fly_and_collide.rs`). It compares the drawn rider, as the shell builds it,
against S228's fly capture. The probes were temporary env switches in that test and in
`Riding::ride`. All of them have been removed.

- **M**: pose the chassis from `rider.animation` through an `anim::Rig` over the bike's set. This
  replaces the stance's pose, then runs the drives.
- **U**: hang the target under `before`, not `holds.root × before`.
- **F**: put the request frame on the ride, not `holds.root × m`.
- **J**: write the capture's body joints (`p1_j<n>.q/.t`) over the port's after `Body::ride`.

| run | hard-turn lean | root roll | lean to a side, worst (ft) | hard ticks |
|---|---|---|---|---|
| before S253 | 0.795 | 0.896 | 0.713 | 274 |
| M | 0.790 | 1.027 | 0.878 | – |
| U (landed) | 0.920 | 0.888 | 0.609 | 276 |
| M + U | 0.960 | 1.019 | 0.458 | 280 |
| M + U + J | 0.999 | 1.018 | 0.205 | 287 |
| M + F | – | – | 1.689 | – |

- **M** alone fixes the root's roll but not the lean. **U** fixes most of the lean. With both, plus
  the original's own body joints, the lean is the original's within 0.1%.
- During the probes I noted a U-only ratio of 0.958. The clean rerun after the probes were removed
  gives 0.920. The 0.958 was most likely measured with another switch still set.

## The port

- `ragdoll_riding::Riding::ride`: `under = *before`, with a comment and its doc saying why.
- The pin in `fly_and_collide.rs`, now pinned at the landed U row:
  - the lean ratio 0.79 → 0.91;
  - the lean to a side: p90 0.25 → 0.15, worst 0.75 → 0.65;
  - the lean as a vector: 0.95 → 0.85;
  - `DRAWN_TICKS` (1675, 274) → (1675, 276).
  - The root bounds are unchanged (worst root up 0.283).
- `a_full_whip_keeps_the_hands_on_the_bars` (`crates/game-render/tests/whip_tricks.rs`):
  - At the full whip the body's root joint has −35° of roll, against the drawn root node's −56°. The
    grips roll with the root node, so the hands are now held against a target that no longer counts
    the crossup twice.
  - The ragdoll's hands are 0.086 and 0.144 ft off the grips. The bound goes from 0.1 to 0.15, still
    inside V100's 0.2.
  - Its pelvis still moves further than the stand-in's (0.49 against 0.32 ft). But the direction
    against `lean_in`'s now has cosine −0.2, where it was over 0.75. That check compared the ragdoll
    against a port stand-in, not the original, so it was dropped from the asserts; the cosine is
    still printed in the message.

## How it was checked

- `./kit check -- -p game-sim --no-fail-fast`: green except `the_humans_tricks` (trick.rs, rows
  whose pad is not field 2k−2). That one reds on C6's capture before this task too.
- `./kit check -- -p game-render --no-fail-fast`: green, 293 tests.
- `./kit check -- -p openmx --no-fail-fast`: green except
  `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest` (menu.rs:5359, Fog reads
  "Whole Map" where the test expects "Farther Fog"). It fails the same way run alone. That is menu
  text, which this change does not reach.

## Not verified / not 1:1

- **Not looked at on screen.** The riding ragdoll is what the shell draws by default (S90). The turns
  are capture-backed, but no run was watched.
- **The whip's lean direction is unproven.** The test above compares the ragdoll against the port's
  stand-in. No capture of the original's riding body in a full whip was compared.
- **The root is still the wrong machine.** The drawn root rolls 0.89 of the original's and has no
  rest roll. The M row shows the fix, but it is not landed (follow-up).
- **The remaining 4% under M + U** is the port's body joints. Writing the original's over them (J)
  closes it, so the gap is in the body's clip layer (V121's partings). Not chased.
- **The request frame's root node** (`holds.root × m`) is kept on S111's evidence. Only the F row
  tested it, and that row was much worse.
- **The menu failure was not checked against HEAD.** It is named above as unrelated on what it
  tests, not on a run of the old code.
