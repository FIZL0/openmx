# MD166 — which camera a racing machine gets

## The finding

SLUS-21104's camera controller (oracle `atv_camera_controller_*`, `# MD166`) holds two cameras:

- `+0x7c` the race camera (the `RC_<kind>Default.pb` files).
- `+0x80` the follow camera (the `FC_` files, MD82b's `ModFollow`).

Its constructor makes the follow camera current. When a player gets on a machine, the controller picks
a camera from the event's type word (`atv_event_type`): the race camera only for type 5, Hill Climb.
Every other event, races included (Supercross, National, Open Class, A to B...), gets the follow camera.
Getting off picks follow too.

Proved in RAM, not only read:

- `context/mxvatv-ps2/md166/atv-race.p2s`: Desert Baja Race (type 8), ATV, current = follow.
- `context/mxvatv-ps2/md166/car-race.p2s`: the same race in a trophy truck, current = follow.
- MD39a's buggy free ride (type 17): current = follow.

`research/md166/cams.py` prints this for any `.p2s`.

So the task's "port it beside ModFollow" is already done: the race camera *is* ModFollow. The shell's
`main.rs` uses `chase::mod_follows` in races as in free ride, so no shell change. I wrote the race
camera's update down in the oracle (`atv_race_camera_update`) and parked its port as MD185 (later),
for when the shell gets a hill climb. The docs on `ModFollow` and `chase::mod_race_presets` now say so.

## The race states

From `md39a` options state: Event Type → Racing → Open Class Racing → Desert Oasis → Continue →
Player 1 Options (Vehicle: right ×1 ATV; right ×4 gave a trophy truck, not the buggy MD39a's notes
suggest) → Start Event → wait ~1800 ticks → Proceed at the flyby → 150 ticks to the start lights.

## The captures

`research/md82b/play.sh` with MD39a's drive tape from each race state, 730 rows:

- `context/capture/md166/atv-race.csv`: frames 37309..38769, one gap.
- `context/capture/md166/car-race.csv`: frames 35487..36955, four early gaps.

The first ATV try got 613 rows (the emulator ran slow under load). I set the copy's `NominalScalar`
to 0.3 for the captures and put it back to 1 after.

## The test

`tests/mxvatv_camera.rs` gets both captures (`atv-race` on `FC_ATVDefault.pb`, `car-race` on
`FC_BajaDefault.pb`). Two things races have that free rides didn't:

- **Put-backs.** Riding off the course sets the machine back on the track (~360 ft). The original
  resets its camera the frame before the machine moves; that is not the follow update. A row whose next
  row is over 50 ft away is skipped and the replay reseeds after it.
- **Slow read-aheads.** The ATV slowing to ~2 ft/s after a wreck had a row read a tick ahead, then
  repeated. The jump rule only looked over 5 ft/s; it now also counts a slower jump when the next row
  repeats it. atv-s-spawn moves one row from replayed to reseeded with this; all others are unchanged.

Results:

- atv-race: 607 rows bit for bit (17 doubled or dropped, 68 wrecked, 2 put back).
- car-race: 714 rows bit for bit (3 doubled or dropped, 2 put back).
- Mutation: `car-race` on `ModMachine::Buggy` fails at the seed ("none of rows 0-11 steps"), so the
  test tells the Baja file from the Buggy one.

## Not verified / not 1:1

- The race (Hill Climb) camera is not ported or captured. Parked as MD185.
- The controller's vtable slot +0x84 (a setter that picks either camera) has no direct caller found;
  a vtable call elsewhere could still pick the race camera in some event. Only types 8 and 17 checked
  in RAM.
- One track (Desert Oasis), one race type (Open Class), two machines. Bikes, the buggy and other
  cars in a race are inferred from the controller code, not captured.
- The put-back's own camera reset (the frame before the machine moves) is skipped, not ported. The
  shell's race put-back may not reset the camera the same way.
- The captures ran at 0.3 emulator speed; the game logic is the same, but this is not checked.
- The AI riders and their cameras are not compared.
- The shell is unchanged; no shell race was watched on screen for this task.
