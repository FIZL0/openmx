# 1 — mxvatv trick hold window

Task S410. 2026-11-18.

## What I did

- On a trick-pack ride, a trick's node now grows only in MX vs. ATV's own hold window. Before (S358)
  it grew on any frame the port's trick clock counted as held, easing in included.
- `score.rs`:
  - `Score::atv_hold_marks`: which marks the original's trick step makes on a running stunt's node,
    in its order (on, then off).
  - `Score::atv_hold`: makes them.
  - `ATV_HOLD_EARLY` (0.3) and `ATV_HOLD_SLACK` (state 4's tuning word, 0x3d23d70a = 0.04).
- `rider.rs`: each frame a trick runs (not its start frame), the marks are worked out from the trick
  clock as it was **before** it ran, the stunt's HoldTime and Duration, the frame's dt, and whether the
  trick's own button group is held. A small helper `trick_group` gives a trick's group (slot or
  extra). The pack's fallback groups (MD87) count as the same buttons.
- Nothing taken from ../opentennis: it has no trick scoring.

## What the original does

The trick step (`./kit fn 0x004e49c0 -p atv`, already ported as `handling::update::trick` by F42).
For the running stunt, before its clock runs:

- It marks the node when the asked kind (low four bits) is the stunt's kind, HoldTime > 0, and the
  clock is in [min(HoldTime, 0.3), HoldTime + 0.04].
- If that held and the clock is at or past HoldTime, the clip is held there. Nothing else runs.
- Otherwise the clock runs on by dt. Unless the stunt is one of the kept five on a driven bike:
  a clock still under Duration lets the node go (mark off), and one at or past it ends the stunt.

So inside [0.3, HoldTime) the node is marked and let go in the same step, and does not grow. It only
grows on frames the clip is held at HoldTime, and stays marked until a frame lets it go.

## How it was proved

- `tests/score.rs` `a_pack_ride_holds_a_trick_in_mx_vs_atvs_window`: 20 000 generated cases (clock,
  HoldTime, Duration, dt, kind match, kept, driven), aimed at each edge of the window. The marks
  `atv_hold_marks` makes are compared in order with those `handling::update::trick` makes on a RAM
  model. All equal. Each of the three outcomes is hit over 500 times.
  - `handling::update::trick` is itself proved against SLUS-21104 on 10 000 generated cases
    (`tests/mxvatv_bike_ride.rs`, F42). So this is a two-step proof, not run in the Ee directly.
- `tests/ride.rs` `s410_a_pack_trick_grows_in_its_hold_window`: the pack's Varial on the flip ramp.
  Let go at once, it stays at 1000. Held, it first grows after tick 40 (the old way grew from about
  tick 19), then every tick until the release, at 1000 a second.
- `./kit check -- -p game-sim --test score --test ride --test bpb_stunts --test rotation_modifier
  --test rider_anim --test mxvatv_stunt_score`: all pass. MD25's ride hashes did not move.

## Not verified / not 1:1

- **The clock is still MX Unleashed's.** The window is read off the port's trick clock (its ease into
  the hold, its own [HoldTime, HoldTime + 0.1] stop). The original's clock may reach HoldTime on
  other frames. So the frames a node grows on are the original's rule, but not proved to be the
  original's frames. Needs S411's ride capture.
- **"Same kind" is the trick's button group.** The original compares the asked kind's low four bits
  to the stunt's kind byte. I read that as the button group, and count the pack's fallback groups
  as the same buttons (the original has no fallbacks). Not checked against a capture.
- **The gesture read** is this frame's. I did not check whether the original's asked kind is written
  before or after the trick step in its frame.
- **"Driven" is assumed.** The kept-stunt exception needs the bike driven (+0x1194 / +0x11d0). The
  rider passes "kept" alone. No trick id is a kept stunt, so this never matters on a ride.
- **The mark's id** is the trick's id. The original marks by the clip's id byte (+0x58). I assumed the two are the same.
- **The end.** The original ends a trick when its clock (plus dt) reaches Duration. The port's clock
  ends it when the clock passes Duration. On a frame where they differ, a node can stay marked one
  frame longer.
- **Not run in the Ee.** The proof goes through F42's port of the trick step, not SLUS-21104 directly.

## Next

Nothing. S411 (ride timing capture) is the check on the frames.
