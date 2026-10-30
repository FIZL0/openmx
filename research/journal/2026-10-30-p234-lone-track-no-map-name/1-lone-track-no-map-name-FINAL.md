# 1 — lone track no map name (FINAL)

Task P234. 2026-10-30.

## What I did

- User 2026-10-30: on a non-world map the radar should not name the current map under the arrow.
- `main.rs`: the ride's `map_names` is now `world_mod.as_ref().map(|w| w.map_names()).unwrap_or_default()`:
  only a world mod's tiles are named; a disc track or a track mod gets empty `MapNames`, so
  `around` gives `(None, [None; 4])` and `ride_hud` draws no map line and no rim labels.
- `hud.rs`: `MapNames::alone` removed (it had no other caller); the struct's doc says a lone track has no maps.
- `worlds.rs`: the map-names test asserts `MapNames::default().around(..)` is `(None, [None; 4])`;
  `map_title("free09")` is still checked as "Bridgeport" (worlds still name their tiles through it).
- `tests/ride_hud.rs`: `a_lone_track_names_no_map_under_the_arrow` — empty names at three spots draw
  the radar but no `d.map` and no `d.around`.
- Nothing in opentennis to take: its HUD has no radar map names.

## What the original does

The original's radar names nothing (one map per ride); the map names are the port's own (MD42), so a
lone track now matches the original here.

## How it was proved

`./kit check -- -p openmx --test ride_hud` (44 pass) and the worlds lib test; the full gate in `./kit done`.

## Not verified / not 1:1

- Not looked at in a live ride window: the change is a data path (empty `MapNames`) covered by the
  ride_hud sprite test, and the shell's ride HUD draws from those sprites.

- The gate's switch_tape replay test gave two different ride hashes under load (~40) and passed
  alone; HUD map names do not reach the sim, so filed as K15 (critical) rather than chased here.

## Next

none
