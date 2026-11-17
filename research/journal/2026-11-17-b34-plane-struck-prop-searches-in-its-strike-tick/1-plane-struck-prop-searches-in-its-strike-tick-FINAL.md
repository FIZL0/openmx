# 1 — plane struck prop searches in its strike tick (FINAL)

Task B34. 2026-11-17.

## What I did

- Of the brief's three items I took (3), the plane, because it is the one on this task's files.
  Items (1) and (2) change `prop_set.rs` / `prop_contact.rs` and need images of their own; filed.
- `Flight::frame_props_in(…, movers)`: `Flight::frame_props` with the props' pass movers. Its
  `knocked` step builds the strike with `by = (me, first)` and puts it on with
  `PropSet::strike_in`, so each prop the strike files runs `search_late` in the pass's world and
  movers. `strike_in` also puts the late prop's probe mirrors on a riding bike's frame onto the
  strike's entries. Those are a bike's, not the plane's, and the plane's lane drops the strike's
  entries, so they go nowhere. The props' rays on the plane's hull (`struck_on`) still run after.
  `frame_props` (no movers) keeps B28's hearing.
- Callers: `Sim::step`'s plane lane (its `prop_movers` from `frame_held`), and openmx
  `Plane::step_traffic`'s own-set lane. `frames_together_props` and the shared-seat lane keep
  hearing: the seat lane needs `Shared::movers_mut` made public (prop_seats.rs) and the fleet a
  movers argument through `fly_together`. Filed.
- research/s241/slots.py found the copy at `<root>-slots/pcsx2/sN`, which no longer exists; it now
  asks `ps2kit.emu.config_dir()` (`openmx-slots/pcsx2/sN`). strike.py `--at FRAME` saves slots 3
  and 8 at FRAME and FRAME+1 instead of the near slots.
- Nothing taken from ../opentennis: it has no props struck by a vehicle.

## What the original does

The plane's collision object is filed first on free06's world node (S241). The node's collide walk
re-reads its filed count, so a ball the plane's rays wake (filed at the list's end) is searched in
the same walk: in the strike tick its list is the plane's mirror (kind 1, naming the plane's
record), then its own ground entry (kind 0, no other side). Image b34at-11066 shows exactly that.

## How it was proved

- Capture, PCSX2 copy 4 at NominalScalar 0.25, started with `WAYLAND_DISPLAY=wayland-1` and the two
  accessibility variables: `slots.py put biplane 1`; `aim.py PID 1 0 25 20 2 0 8` (aimed at frame
  11054, saved in slot 2); `run.py PID`; `strike.py b34 2 0 --frames 60` (ball 0 woke at 11067);
  then `strike.py b34at 2 0 --frames 40 --at 11065`. Images 11067–11069 of the two runs are byte
  for byte the same, so the rerun is the same run. `slots.py ram` wrote
  `context/states/s241-b34{,at}-*.eeMemory.bin`; the .p2s are in `context/s241/b34*`, the csvs in
  `context/capture/s241/b34*.csv`.
- Ball 0 is asleep in 11065 (enabled 0, velocity 0) and awake in 11066, so 11066 is the strike
  tick.
- `tests/prop_plane.rs a_ball_the_plane_wakes_searches_in_its_strike_tick`: free06's props and
  terrain off the disc, the balls seeded from 11065, the port's plane with its hull and the image's
  body, one `Sim::step`. Ball 0's list against 11066's: same length (2), and per entry kind,
  surface, from, on_body, normal, point bit for bit, and the same side named (the plane as striker
  0, then the terrain).
- With the plane lane switched back to `frame_props` the same test fails: the port's list is the
  mirror alone (`[(1, plane)]` against `[(1, plane), (0, none)]`).
- `./kit check -- -p game-sim --test prop_plane --test knock_launch --test flight --test
  plane_meets_plane`: 21 passed. `-p openmx --test plane_seat_props`: 12 passed.

## Not verified / not 1:1

- Ball 0's body after the step is close but not bit-exact: velocity port (15.150, −11.929, −7.205)
  against (15.056, −11.749, −7.170). The test seeds only the plane's body (its wheels, surfaces and
  controls are the port's fresh ones, as in S241's Sim test), so the plane's push differs. Not
  chased: the plane's full state seed is S241's open note, not this task.
- The arms on ball 0's entries are the resolve's and are not compared.
- Brief item (1): a woken prop's rays against the other props (`prop_props`) are still not cast in
  its strike pass, for the bike or the plane. Needs `prop_set.rs`/`prop_props.rs` and an image of a
  strike that wakes a ball beside another. Filed.
- Brief item (2): `search_late` in a strike still gets no wreck hulls (`&[]`). Needs a pre-strike
  image with a downed rider beside the struck prop. Filed.
- Shared-seat plane lane (openmx `step_traffic_shared`) and planes flown together
  (`frames_together_props`, `fly_together`): still heard only (B28). Filed.
- The car's prop strike (car.rs, the same `PropSet::strike` hearing) and the Kiowa: not touched.
  The Kiowa's strike code (`kiowa.rs`) has no `PropSet` strike at all that I found, so there may be
  nothing to change there. Filed with the car.
- Only one plane strike (free06, ball 0, from one aim) is checked.

## Next

Done.
