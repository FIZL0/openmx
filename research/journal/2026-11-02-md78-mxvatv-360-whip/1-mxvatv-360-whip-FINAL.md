# 1 — mxvatv 360 whip (FINAL)

Task MD78. 2026-11-02.

## What I did

- Measured a 360 in the `whip_tricks` harness (thq1 on the 250sx, R1 + left-right flick off the
  `Ramp`). Body and bike were both in `SPIN_RIGHT` for ~85 frames, and the clock ran the clip end to
  end. But the drawn pelvis moved 0.2 ft and the feet 0.35 ft. Dumping the clips showed what
  that was hiding:
  - `MX_ThreeSixtyR.mot` (v7, 45 frames, 1.467 s, 18 joints) writes a Pelvis position. It goes from
    (0, 4.29, -1.16) to (-0.8, 3.5, 0) at 0.6 s and to (-1.2, 3.29, -1.6) at 1.0 s, then back to
    (0, 4.2, -2.2). The pelvis turns up to 60°, the legs to ~100°. That is the body thrown out
    sideways, off the seat.
  - `Bike_ThreeSixtyR.mot` (same length, 6 nodes) keeps the root still and turns `Handlebars` up to
    72° at 0.7 s. That is the bike's half of the whip: the bars turn to follow the rider's hands.
- The cause: the 360 states restart at `rate_stunt` (4.0, the gesture slots' fade *rate* per second;
  MD21 gave it to them). Then they go down the played-once arm, where the same number is handed
  to `Player::play` as a cross *length* in seconds. A 4 s cross under a 1.47 s clip: the `fade` was
  still on when the clip ended.
- Fix (`game-sim/src/vehicle_anim.rs`, played-once arm): for the two 360 states the cross is
  `1 / rate`, i.e. the gesture rows' own 0.25 s. A held request's zero rate is still a hard cut. The
  wreck-and-hold and start-line groups are untouched: their rate is the original's and their length
  is the original's quirk. The bike's machine is the same `Driver`, so `Bike_ThreeSixtyR`'s bar turn
  gets its full weight too.
- After: pelvis out to (-1.22, 3.28, -1.53) and feet 2.35 ft off the pegs mid-clip. The hand holds
  stay at 1 all through. Feet back on the pegs (≤0.02 ft) once the airborne row has them.

## What the original does

- The 360 step (`./kit fn --program pc 0x00600790`; PS2 `0x4e53d0`) plays threesixtyL/R through the
  vehicle's play slot (`./kit fn --program pc 0x00600fa0`) once it is past 35°. It stops the clip
  (`0x005fb140`) when it is back under 35° in clip state 2.
- The play makes a layer on the vehicle's animation controller (`./kit fn --program pc 0x00498750`)
  with a fade-in and fade-out each `record+0x50/+0x54 × record[1]`, clamped ≥ 10. The record comes
  from `0x00551270` (id < 0x2e). The layer stores both as integers: milliseconds, by the ≥ 10 floor.
  The record values live in the running game, not in the exe or the pack ini (`MX_Rider_Stunts.ini`
  has HoldTime 0.72 / SafeTime 1.26 / Duration 1.50 and no blend keys).
- Bike and rider each get their own clip (`UsedForBike = T`), started together.

## How it was proved

- `crates/game-render/tests/whip_tricks.rs::a_360_throws_the_rider_off_the_pegs` checks:
  - rider and bike both in threesixtyR for > 60 frames;
  - the drawn pelvis > 0.8 ft to the side and > 0.5 ft down off its seat;
  - feet > 1.5 ft off the pegs;
  - hand holds at 1 all through;
  - feet back within 0.05 ft half a second after.

  It fails on the old 4 s cross (the pelvis swings 0.17 ft) and passes with the fix.
- The rest of `whip_tricks` (V64/V69/V98/V100 hands-on-bars and bike-with-rider), `pack_clips`,
  `trick_chain` and game-sim `ride` (67) plus the `vehicle_anim` unit tests all pass.
- Looked right beside the V69 film. `./kit shot`-style headless shots of free06's first lip, shot
  through `context/shots/md78/spin.tape`, frames 254–314, are in `context/shots/md78/`:
  - `before-grid.png`: the rider sits square through the spin;
  - `after-grid.png`, `after-swing-zoom.png`: the rider flung out sideways, legs off, hands on the
    bar, and back on by the landing.

  Compared by eye with `context/shots/v69/mxvatv-360-whole.jpg`.

## Not verified / not 1:1

- The 0.25 s cross in is chosen, not read. The original's fade-in and fade-out come from a runtime
  record (milliseconds, ≥ 10). Reading it would need the record table out of a MX vs. ATV RAM image
  at the PS2 equivalent of the PC lookup.
- The fade *out* is unchanged. The port leaves the 360 when its clip state drops (the body returns
  to the airborne row at `rate_pose`), while the original stops the layer at the record's fade-out.
  Not compared frame by frame.
- The original may hold or loop the layer (it sets layer flag 0x20 after the play). The update that
  reads that flag was not found. The port plays the clip once, and a 360 that runs longer than
  1.47 s replays it (MD36).
- The shots are by eye against a film of the original on a different track and camera. The swing's
  timing against the bike's yaw is not measured against the original.
- The hands are held by the drawn hold (V69/V100), not by the clip alone. Where the clip's hands are
  off the turned bars, the hold bends the arms.

## Next

Nothing. Reading the original's fade record (see above) would make the cross 1:1.
