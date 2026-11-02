# 1 — backflip rider side

Task S223. 2026-11-02.

## What I did

- `handling::step::flip_wreck_steep`: SLUS-21104's `atv_bike_flip_wreck_steep` (0x004e8370) as its own
  function. MD63c had already folded the same predicate into `wreck_test`'s reason-15 clause, which now calls
  this function. `handling::ride_backflip` takes a `steep` closure. The ride gives it the struck class (the
  port's world has none, MD63c), `air.airborne`, and the frame's g loads so far (`mv::Ride::g_peak`, as
  `+0x1250` stands before this substep's peak rates run). A yes latches `Flip::struck`, and the rider enters
  `Wreck::Struck` (0xf) for it instead of `Wreck::Tail` (0xb).
- The rider's stunt id (`Flip::stunt`, wired and hashed). `ride_backflip` now does what the start and end
  calls and the id clear do:
  - the clip's start sets it when no trick holds the word;
  - a trick held clears it (a trick's start writes its own id);
  - the flip's end clears it (an id over 40 goes to 255).
  `handling::stunt_stand_in` is the one stand-in for the id by range. Both `ride_backflip` (its `FlipIn::stunt_id`,
  0 before) and `ride_airborne_control` use it. The air control now reads the backflip's stunt from the clip's
  start to the flip's end. Before, it read the clip running, which stops when the stunt comes round, before
  the landing. So in that gap the original's gyros and lean force stay off (id 41) and the stick goes in unscaled.
- Tests:
  - `tests/mxvatv_air.rs` `mxvatv_flip_wreck_steep_is_the_originals`: 10,000 states. The original runs
    unhooked on state04: record null or touching or not, object null, class word vs `*0x00542db0`,
    airborne, and loads on and around the three limits. Every path is hit ≥ 200 times.
  - `mxvatv_start_stunt_always_starts_and_keeps_a_trick`: the original's start runs on 2,000 states with
    no animation objects. It answers 1 every time and writes `+0x1528` only from an id ≥ 41.
  - `tests/mxvatv_ride.rs` `the_backflips_stunt_and_its_mid_clip_reason`: the ride-level wiring.

## What the original does

- `atv_bike_start_stunt` (0x004e6370, a0 bike, a1 id, a2 1) returns 1 on every path. So the port's "always
  accepted" was the original's answer, and the clip always starts. Its work is as follows:
  - A flip id is one of `+0x13b8..+0x13c8`, the five flip stunts; the backflip's is `+0x13b8`. If the id is
    not a flip id, it is written to `+0x1528`. If it is a flip id, it is written only when the current id is
    ≥ 41 and the id is not one of the 360's pair (`+0x13c4`/`+0x13c8`).
  - On the ground (`+0x324`) or with `+0xc80` set, a non-flip id returns 1 at once.
  - It zeroes `+0x14d8`/`+0x14dc`/`+0x14e8` and sets `+0x1650` = 1.
  - With the rider's animation objects (`+0x680`/`+0x684`) it blends to and plays the clip (0x0046aae0,
    0x0046b110, fade 12). Non-flip ids first end the stunts queued on the list (vt +0x4ac).
- `atv_bike_end_stunt` (0x004e4db0) only stops the clip on the animation objects (0x0046acf0, or for id -1
  0x0046b4c0 + 0x0046a760). It never touches the id or a score.
- `+0x1524`, the word the air control and wreck test read, is copied from `+0x1528` once a frame at 0x004dd9cc,
  unconditionally, in a per-frame rider function (the quad has its own copy at 0x004be4e8). The unwreck
  (0x004e23e4) sets both to 0xff.
- 0x004e8370 decoded: record `+0xc8` → `+0x34` → `+0x20c` → `+0x94 & *0x00542db0`. Then `+0xde0` gives yes.
  Otherwise yes iff `+0x1258 > +0x13ec`, `+0x1258 < -+0x13e8`, `+0x1250 > +0x13f0` or `+0x1250 < -+0x13f0`.

## Not verified / not 1:1

- **Scoring.** I found no score call on the backflip's path. Start, end, the set-clip call and the event posts
  that read `+0x13b8` (0x004e72f4…, category 8) do not score. The port's backflip is still scored by MX
  Unleashed's rider watch (`score::Score::frame`), as on Original. Whatever MX vs. ATV scores its backflip on
  (likely the stunt system off `+0x1528`) is not found.
- **The clip.** The port's rider is MX Unleashed's. Its backflip clip plays off `air.flipping` (V35), not off
  MX vs. ATV's start and end calls, and MX vs. ATV's animation system (fade 12, its blend) is not ported.
- `+0x1524` lags `+0x1528` by up to a frame (the copy is once a frame, phase against the physics not read).
  The stand-in has no lag: the air control sees the stunt on the substep the clip starts.
- "A trick's start takes the word" is inferred from the start function writing every non-flip id. The trick
  machine's own MX vs. ATV start call was not traced.
- The ride's steep test always answers no, because the struck class is never true in the port (MD63c), so the
  mid-clip wreck is still 0xb in practice. The 0xf path is proved on the function and wired, not ridden.
- `start_stunt` was run only without animation objects. The animation path's return (also 1) is read from the
  disassembly, not run.
- `cargo check -p openmx --tests` fails on HEAD over `Heli::touching` (P216a). This is not this task's change:
  filed as B11 (critical).
