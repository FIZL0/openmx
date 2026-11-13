# 1 — supercross has no shootoff (FINAL)

Task P332. 2026-11-12.

## What I did

- Found the launch: the map edge's throw (`game_sim::off_map`, the cannonball). sx01's grid stands at
  x = 94.9 ft, inside the 150 ft margin, so the armed edge threw the rider on tick 0 at 77 ft/s.
- The original's rule was already ported and proved (S73, `off_map::armed`, oracle in
  `crates/game-sim/tests/edge_armed.rs`): the bike builder arms the edge only on a player's bike at an
  outdoor event — Nationals or free ride — never in a Supercross stadium. The shell never told the
  rider the league: `Rider::new` arms it as a free ride, and only a mod's `InStadium` toy (P242)
  disarmed it. So every disc Supercross track was ridden with the edge armed.
- `menu::race::league_of(track)` (crates/openmx/src/menu_race.rs): an `sxNN` track (RACE_SUPERCROSS) is
  Supercross, a National (RACE_NATIONALS) Nationals, anything else free ride. The shell's `new_rider`
  (crates/openmx/src/main.rs, which the web build mounts as `shell`, so beta.openmx gets it too) now
  sets `edge_armed &= !stadium && off_map::armed(Built::Player, league)`.
- Did not move the spawn: the user said the race maps should have no shootoff at all, and the original
  decides by league, not by where the grid is.
- Nothing taken from opentennis: it has no map edge.

## What the original does

The bike builder writes the edge flag from who the bike is for and the event's league (S73's journal
has the `./kit fn` recipe). The league here is the track's kind as the race pick lists it.

## How it was proved

- The arming rule itself: S73's oracle (`edge_armed.rs`), unchanged, plus its ride test
  `a_disarmed_bike_rides_past_the_edge`, which rides a `(Player, Supercross)` bike past the margin for
  30 frames without a throw, while a free-ride one is thrown.
- New unit test `only_the_supercross_tracks_disarm_the_edge` (menu_race.rs): all 24 sx tracks are
  Supercross and unarmed; all 22 Nationals and the free-ride/world/mod ids are armed.
- The shell, headless (`openmx --track T --race --trace 90 --trace-feel`), before and after:
  - before: sx01 `thrown: tick 0 at 94.9, 445.1`, 77 ft/s; nat01 holds on its grid;
  - after: sx01 and sx12 hold on their grids (speed ≤ 0.24 ft/s, no throw); nat01 unchanged, byte
    for byte.

## Not verified / not 1:1

- That the original's event league for an MX Unleashed race on an `sxNN` track is the Supercross one
  is read off the front end's two race kinds (RACE_KINDS), not traced in a capture of the original's
  race event. Reason: it is the only way the disc's tracks are offered, and S73 already proved the
  rule on the league word; no capture of a race event's league was taken.
- A free ride on an `sxNN` track (`--track sx01` without `--race`) is disarmed too. The original
  offers no free ride in a stadium, so there is nothing to compare; this follows the user's ask
  (race maps have no shootoff at all).
- The AI racers are not built through `new_rider` (the seats after the first are human players, also
  `Built::Player`); an AI bike is never armed in the original (`armed(Ai, _)` is false). Whether the
  port's race AI riders keep `edge_armed` off was not checked here.
- No new test in `crates/game-sim/tests/race.rs`, though the brief named it: game-sim does not know
  track ids, and the "a Supercross rider near the edge is never thrown, a National's is" case is
  `edge_armed.rs`'s ride test already; the shell's mapping is tested in menu_race.rs.
- sx01's rider creeps toward the gate at up to 0.24 ft/s over the first 90 trace ticks (0.5 ft); the
  `--trace` loop does not run the race's start hold (S290's freedom behind the gate), so this was not
  judged against the original. The web ride itself was not driven in a browser.

## Next

Nothing.
