# 1 — hand reset position (FINAL)

Task S66. 2026-10-21.

## What I did

- **The put-back record** (`crates/game-sim/src/ride.rs`):
  - `Back { at, facing, up }` is a new field, `Ride::back`. It is in the hash walk (`back.at`,
    `back.facing`, `back.up`) and in the snapshot wire.
  - `Ride::keep_back(probe, heading, world, curve)` is the original's record update.
  - `clear()` is its clearance ring.
  - `flat_unit()` flattens the heading.
  - `Ride::place_back()` / `place_on()` set the bike down on the record, with the record's up as the
    look-at's up.
  - `Ride::frame` keeps the record after the air step. Its probe is the sweep point (the held transform
    at the top of the frame applied to (0,3,0,1)) at the body centre's height, and its heading is the
    node's row 2. A bare `Ride::frame` respawn now uses `place_back`.
- **The rider** (`crates/game-sim/src/rider.rs`):
  - The chord now runs before the physics, as the original's input slot does. It places with
    `place_back`.
  - The machine's reset (the wreck clock) also places with `place_back`.
  - While the rider is down, the record is kept off the wreck body: its centre is the probe and its
    forward row is the heading. In a cannonball the record is not updated (see below).
- **`CurveTable::coarse`** (`camera.rs`) is the table's `{cos, sin}` entry with no correction, which
  the ring is fanned out by.
- **The shell** (`crates/openmx/src/main.rs`):
  - A put-back tick is a cut: the picture is drawn at the new state, not blended from the tick before.
    `Ghost::put_back()` (`ghost.rs`) says when.
  - Before this, the blend's 30 ft snap always fired, because a put-back went to the spawn. Now the
    bike lands a few feet from the wreck, so the bike would slide, and the rider would morph from the
    ragdoll's bones into the riding pose over one tick. That is the "at first drawn in the wrong spot"
    of the user's note.
  - The console line now says "put back" rather than "back on the start line".
