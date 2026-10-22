# 1 — camera out of the ground (FINAL)

Task S103. 2026-10-22.

## What I did

- Found what keeps the original's race camera eye out of the ground: not a probe in its frame, but the
  base filter's own call into a class slot, which the race camera overrides with an eye floor. S24
  wrote that nothing calls that slot; the base filter does, twice a frame. Corrected in
  `research/oracle.txt` (S103 block).
- Ported it: `Race::keep_off_the_ground` and `Race::filter` in `crates/game-sim/src/camera.rs`
  (float arithmetic through `F` only), `Follow::min_height` / `Follow::ceiling`, `Race::ceiling_for_fog`,
  and `Surface::has_ground` (false for `Nothing`, so a camera with no world is left alone as the
  original's is with a null world). `Race::update` now runs its filter through `Race::filter`.
- The shell (`crates/openmx/src/main.rs`) sets both cameras' ceiling from the scene's fog distance, as
  the race camera's setup does; the call already hands the camera `world.place`, whose `on_surface` is
  `world::Ground`'s.
- Captured the original's camera behind `first_recording.p2m2` twice (`bike-camera.toml`,
  `play_camera.sh` beside this file) and replayed both through the port over free09's terrain
  (`crates/openmx/tests/bike_camera.rs`).

## What the original does

The base camera's filter (`./kit fn 0x001fbc20`) calls vtable slot +0x108 as `slot(cam, position,
target, 1, 1)` on the raw eye before the lags, and — only in the arm where the preset's lags run and the
camera is not snapping — `slot(cam, &lagged, target, 1, 0)` on the lagged eye after them. The base
class's slot returns 1 and does nothing; the race camera's (`./kit fn 0x0021e4d0`) is the floor:

    floor = max(viewport near clip * 2, cam+0x1a0)
    if cam+0xe8 (a collision object): a segment probe from the target to the eye  -- null on the disc
    if cam+0x184 (the world):
        ground = ground_query(the eye as it came in).y
        y = min(eye.y, cam+0x1a4); eye.y = y
        if y - ground < floor:  (c.olt.s)
            if lock: cam+0x16c = 1
            if lift: eye.y = ground + floor
        else if lock: cam+0x16c = 0
    return 1

`cam+0x16c` is the follow camera's "pitch down locked" (the base nudge refuses to pitch down while it
is set). The race camera's setup (`./kit fn 0x0021e270`, from the camera's create at 0x001ff270) sets
`+0x1a0 = 2.0` and `+0x1a4 = 200.0` when the scene's maximum fog distance is over 1500, else FLT_MAX;
its constructor leaves 3.0 in `+0x1a0` before that. With the near clip at 2.4 the disc's floor is 4.8 ft.
Its hit callback (0x0021e190) is byte-identical to the base camera's probe hit.

Live RAM, first_recording, recording2 and biplane states, both race camera objects each: `+0xe8 = 0`,
`+0xec = 0`, `+0x184` = the world, `+0x1a0 = 2.0`, `+0x1a4 = FLT_MAX` (free09's fog max is 1000, the
biplane level's 768), near clip 2.4.

## How it was proved

- **Function oracle, 4000 generated cases** — `crates/game-sim/tests/camera.rs`
  `the_eyes_floor_is_the_games_own`: the EE runs the disc's floor on random eyes, grounds, floors,
  ceilings, lock/lift flags and world/no world (1843 lifted, 734 clear, 385 capped, 1038 no world);
  the port's eye, lock and every ground query point match bit for bit.
- **Function oracle, 4000 frames** — `one_race_frame_is_the_games_own` now runs the disc's whole race
  frame with the floor slot wired in and a world under it (2318 frames lifted); eye, look-at, every
  region and the sequence of probe and ground queries match bit for bit.
- **PCSX2 capture, as the disc has it** — `context/capture/s103/bike-camera.csv`, 1811 rows of the
  human's free09 ride (frame_counter 2774..4586), two wrecks. The original's floor never locks on this
  ride: its eye is never nearer free09's dirt than 6.46 ft. The port, fed the captured block over
  `world::Ground`, puts its eye on the original's within 0.01 ft on 1184 of 1186 compared rows (the
  other two are torn reads, 0.18 ft), never locks either, and the captured eye is never under 4.8 ft
  over the port's ground. Test `the_bike_camera_keeps_the_originals_eye_over_free09`.
- **PCSX2 capture, floor raised** — `bike-camera-raised.csv`: the same playback with the camera's
  `+0x1a0` poked to 12.0 over PINE after the state loaded (the VM paused), so the floor works over the
  hollows and hills: the original's lock is set on 993 of 1811 rows. Of 1127 compared rows the port's
  lock disagrees on 1 (an eye sitting on the floor within the grounds' difference); 820 eyes match to
  0.01 ft; 123 more differ by exactly how far the original's eye sits from the port's floor, i.e. both
  are lifted onto the floor over their own grounds and the whole difference is the ground (at most
  0.384 ft); the rest are the lags carrying that lift a few rows (0.01-0.06 ft). Test
  `the_raised_floor_holds_the_eye_where_the_originals_is`.
- `./kit check` passes.

## Not verified / not 1:1

- **The ground under the eye.** The camera's floor is exact; what it stands on is `world::Ground::at`,
  whose bilinear blend is that module's own and not the game's terrain query (its module note). The
  raised capture shows them up to 0.384 ft apart under the eye (free09, rows 90-98), so the port's eye
  can sit that much higher or lower than the original's wherever the floor holds it. Filed as
  S104.
- **The segment probe arm** (`cam+0xe8` set) is not ported: the collision object was null in all three
  states read, so it never runs on the disc. If a mode sets it, it is missing.
- **The ceiling's 200 ft arm** is ported but unproven live: no track was captured with a fog distance
  over 1500. That the setup's fog record is the scene's `MaximumFogDistance` (`view::Scene::fog_far`) is
  read from the setup's code and the free09/biplane values, not checked on a third level.
- **The rows after a wreck** are not compared: the original eases its eye back from where the wreck
  camera left it over ~50 rows, which the port's hand-back (a snap, `chase::WreckCamera::switch`) does
  not. That is the wreck camera's hand-back (V31's), not this floor; the first row back on even shows
  the wreck camera's eye under the floor (rows 576, 1116 of the raised run).
- **Poking the floor changed the ride.** The raised playback wrecks three times where the plain one
  wrecks twice, from row ~1000 on. Whether the camera's state feeds the ride or the playback simply
  diverged was not looked into; the test replays each capture's own block, so it does not matter to it.
- The derived camera's nudge (0x0021f770) does not read the lock; the shell never calls a nudge, so the
  lock is written and read by nothing in the port but the tests.

## Next

Nothing for S103. S104 is the game's own terrain query under `world::Ground::on_surface`.
