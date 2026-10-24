# 1 — brake slope slide (FINAL)

Task S106. 2026-10-24.

## What I did

- **Merged origin/task/S58 into task/S106** (928e5374). S58 was finished and pushed on 10-24 but was
  not on main yet. S106 needs its watch file (`watch/s58-slip.toml`), its quarter-speed capture
  scripts, its save-state replay harness in `tests/replay.rs`, and its ride.rs fixes (see the
  findings below: one of them is S106's fix). Landing S106 lands S58's code too. The merge was clean.
- Recorded four braked-slope captures of `watch/s58-slip.toml` in the original at a quarter speed, in
  `context/capture/s106-slope/` (scripts in `capture/` here: `setup.py`, `tele.py`, `capture.sh`, the
  `.pad` files, `look.py`):
  - **nose**: `tele.py 0 -11.7 45 30 1` stands the bike on the 53° face ahead of first_recording's
    start (free09), nose down, with the brake held. Recorded from that slot with the brake held 30 s
    wall. RAM image: `context/states/s106-nose.eeMemory.bin` (`./kit state ram` of slot 1).
  - **side**: `tele.py 95 -2.7 -10 20 2`, the 46° face to the start's right, side on (falls to +x),
    with the brake held. RAM image: `context/states/s106-side.eeMemory.bin`.
  - **roll**: from first_recording's state, 1.25 s (game time) of gas, off, then the brake. It leaves
    the lip at 37 ft/s, flies 45 ticks and lands on the face at 64 ft/s with the brake held.
  - **roll2**: from first_recording's state, 0.5 s of gas, a coast to the lip (about 15 ft/s), the
    brake from just before the edge, held 7.5 s, then let go.
- `tests/replay.rs`:
  - `SaveState::open_at` opens any RAM image (`open` calls it).
  - `seed_levers` seeds the rate-limited levers off the rider.
  - `seed_engine_state` seeds the engine's gear, ratios, clutch scale and lock, revs and curve cursor.
  - `replay_a_save_state_case` is S58's test body as a function, with those two seeds, and a `late`
    list of ticks whose capture row is not compared.
  - `S106_CASES` and `the_braked_slopes_replay_from_their_save_states`: a ratchet per case on the
    first tick the path (chassis origin) and the slip flags part.
  - S58's own test is left untouched (it still passes with its own seeding), so later edits to it by
    S107/S112/S113 merge cleanly.
- Ran the four cases against **main's** `crates/game-sim/src`, in a scratch worktree with this test
  file, to find what the user saw. Then I added only S58's speed-source change to main's code (below).
- Oracle: an S106 block (the finding, plus the engine and lever words a state carries).
- Plan: S114, for roll's landing (S67's ground).

## What the original does

**The original's bike does not slide down a steep face with the brake held.** In every case it
stops and holds:

- **nose** (53°, brake held from the drop, 10 ft/s at the save): both wheels lock. The front unlocks
  at tick 4 and the rear at tick 8. It stops in about 20 ticks, then creeps at 0.024 ft/s (0.16 ft in
  160 ticks) for as long as the brake is held.
- **side** (46°, side on): it holds from the start, creeping at 0.04 ft/s. It does not slew round or
  tip.
- **roll2**: over the lip at 15 ft/s, a short hop. It lands at 25 ft/s on the face and locks a wheel
  at a time. The rear is locked for about 30 ticks. It stops 50 ticks after landing and holds for 150
  ticks at 0.036 ft/s. Let go, it rolls away to 40 ft/s.
- **roll** (64 ft/s landing): it brakes to a stop near the bottom of the face, about tick 160.

