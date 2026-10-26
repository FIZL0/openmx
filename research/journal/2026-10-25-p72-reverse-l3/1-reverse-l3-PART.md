# 1 — reverse l3 (PART)

Task P72. 2026-10-25.

## What I did

The brief changed twice (L3, then the user's 2026-10-22 note). The note is what this did: no reverse button
under either layout. Reverse is the original's own rule. Under New you brake to a stop, keep L2 held and
press R2; under Original you hold Square and Cross. The rider backing up plays the reverse pair.

- `crates/game-sim/src/ride.rs`: `Controls::reverse_rule(speed, airborne)` and `REVERSE_BELOW = 5`.
  - This is the tail of the original's pad read, run at the top of `frame_bumped` on the levers the pad gave.
  - Its reverse command lands in a new `Held::reverse`, the original's `+0x648`.
  - The rest of the frame (the reverse push, the request) reads the ruled controls.
- `crates/game-sim/src/rider.rs`: the riding request's `reversing` (`+0x16c`) now reads `ride.held.reverse`.
  - It used to read `Controls::reverse`, which no pad has set since the R3 binding went (P73/P114).
  - That was the whole reason the drawn rider never played the pair. The clips were loaded (`Body::load`
    binds them with the pose table) and the draw takes `Blend::Two`, but the machine never asked for 33/34.
- `crates/game-sim/src/snapshot.rs`: `Held::reverse` goes on the wire so a rollback keeps it.
- `crates/openmx/src/main.rs`: the module doc and `--help` say how to reverse.
- Neither layout has a balance button or a reverse button any more (`from_pad` already had neither, and R3
  is P114's camera reset). New's L2/R2 reach the rule through the shell's remap onto the game's Square/Cross.
- P77's question (switches vs. analogue gas) is the user's and stays open. Only the reverse rule was ported.
  Where the original would set gas to the switch value 1.0 (in the air, both held at speed), the port keeps
  the lever's strength.
- Taken from opentennis: nothing; it has no reverse.

## What the original does

- The tail of the pad read: `./kit fn 0x00250fb0`. When not wrecked (`+0x748 == 0`):
  - brake `+0x640` ≤ 0 or gas `+0x63c` ≤ 0: `+0x648 = 0`.
  - else, speed `+0x438` ≥ 5: `+0x648 = 0`, gas = airborne (`+0x57c`) ? 1 : 0, and the brake stays.
  - else: `+0x648 = 1`, gas = brake = 0.
  - The wreck branch zeros the levers and leaves `+0x648` alone.
- At speed, both held is a pure brake. Slow, it is reverse with both levers let go.
- The push: `./kit fn 0x00257a00`. It is gain `+0x548` × (ReverseSpeed `+0x544` − speed), applied only with
  the rear wheel down. ReverseSpeed is the .phy's mph × 1.46666, and every disc bike says −10.
- The request's `+0x16c` copies `+0x648`. The body machine wants 33 or 34 off it and plays the pair at its
  own clock, `+0xa3c`, which advances by 0.03 × |speed| × dt and wraps.
- Captured from the original: `watch/p72-reverse.toml`, `capture/capture.sh`, `capture/back.pad`,
  `capture/setup.py`. These are free09 cr125 from `first_recording`, at a quarter speed, with Cross+Square
  pressed from a standstill. The CSV is `context/capture/p72/reverse.csv`, 300 rows, not committed.
  - `+0x648` rises on the row the levers do. Gas and brake read 0 while it is up.
  - From the next row the body wants and runs 34 (REVERSE_R; the bars are centred) the whole way back.
  - The clock winds by 0.0128 a tick at −12.77 ft/s (0.03 × 12.77 / 30 = 0.0128).
  - The bike settles at −13.85 ft/s.
  - On release `+0x648` drops to 0, and the state is back to 30 the tick after.
- Notes are in research/oracle.txt under "P72". The `+0x16c` line there said "no pad sets it"; that is
  corrected.

## How it was proved

- `crates/openmx/tests/ride.rs` `gas_and_brake_held_slow_back_the_bike_up`:
  - Both layouts (New R2+L2, Original Cross+Square) through the shell's own pad path, from a standstill
    and from 3 s of gas.
  - `held.reverse` is 1 exactly when the speed before the frame is < 5. Brake alone never reverses.
  - From a standstill it backs up past −1 ft/s (about −14).
  - From speed it brakes first and reverses only once under 5 (frame 98, from 40.6 ft/s).
  - The two layouts are bit-identical.
  - The test bike's ReverseSpeed is patched to the disc's −10 (see below).
- `crates/openmx/tests/reverse_clips.rs` `backing_up_the_rider_plays_the_reverse_pair` (disc 250sx and
  thq1 rider, the shell's two calls a frame):
  - Gas+brake held for 240 ticks at a standstill backs up to −14.03 ft/s.
  - Every tick the rule holds, and the body wants REVERSE_R (34) with the bars centred, as the original.
  - From tick 2 the body runs it, and the frame hands the draw `Blend::Two { right, left }`.
  - The left foot sits 0.76 ft off standing and swings 0.71 ft, so the clock runs.
- `crates/game-sim/tests/replay.rs` S68 `the_captured_backwards_turns_never_wreck`:
  - It no longer overrides the levers and reverse with the captured ones. The pad goes through `sim.poll`
    and the new rule.
  - Its `held.reverse` matches the captured `+0x648` bit for bit on every fresh lever row: flat 327/327
    (299 reversing), slope 355/355 (192 reversing).
- `crates/game-sim/tests/platforms.rs`: the canonical run's random buttons hold gas and brake together
  while slow, so its pinned vectors moved, as they are meant to when the ride reads more of its input.
  The consts were regenerated with `print_the_vectors` / `print_the_riders_vectors` and spliced in.
  `FINAL_DUMP_LEAVES` and `EVERY_LEAF_HASH` stayed put, because the hashed state type did not change.
  `RIDERS_WIRE_LEN` grew by 16 bytes, which is 4 riders × the new `Held::reverse` on the wire.
- By eye: the original's `./kit shot` while backing up (`capture/shots.sh`, `context/shots/p72/orig-back-b.png`)
  beside the port's (`capture/port-tape.py`, `context/shots/p72/port-back.png`).
  - Both riders sit with both legs down beside the bike, feet near the ground, not on the pegs.
  - The port's bike also backs up at 9.4 mph ≈ 13.8 ft/s on free09, next to the original's 13.85.

## Not verified / not 1:1

- **Gas and brake are still strengths, not switches** (P77, the user's open question). In the air with both
  held at speed, the original sets gas to 1.0; the port leaves the lever's strength. On the ground and below
  5 the rule is exact.
- **The body machine's throttle reads the raw pad gas**, not the ruled one: `rider_anim` gets `c` from the
  shell, not the post-rule controls. While reversing the original's gas is 0. Not checked whether anything
  visible reads it in states 33/34.
- **One tick of phase**: in the capture the body's want/state switch the row after `+0x648` rises. The port
  wants it the same tick and runs it the next. This is probably the capture rows' read phase
  ([[capture-rows-read-at-three-phases]]); not proved.
- **The stock bike (`stock::BIKE`) has ReverseSpeed +4**, which pushes forwards. Every disc bike has −10. The
  ride.rs test patches it, and real rides load the disc's .phy. stock::BIKE was left alone, since it is
  outside the rule this task ports; filed as S122.
- **S68 flat ratchet lowered from 330 to 213** (slope raised from 89 to 258). Main already parts at flat tick
  214 with the captured levers (proved on HEAD in a scratch worktree), so it is not this task. Filed as S121.
- **`Held::reverse` is not in the hash walk.** It is on the wire, but a desync in it alone would not show in
  netplay's checksum until it moved the bike.
- **The riding ragdoll path** (`OPENMX_RIDING_RAGDOLL=1`) was not tried with the pair.
- **The shot comparison is by eye**, from one frame each, on different bikes (the original's cr125 from the
  state, the port's default 125sx). The port's arms reach wider to its grips than the original's; not
  chased.
- **One capture**: one bike, one track (free09), straight back with the bars centred, so REVERSE_L (33,
  steering one way) was never seen in the original. The port's left/right pick is its existing
  `rider_anim` code, untested here.
- **The regenerated platforms.rs vectors are this machine's only.** tests/cross/platforms.sh was not run,
  so no other architecture has confirmed them.
- The capture ran at a quarter speed. Its NominalScalar only slows the clock, but that was not proved
  against a full-speed run.

## Next

The work is done and committed on task/P72, but **not ticked**. `./kit done P72` reds on one lane only:
replay.rs `the_dallas_rail_rides_replay_from_their_save_states` ("rider.ride.front.slip.locked agrees
through tick 278, was 279"). It is S120, red the same way on bare HEAD 46830197 in a scratch worktree
(2026-10-25), and only s3 has S89's data to see it. task/P72 is committed locally, not pushed (the run says keep changes local).

First move: once S120 lands, rerun `./kit done P72 "<msg>"` in a slot with S89's capture, then rename this
file FINAL. Also filed: S121 (the flat S68 ratchet) and S122 (stock::BIKE ReverseSpeed). P77 owns the
switches question.
