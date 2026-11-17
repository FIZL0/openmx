# 1 — what MX vs. ATV scores its backflip on

Task R17. 2026-11-16.

## What I did

- Found the scorer. It is the stunt start and end themselves. S223 read their score calls as animation calls:
  the "rider's animation objects" (`+0x680`/`+0x684`) are the bike's score card and the score manager.
- `crates/game-sim/tests/mxvatv_stunt_score.rs`, four tests run in SLUS-21104's own code on state04's real
  bike, card and table (only the syscall stubs hooked):
  - `the_backflip_reads_its_points_off_the_stunt_table`: the table's base and rate for all 46 rows.
  - `a_backflip_scores_its_held_time_and_banks_on_the_end`: start, 15 ticks of growth (checked bit for bit
    against a model each step), end, bank, and the repeat scale's drop.
  - `a_combo_banks_its_sum_times_its_count`: backflip + wheelie bank at (a + b) × 2.
  - `a_wreck_loses_the_backflip`: the wreck's cancel banks nothing.
- research/oracle.txt: the R17 block under S223's rows (S223's two wrong readings corrected in place),
  `atv_score_grow` and `atv_stunt_land_bank`.
- Filed S340 (low): port this into the rider's points for trick-pack rides.
- No product change (an R task). Nothing taken from ../opentennis: it has no trick scoring.

## What the original does

Recipes: `./kit fn 0x004e6370 -p atv` (start), `0x004e4db0` (end), `0x004e4e50` (wreck cancel), `0x0046aae0`
(node push), `0x0046b210` (growth), `0x004e5bd0` (bank on landing), `0x004e4f20` (backflip, start/end calls
near its end).

- **Start** (bike vt +0x4b8, a2 = 1 for the backflip). With card and manager set:
  - a non-flip id first ends the live non-flip nodes;
  - it opens a combo unless one is open;
  - it pushes a node: base = max(PointValue × scale, 10), rate = max(HoldPointValue × scale, 10), mult 1,
    granularity 1, flags 0xc (round down);
  - it marks the node growing when a2 is 1.
  The row is the stunt table's entry for the id. Its `+0` is the ini row (`+0x50` PointValue, `+0x54`
  HoldPointValue, checked against `ATV_Rider_Stunts.ini`). Its `+4` is the stunt's repeat scale.
- **The backflip's numbers** (state04, the ini's row 41): PointValue 2000, HoldPointValue 10000. Tricks 0–40,
  the wheelie and the stoppie are 1000/1000. The 360s are 2000/15000.
- **When**: the backflip step calls start(backflip id, 1) when the clip begins (node `+0x84 <= 0`, `+0x94 > 0`,
  clip word 0). It calls end(backflip id) when the flip comes round (`+0x84 > 0`, `+0x94 > -0.79`, clip word 2).
- **Growth** (the manager's tick): every growing node gets held += dt, points = (base + held × rate) × mult,
  rounded down to 1. So a backflip is worth 2000 + 10 000 a second from clip start to clip end.
  Half a second (15 ticks) gives 6999, not 7000: the held time falls just short of 0.5.
- **End**: finishes the node (×1, rounded). It stays in the combo.
- **Bank**: the stunt clock (already ported as `handling::update::stunt_clock`) waits 0.2 s on the ground, then
  calls end(−1). That finishes every live node and adds (sum of nodes) × (node count) to the total, floored
  to the manager's granularity (1.0).
- **Wreck**: the cancel (vt +0x4b0) finishes every node as not landed and closes the combo with nothing added.
- **Repeat**: the bank's watcher multiplies each banked stunt's scale by 0.8 (floor 0.4096; state04's table
  `+8` and `+0xc`). The next backflip opens at 1600 and grows 8000 a second.
- S223 corrections: `+0x13c4`/`+0x13c8` are the wheelie (42) and stoppie (43) ids, not "the 360's pair". The
  360s are `+0x13bc`/`+0x13c0` (44, 45). The update starts the wheelie and stoppie after a second on one wheel.

### Against MX Unleashed (the port's `score::Score`)

| | MX Unleashed | MX vs. ATV |
|---|---|---|
| backflip | flat 10 000 | 2000 + 10 000/s, clip start → clip end |
| scored when | on landing (or after the flip comes round) | node opens at clip start |
| combo multiplier | each trick × its place in the combo, at its own end | whole combo × node count, at the bank |
| bank | both wheels down, or slow | 0.2 s on the ground |
| repeats | no penalty | × 0.8 per banked repeat, floor 0.4096 |
| wreck | combo lost | combo lost (same) |

The score calls themselves were already ported by F31b5h (`handling::run`), but only for the frame replay.
The rider's points never use them. Also, the trick pack (`trick_pack::Moves`) reads HoldPointValue as a flat
backflip/360 score, which is not what the original does with it. Both go to S340.

## How it was proved

Function oracle on state04's RAM image (`tests/mxvatv_stunt_score.rs`, 4 tests, deterministic sequences, not
generated inputs). It compares node words, card total and flags bit for bit, and growth against a
`ps2_float` model at ticks 1, 14 and 15. `./kit check -- -p game-sim --test mxvatv_stunt_score`: 4 passed.

## Not verified / not 1:1

- **The port still scores MX Unleashed's way.** Nothing in `score.rs` or `handling/steps.rs` changed: R is
  research only. The port is S340.
- **The backflip's start/end moments were read, not run.** The tests call start and end directly. That the
  backflip step calls them on the node-row conditions above is from the decompilation of the backflip.
  The port's MX vs. ATV backflip test (`mxvatv_backflip_is_the_originals`) hooks both calls, which is
  indirect support.
- **The bank on landing was read, not run here.** The 0.2 s clock is already ported (`handling::update::stunt_clock`). Its
  trigger conditions are as read.
- **Growth was checked over 30 ticks at most.** It was not checked over a long hold. The rounding has a large
  margin at these sizes.
- **Repeat beyond one step not run.** The 0.95 recovery and the 0.4096 floor are from F31b5h's `repeat` and
  the table words, not run here.
- **The trick path is unchecked.** The trick request calls start with a2 = 0, so a trick's node seems not to
  be marked growing (a flat 1000). That is read only; tricks were not part of this task. Noted on S340.
- **The listeners' side effects were not checked** (HUD gauges, event posts, best combo). The tests let them
  run on the image but compare only the card and nodes.
- **The PC exe was not read** for this. SLUS-21104 was the source throughout.
- **Only state04's table.** Another machine or build could load other ini numbers. The code reads them from
  the ini, so only the numbers would change.

## Next

S340 ports it into the rider's points.