- **Captures** (`capture/putback_watch.py`): each tick it logs the record, the applied point, the
  node's rows, the sweep record, the ragdoll part and the race manager's heading, all as bits.
  - Scripts: `ride`, `down` and `downchord`.
  - Five captures in `context/s66/`, not committed:
    - `free06-ride2.txt` (slot 6): two chord put-backs while riding.
    - `free06-down.txt` (slot 6): a wreck and the clock's reset, then a chord.
    - `free06-downchord.txt` (slot 6): a chord while down.
    - `free09-ride2.txt` (slot 7, recording2's state): two chord put-backs while riding.
    - `free09-down.txt` (slot 7): a wreck, the clock's reset, then a chord.
  - `free06-ride.txt` and `free09-ride.txt` are earlier captures from before the script logged the
    sweep, part and heading bits. Nothing reads them.
  - RAM images: `./kit state ram 6` and `./kit state ram 7`.
- **Tests** (`crates/game-sim/tests/ride.rs`):
  - `the_put_back_record_is_kept_bit_for_bit` and
    `every_captured_put_back_lands_where_the_original_put_the_bike` are new (below).
  - `the_reset_chord_is_the_originals` part 4 now asserts the put-back lands where the bike rode, not on
    the start line.
  - Pins moved because the state gained three vectors: Sim 9784 bytes / 909 leaves, Ride 2416 bytes /
    631 leaves, HUNDRED_TICKS ×3, and the platforms.rs vectors spliced constant by constant.
  - The canonical run's POS_Y/VEL_Y moved because its chords now land on the floor at the record, not a
    foot up at the spawn.
- `research/oracle.txt` has an S66 block: put_back_keep, put_back_clear, put_back_solid,
  put_back_gate and hand_reset_thunk.
- Follow-ups filed: S69 (the rider's bones through the ghost) and S70 (the clearance against the
  original's meshes).

## What the original does

- **Order in a tick.** The chord handler, `./kit fn 0x0027bef0`, is reached through a thunk
  (`./kit dis 0x00281930`, this −= 80) in the actor's input interface. So it runs before the update.
  The chain is: ride_reset_request → player_respawn → bike_unwreck → vt+0x138 (`./kit fn 0x0027b650`)
  → ghost_start(this, 0). That places the node at +0x3b0 exactly, oriented by
  basis(+0x3c0, +0x3d0, orthogonalise, keep forward = 0), and copies +0x3b0 to +0x5e0. Unwreck also
  clears the ragdoll (rag vt+0x120(0)) on that tick. The physics then runs from there. After the
  physics, the race manager calls the record update.
- **The record update**, `./kit fn 0x0027e9d0` (bike, point, heading = race +0x120, the node's row 2):
  - The probe, in freeride, riding: the collision record's +0xe0 x/z/w (the sweep point, which is the
    node of the tick before applied to (0,3,0,1)) and the body centre's y.
  - The probe while +0x748 (reason) != 0: the ragdoll part's +0x40.
  - ground_query on the bike's terrain (+0x7ac).
  - One probe from 10 ft above the probe straight down, (y − ground) + 10 long. If it hits above the
    ground, nothing is kept.
  - If +0xc84 != 13, the clearance (`./kit fn 0x0027e530`) runs, and when it passes and the normal's
    y > 0.8, the record is kept:
    - point = spot + old up × +0x13a0 (that word is 0.0 on both tracks);
    - heading = the heading with y = 0, made unit by vrsqrt, w kept;
    - up = the normal.
  - The checkpoint copy +0x1350 is taken only in race modes.
- **The clearance**, `./kit fn 0x0027e530`. One bundle of 14 probes, radius 40:
  - probe 0 goes down from y+10, 20 long;
  - probes 1..12 are level at y+3, 20 long, along (z·sin + x·cos, 0, z·cos − x·sin) for angle
    ((i−1)·2π)/13. The (cos, sin) is the curve table's entry with no correction, after the two-step
    wrap;
  - probe 13 runs from 20 behind the flat heading at y+6.5, along it, 40 long.
  - Then ground_query moves the spot onto the ground and gives its normal.
  - The mesh query (`./kit fn 0x0027df50`) lets probe 0 lift the spot onto anything above the ground
    (taking its normal). A hit on any other probe refuses the spot.
- **The hand reset's own gates**, `./kit fn 0x0027bef0`:
  - +0x10d0, the chord latch +0x14d0, game mode 2, reason 13 (cannonball), and +0x116 outside mode 5;
  - the two commands 0x4c/0x4d, either one pressed while the other is held;
  - the owner's pad.
  - Race modes 1/2 first copy the checkpoint record into +0x3b0 when race flag 0x800000 is set.
  - In mode 3 the reset happens only when the player answers the name bit of "StuntCompetition" or
    "HitFreeride" (or 0x004e1df8 is clear); otherwise it calls `./kit fn 0x003a6dc0` instead.
- **The automatic (clock) reset** places at the same record. It was seen in both `-down` captures.

## How it was proved

All of the following is a save-state capture compared bit for bit.

- **`the_put_back_record_is_kept_bit_for_bit`**: on every captured tick where the original moved its
  record (and the tick is not the put-back itself), `Ride::keep_back` is handed that tick's probe and
  heading and the record from the tick before. The test asserts that the record's point and up come out
  identical to the bit.
  - The ground is the original's own ground_query, run in the interpreter on the save state's RAM
    against the bike's terrain (`TheOriginalsTerrain`).
  - A row is read while the game runs, so it can hold this tick's sweep with the tick before's record,
    or the race manager's heading from either side of the physics. The probe is therefore taken from
    this row or the one before; it must be the one whose x/z is the record's. The heading comes from
    the race manager's copy or the node row, on either row.
  - Counts per capture (ticks kept bit for bit / held / probe not in any row):

    | Capture | Kept bit for bit | Held | Probe in no row |
    |---|---|---|---|
    | free06-ride2 | 434 | 103 | 0 |
    | free06-down | 463 | 74 | 0 |
    | free06-downchord | 292 | 66 | 1 |
    | free09-ride2 | 380 | 125 | 12 |
    | free09-down | 406 | 115 | 13 |

    That is 1975 ticks in total, including the down ticks. The heading matched a row on every one of
    them but one.
  - The test asserts > 1000 ticks and > 50 down ticks, so a skip can't pass as a verdict.
- **`every_captured_put_back_lands_where_the_original_put_the_bike`**: on all 9 captured put-backs, the
  original applied the record as it stood before the press. `place_back` from that record puts the
  origin on the same bits and the node on the look-at of the record. The nine are:
  - chord while riding: free06 3047, 3176; free09 2631, 2758; free06-down 3112; free09-down 2706;
  - chord while down: free06 3015;
  - clock reset after a wreck: free06 3059, free09 2663.
- `./kit check` passes with no failures on the final run. Earlier runs had only the known load flakes
  (netplay four_peers, udp two_builds).

## Not verified / not 1:1

- **The captured pad is not ridden in the port end to end.** The brief asked to replay the same pad in
  the port. The port's ride is not bit-exact with the original over thousands of ticks, so a whole-run
  replay would not reach the put-back on the same tick at the same spot. Instead, the tests hand the
  record update and the placement the original's own inputs, tick by tick.
- **The bike's state after the placement tick.** After the placement, one tick of physics runs, and its
  yaw differs from the original's by 0.1–0.3° riding and ~0.85° down. That belongs to the physics and
  sweep ports, not the put-back.
- **The rider's pose through P57's ghost is not compared frame by frame.** There is no capture of the
  rider's bones. What is done:
  - the ragdoll and the crash are cleared on the put-back tick, as unwreck does;
  - the shell no longer blends across it.
  - Whether the original's body actor shows the riding pose on that first tick is not measured. Filed
    as S69.
- **The clearance against solid things is not compared.** The tests' world answers `solid` with
  nothing, so ticks the original refused (a roof overhead, a ring probe meeting a mesh or prop) are only
  counted as "held", not compared. In the game, `World::solid` is the port's own props sweep, not the
  original's mesh query. Also not ported, because the tracks don't have them or they are not on the
  freeride path:
  - the type-3 groups inside 20 ft;
  - the checkpoints in mode 1;
  - the broad-phase cache on +0x1390 / +0x13a4.

  Filed as S70.
- **The bounds-table gate** (`./kit fn 0x0027e410`) is not ported. Its table can't match points on
  these tracks, and it passed on both.
- **The cannonball branch (+0xc84 == 13)** is not ported. It follows a ragdoll flag (*(rag+0x120)+0x40)
  that the port doesn't model. With the flag clear, the original keeps the record unchanged, which is
  what the port does during a cannonball. Also, +0xc84 lingers after a reset in the original (it still
  read 11 forty ticks after free09's clock reset), while the port keys off the live wreck reason. That
  only matters for the cannonball case.
- **Race modes are not ported**: the record's race-mode probe (the point handed in, with no ground
  query), the checkpoint copies, and the mode-1/2 hand-reset branch. Also not ported: mode 3's
  name-bit test and its other branch (`./kit fn 0x003a6dc0`), because the port has no game types. Every
  capture took the reset.
- **The down probe is the wreck body's centre.** The original reads the ragdoll part's point. The port
  has no separate part point, so it uses the wreck body's centre (`Crash::body.cofg_world`) and the
  body's forward row. The bit-exact test feeds the captured part point, so the port's own wreck-body
  point is not proved equal to it.
- **The riding probe ignores the sweep's push-back.** The original's sweep point is the node of the
  tick before applied to (0,3,0,1), and the port computes it that way. That matched 524 of 539 ticks in
  free06-ride2. The 15 misses are ticks where the vehicle sweep pushed the point back (the standing
  start, the ticks after the put-backs, two bumps). The port has no vehicle sweep-back, so on those
  ticks its probe differs.
- **The riding probe's height** comes from the capture's 4-decimal body centre, not its bits. That
  height only starts the ground query, and every compared tick came out bit-exact anyway.
- **The shell's ground is the port's terrain, not ground_query.** In game, the record's y can be an ulp
  off the original's, as with S41's biplane ground.
- **Chord and gesture order**: the chord now runs before the gesture read, so on the chord tick the
  gesture reader sees the rider as not down. The original's order between the two inside its input
  phase was not measured.

## Next

None. S69 and S70 are filed.
