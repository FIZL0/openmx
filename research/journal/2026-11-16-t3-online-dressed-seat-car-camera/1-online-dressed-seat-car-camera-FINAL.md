# 1 — online dressed seat car camera (FINAL)

Task T3. 2026-11-16.

## What I did

- **The route.** No headless path seats a second local player online. `Online::can_sit` is true only on a
  lobby ride, and only the front end starts a lobby ride. The `--peer` rides that the headless tests use
  have no room, so `seat_pick` is `None` and no seat is ever redressed either. So I made S268's throwaway
  patch a real, kept test hook instead of building a headless lobby (filed as N27).
- **main.rs.** New `--online-script` word `dress`. At its tick, the host rebuilds its own shell seat 0
  through `dress_peer`, with the same `PeerDress` the other two call sites build. The new rig replaces
  `rigs[0]`, and the shell prints `online: seat 1 dressed as a peer's, N car cameras`. The flag parser
  and the `--help` text accept the word. Nothing changes unless the word is given.
- **tests/online_car.rs.** `ride_online_with` (`ride_online` plus extra host arguments). New test
  `an_online_seat_dressed_as_a_peers_follows_its_car_on_the_cars_presets`. It runs two shells on free09
  with `--trace-camera` on the host: `20:dress,40:monster,41:gas`, then again without `dress`.
  - Both runs: 0 desyncs on both machines.
  - The dressed line says 3 car cameras.
  - From the dress up to the switch, camera 0 stands under 20 ft (the bike's 14.418 ft).
  - From the frame after the switch, every traced frame stands at the undressed host's truck distance,
    which is more than 8 ft past the bike's (28.240 ft).
- The trace's tick numbers are the shell's frames, which count while a shell waits for its peer: a
  session tick has no fixed trace number (tick 60 was still the bike in one run). So the test marks
  the moments by the shell's own `online: seat 1 dressed` and `vehicle: monster` lines.
- No `../opentennis` checkout on this machine, so nothing was taken from it.

## What the original does

Nothing to compare: the original has no online play. The car cameras themselves were proved against
captures in S246 and P264.

## How it was proved

- `./kit check -- -p openmx --test online_car an_online_seat`: passes.
- Control: with `dress_peer` building `car_cameras: Vec::new()` (S268's bug), the test fails. Reverted
  afterwards.
- By hand, release and debug builds: the dressed host's camera 0 is 14.418243 ft on the bike and
  28.240238 ft in the truck, the same numbers S268 got.

## Not verified / not 1:1

- **A real second local player in a lobby ride (N20) is not what the test seats.** No headless lobby
  ride exists. The test rebuilds this machine's first seat through the same dressing instead. That
  proves `dress_peer`'s rig carries the car cameras and follows a car when looked through. It does not
  cover the N20 seat's own path: its view laid out, its pad, its camera picked by `Rig::follow` from
  shell seat 1. Filed as N27.
- **A redressed lobby seat (a pick change) is not run either.** It needs a room, so again a lobby ride.
- **The camera's look-at, pitch and eye are not compared.** Only the distance and the preset count are.
  The distance tells the truck's preset from the bike's, which is what S268 broke.
- **The first two traced frames after the switch are skipped.** They land around the switch's own
  frame, where the line order and the camera's frame can be one apart.

## Next

Land it.