How it holds: the brake's torque fades to zero below 3.3 ft/s (`surface_drag`: `MaxBrakingTorque ·
clamp(0.3 · forward speed, ±1)`), so at a standstill the brake does nothing. What holds the bike is
the tyres' rolling grip. `tire::force` caps the drive force at `MaxDriveForce · load` (race125p: 2.0
front, 2.2 rear), and the locked skid force is 1.9/2.1 · load. Both are above tan 53° = 1.33, so a
race125p cannot skid down any face under about 62°. The creep is the tyre law's small residual slip.

**What the user saw.** Before S58, the port shed speed faster on the face and stopped sooner and
higher up:

- **nose** on main's code parts at tick 2 (8.66 vs 9.73 ft/s at tick 3) and stops 0.18 ft higher up
  the face.
- **roll2** on main brakes from 25 ft/s to a stop in 40 ticks where the original takes 50 (4.3 vs
  12.7 ft/s at tick 137).

That is "the port's bike stays put where the original's slides". The original does not slide
forever: it slides further before the grip stops it.

Two S58 changes close the gap:

- **The bike's speed** (`+0x438`, the air state's `+0x430.z`) is the touching wheel's view of the
  chassis (`+0x190`), a substep behind, not the chassis' own local z. The move step reads it for the
  stoppie, the clutch's launch scale and the steering fade. Main's code with only this change (the
  `forward_speed` line and the two `astep.*_vel` lines) runs nose bit-exact through tick 12. So this
  is S106's fix.
- **The levers only rise** (S58). On main a released gas walked down, so roll2 parts at its gas
  release (tick 31), before the brake.

S58's journal (`2026-10-23-s58-slip-cases`, its "The limiter" line) calls `+0x650` the gas. It is the
brake (front). `+0x654` is its rear copy, `+0x658` the gas and `+0x65c` the clutch, as
research/oracle.txt already says (bike_input_limiter, `./kit fn 0x251a50`).

## How it was proved

- Save-state captures replayed bit for bit: `crates/game-sim/tests/replay.rs`,
  `the_braked_slopes_replay_from_their_save_states`, four captures (nose and side 200 ticks, roll
  240, roll2 421). With this branch's code:
  - The slip flags (each wheel's spinning, locked, sliding) agree on **every tick** of nose, side and
    roll2, and on roll through tick 112.
  - The path agrees bit for bit through tick 27 (nose), 76 (side), 61 (roll) and 73 (roll2). After
    that it drifts by ulps (S107's drift); roll2's speed is still within 0.0001 ft/s while the bike
    stands on the face at tick 300.
- The same test against main's src (scratch worktree) shows nose parting at tick 2 and roll2 at
  tick 31. With only the speed-source change added, nose is bit-exact through tick 12 (the trace
  stopped there).
- `./kit check -- -p game-sim --test replay`: 82 tests pass, S58's `the_slip_cases_replay_from_the_save_state`
  included, with the new seeds only in S106's helper.

## Not verified / not 1:1

- **Not bit-exact over the whole ride.** The path parts by ulps at tick 28/77/62/74. The first
  parting lanes are the chassis' angular history and quaternion at tick 1–2, by an ulp or a few: the
  drift S107 is filed for. The ratchets hold these ticks; they do not prove the rest of the ride bit
  for bit.
- **roll's landing parts.** At the 64 ft/s touchdown (tick 107) the springs are 321 in the original
  and 484 in the port, and the slip flags part from tick 113. Landings are S67's; filed as S114. Not
  chased here.
- **nose's tick-1 row is not compared** (`late: &[1]`). The recorder read it a tick late: it equals
  tick 2's row on the path. The port's tick 2 equals it bit for bit.
- **Which mv reader of the speed made the nose case part** (stoppie, clutch scale or steering fade)
  is not pinned down. The one-line source change is proven to close it, not the mechanism inside the
  move step.
- **Only race125p on free09's dirt** (surface 13) was recorded. Other bikes, tyres or materials might
  slide in the original where a face is steeper than their grip; the tan⁻¹(1.9) ≈ 62° limit is read
  off the tyre law, not recorded.
- **What the user rode is not known.** If they saw a slide on a face steeper than about 62°, or with
  another bike, these captures do not cover it.
- **Teleported states.** tele.py moves the bike in RAM and lets 20–30 frames run before saving, so
  nose was still settling (10 ft/s) when saved. That is a real ride state, but not one a human rode
  into.
- **Captures at a quarter speed** (NominalScalar 0.25), as in S58. A full-speed run was not
  compared.

## For S58 / S113

- S58's merged code is what fixes S106. Nothing new is needed in tire.rs, wheel.rs, move.rs or
  ride.rs.
- S113 (a held traverse across a steep side slope): `capture/tele.py` makes such a save state (the
  side case's `95 -2.7 -10` is a 46° face falling to +x). Add a held-traverse pad to `capture.sh`.
  `replay_a_save_state_case` seeds the levers and the engine, which a teleported state needs.

## Next

Done here. S114 for roll's landing; S107 for the ulp drift that ends the path ratchets.
