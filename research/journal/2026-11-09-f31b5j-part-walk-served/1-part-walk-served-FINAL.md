# 1 — part walk served (FINAL)

Task F31b5j. 2026-11-09.

## What I did

- **`update::Out::Walk` is served by `tail::walk`** in `crates/game-sim/tests/mxvatv_bike_ride.rs`'s
  `UpdateInRam::out`, through a new `PartWalk` (`Listeners` on RAM). A group's step (the walk itself)
  recurses into the port's walk. A leaf's step runs the original's code, with its closing walk (`TAIL`)
  hooked and stood in for by the port's walk. Each leaf step is watched: it may not change a word of the
  bike or its physics blocks (`bike_and_blocks`). The census books each kind's instructions, the steps
  that moved `rand`'s state, and any step that built the smooth noise.
- **`PART_LEAVES`**: the four kinds of leaf the walks reach on state 4, keyed by their step. An unknown
  leaf panics. The walks reach 14 listeners: one `models`, one `effects`, eleven `emitter`s, one
  `second emitter`.
- **Every-port tests** (`ride_/flip_replays_with_every_port_at_once`) now also assert `walk_served`: no
  `the tail: the update's Walk` in the interpreter's census, and every kind of leaf stepped. Both pass,
  with the RAM matching the handing run's (flip-1 apart from the dropped cue's word, as before).
