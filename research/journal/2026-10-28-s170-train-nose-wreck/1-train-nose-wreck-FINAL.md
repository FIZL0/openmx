# 1 — train nose wreck (FINAL)

Task S170. 2026-10-28.

## What I did

- **Captured the original's train nose meeting a standing bike** (copy 2, quarter speed,
  `research/s170/nose.py`). Starting from S129's across state (slot 10), I paused and teleported the
  bike to (4060, 139, 2511.3), on the rails of the z≈2511 straight. The train runs +x there. The
  teleport keeps the state's heading, about 54°, so the train comes from 36° off the bike's tail. I
  held both brakes. The log, one line per frame, is `context/s170/nose1.log`. Slot 15 holds a state
  60 frames after the teleport (frame 16423), kept as `context/s170/stand.p2s`.
- **Mapped the train's loop** in the original (`research/s170/loop.py`, `context/s170/loop.log`): the
  engine's clock and position every 10 frames, round a whole lap (about 125 clock units). It has a
  straight at 230° from (3243, 4500) to (3060, 4346), clocks 108.6–112.3.
- **Captured the head-on case** with the same script (`context/s170/nose2.log`; slot 16, kept as `context/s170/headon.p2s`). The bike
  was put on that straight at the engine's own position at clock 110.85, (3130.03, 139, 4405.13), still
  facing 54°, so the nose meets it 4° off head-on.
- **Pinned both answers in the port's `crates/openmx/tests/train.rs`:**
  - `a_bike_standing_on_the_rails_is_thrown_by_the_trains_nose` (head-on, heading 10 on the 190°
    straight) now asserts that the first wreck is 1. It used to accept 1 or 6.
  - New, `a_bike_the_nose_meets_from_behind_is_carried_into_a_scenery_wreck`: the same spot at heading
    154, so the train comes from 36° off the tail as in the capture. It asserts that the first wreck is 6.
- **No change to the simulation.** S151 already names the car as the mover, and the capture confirms
  that. The task's "if 1, name the car as the mover" was already done, and the port gives the
  original's reason in both cases.
- From opentennis I took nothing: it has no trains.

## What the original does

- The across state's bike is **not a ghost**: `+0xb7c` is 0 on every frame. Its search (`+0x7bc`)
  holds 36 candidates, 23 of them category 0x48 (the 21 cars and two toys). So the cars are in its
  scene, unlike S129's braked state.
- Frame 16643 (engine node at x 4024.8, the bike at 4059.5) is the first contact. The bike's
  collision record (`bike+0x7d0`) holds own entries (kind 0) with **other `+0x74` = 0** and
  **mover `+0x78` = the engine** (0x00d525f0 in this state). The entries reach 1.55, 3.64, 3.04,
  4.58 and 1.45. This is S151's reading, seen live.
- Over frames 16643–16649 the train pushes the bike from 0 to 19, 28, 31, 42, 45 and 49 ft/s with
  no wreck. On frame 16650 it enters **reason 6** (scenery). The bike then flies on at
  (42.1, −1.9, 11.5) ft/s and comes to rest at frame ~16963, at (4131.9, 134.6, 2528.4).
- **Head-on, the original enters reason 1** (fore-and-aft, over the bars) on the very frame the nose
  first touches the bike (18775, with the engine node 33 ft from the bike, as in the other capture).
  The bike is thrown at (−39.3, 1.2, −28.5) ft/s, the way the train goes. So which reason the nose
  gives depends on how it meets the bike: head-on is 1, and from behind the bike is carried and then
  struck on the head ray, 6. Both are in `research/oracle.txt` under S170.

## How it was proved

- **Two live captures** of the original, as above. In each, the per-frame log of `+0x748` shows the
  reason and the frame it was entered. The record's `+0x74`/`+0x78` are read on the contact frames
  (frames between them read an empty record: PINE reads land between ticks).
- **The port** rides the same two geometries in the shell (headless free06, `--trace-at`):
  - heading 10 (head-on) gives 1, and so do 6 and 14;
  - heading 154 (36° off the tail) gives 6, and so does 149.8.
- `./kit check -- -p openmx --test train`: 4 passed.

## Not verified / not 1:1

- **The port's two answers are checked at a different place, and at a different time on the train's
  clock, from the original's.** The original was captured at (4060, 2511) and (3130, 4405). The port's
  tests ride the shell's own spot, (2385.8, 3779.9) on the 190° straight, where car 0 arrives about 50
  ticks after the start. They match only the angle between the bike and the train (and the bike on the
  line). Riding the port at the captured places would need the shell's train clock moved on 1,700+
  ticks, which a headless debug test cannot afford.
- **Only the reason is compared, not the ride.** The original's from-behind bike is carried for seven
  frames, reaching 0 → 49 ft/s, before its 6. I did not compare the port's tick count or speeds before
  its 6, nor the head-on throw velocity. A bit-exact replay would need these captures re-taken as
  `./kit record` captures from the saved states (slots 15 and 16; `context/s170/stand.p2s` is the
  from-behind one), plus S186's wagon contacts ported.
- **The angle in between is unknown.** At 157.8° (32° off the tail) the port's bike is shoved aside
  and does not wreck within 200 ticks. I captured the original only at 36° and at 4° off head-on, so
  how far either answer holds as the angle changes is not checked.
- **The states are teleported** (tele.py's method), not ridden into place. Their search is across's
  gather, which the teleport does not refresh: it still says 36/23 on every frame. The cars were in it
  anyway, so the bike met them as a non-ghost would.
- **Both states (`stand.p2s`, `headon.p2s`) live only in s2's `context/s170`**: context/ is per worktree.

## Next

Nothing for S170. S186 (the wheels' swept contacts on the wagon) would let these captures become replays.