- **What the leaves give the bike** (`replay_without_part_leaves`, modes in `IN_RUST`):
  - `ride_replays_without_the_part_leaves`: all leaves dropped, ride-2 is still C15's on all 459 rows,
    and the bike and its blocks match the handing run's after every frame.
  - `flip_needs_the_part_walks_effects`: all leaves dropped, flip-1's bike parts on row 114 at `+0x14e4`
    (the trick hold's clip fraction) and `+0x908` (its copy). With only the effects' own work dropped
    (walk kept), it parts at the same row and words. With only the models' own work dropped, flip-1 is
    C15's on all 465 rows, wreck at 125, put-back at 193, and the bike matches the handing run's.
  - For those modes, `TreeInRam`'s whole-RAM check against the original vehicle update is narrowed to
    the bike and its physics blocks (the leaves' own writes are bound to differ).
- **Oracle kept as it was**: `the_trail_sender_and_part_virtuals_are_the_originals` sets `"the walk
  handed on"`, so the part step's walk still goes to the hooked `TAIL` and is compared as a call, as
  F44 wrote it. The callee census lists (`RIDE_CALLEES`, `FLIP_CALLEES`) lose `update Walk`.
- **Filed F31b5n**: port the effects leaf and the two emitter kinds, at least as far as their `rand`
  draws. Noted on F31b5 that it now waits on F31b5n too.
- Nothing taken from opentennis: it has no interpreter-backed frame and no particle emitters ported.

## What the original does

- The walk is F31b4a's (`./kit fn 0x00128af0 -o atv`), already ported as `tail::walk`. The update's
  part walks are the bike's own listeners (`+0xc`) and each part step's (`+0x24`).
- **models** (`./kit fn 0x00141d30 -o atv`): `+0x58 = (+0x58 == 0)`, a frame parity. Then, for each
  entry of its `+0x228` array (count `+0x234`) that is set and has `+8` clear, it attaches named models
  (`./kit fn 0x0013edb0 -o atv`: a name compare over the model's nodes, then an attach). Then it walks
  its own listeners. 20 instructions a frame on C15: only the parity flips.
- **effects** (`./kit fn 0x004fa4a0 -o atv`): its table at `+0x24` (count, then entries 0x2c apart).
  Each entry gets a flags byte at `+0x28` (bit 0: both pointed-at words zero, bit 1 / bit 2: the
  `+8` / `+0xc` word set, 0x10 or 8 by the table's `+0x24` word). When the char at `+0x24` changed
  from `+0x29`, it is stored and each item of the `+0x28` list bound to the entry is told (item vt
  `+8`). Then each item is turned on (vt `+0x18`) or off (vt `+0x1c`) by its mask (`+0xc`) against
  the entry's flags and the table's bit, and stepped (vt `+0x14`, `dt`) while on (`+8 == 1`). Then
  it walks its own listeners. Some item steps draw `rand`.
- **emitter** (listener at object `+0xc`, step `./kit dis 0x00511c10 -p atv` is a thunk to
  `./kit fn 0x0050ceb0 -o atv`) and **second emitter** (thunk `0x004ffd20` to
  `./kit fn 0x004fbcc0 -o atv`). Each reads its two spots (`+0x30`, `+0x40`) and their distance over
  `dt`, and above its `+0xf4` spawns particles. It draws `rand` (`./kit fn 0x003561a8 -o atv`) for the
  count and spread. No closing walk.
- **How `rand` reaches the bike**: the trick hold (`update::trick_hold`) samples the smooth noise.
  State 4's noise tables are not built yet, so the first sample builds them off `rand`
  (`NOISE_BUILD`), which the emitters (and some effects items) have been drawing on every frame. No
  part leaf builds the noise itself (the census's `steps that built the noise` stays empty). The
  wreck's out-of-bounds throw and the rider's reset draw on the same `rand`.

## Census (every port on; instructions, and steps that moved `rand`)

| kind | ride-2 | flip-1 | rand-moving steps ride-2 / flip-1 |
|---|---|---|---|
| models | 12.4k | 12.6k | 0 / 0 |
| effects (own work) | 0.89M | 0.89M | 26 / 9 |
| emitter (11 listeners) | 3.03M | 1.82M | 657 / 428 |
| second emitter | 92k | 93k | 0 / 0 |

The update's `Walk` itself no longer appears; the walk is Rust.

## How it was proved

- Bit-exact replays of C15 ride-2 (459 rows) and flip-1 (465 rows) off
  `context/mxvatv-ps2/state04.ee.bin`, with every port on and the walk the port's. Every row is word
  for word C15's, and the whole RAM matches the handing run's after every frame.
- The same replays with the leaves (or one kind's own work) dropped, compared each frame against C15
  and against the handing run's bike object and six physics blocks.
- `./kit check -- -p game-sim --test mxvatv_bike_ride`: the whole binary. The three tests that
  expected the old interpreter walk were fixed, and all pass.
- Gate: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **The effects and emitters are not ported, and not inert.** They write no word of the bike or its
  blocks, but through `rand` they reach the bike (flip-1 row 114). So the task's "each part leaf ported
  or shown not to touch the bike" holds for the models leaf only. The other two kinds still run in the
  interpreter inside `PartWalk`. Porting them is F31b5n. I did not port them here: about 7 KB of
  VU0-heavy emitter code plus the effects items' virtuals and the particle spawners, and they need
  their float compares bit-exact to get the draw counts right.
- **The `rand` link is shown by dropping, not by isolating.** I tried undoing all of the leaves' writes
  except `rand`'s state (and then the noise tables too), and flip-1 still parted at row 114. That
  experiment is ill-posed: undoing the emitters' own state changes how many `rand`s they draw next
  frame. So I cannot say `rand` is their *only* channel to the bike. I can say the bike's words that
  part are the noise-driven hold fraction, and that the emitters draw `rand` every frame.
- **The models leaf is shown inert on these two replays only.** Its list at `+0x228` attached nothing
  on C15 (20 instructions a frame), so its attach path (`0x0013edb0`) never ran.
- **The leaves are keyed by step, not by listener.** The 11 emitters' addresses are not pinned, and I
  did not record which group each hangs under. Both are state 4's layout.
- **The noise is built mid-ride only because state 4 never built it.** In a session that already built
  the noise, the hold would not draw `rand`, but the out-of-bounds throw and the rider's reset still
  would. Not checked against a capture with the noise built.
- **`TreeInRam`'s narrowed check** applies only in the dropped modes. In the every-port tests it is
  still the whole RAM.

## Next

F31b5n: port the effects leaf and the two emitter kinds (or at least their `rand` draw counts with
the state that decides them), then run `flip_needs_the_part_walks_effects` with them Rust's.
