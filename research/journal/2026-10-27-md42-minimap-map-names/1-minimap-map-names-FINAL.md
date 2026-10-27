# 1 — minimap map names (FINAL)

Task MD42. 2026-10-27.

## What I did

- `hud.rs`: `MapNames` — each map's owned rectangle and name. `on` is the rectangle holding the
  rider, or the nearest one when the rider is on a join between maps. `toward(from, at, k)`
  takes the maps whose middle is past `from`'s edge in compass direction k (N +z, E +x, S −z,
  W −x, the radar's own bearing). It picks the one whose span across that direction is nearest
  the rider's coordinate (0 when the span holds it), and then the nearest past the edge. `around`
  returns both. `alone(name)` is one map covering everything, with no neighbours.
- `worlds.rs`: `map_title` names a free-ride archive by its town (`free06` → Dallas, from
  `menu::TRACKS`), and anything else as it is. `WorldMod::map_names` uses `regions()`'s rectangles.
  A tile's name comes from its own `mod.json` `name`, then its folder name; a spliced world's
  (P71) disc maps use their stem. All of them go through `map_title`.
- `main.rs`: `map_names` is built once per ride. A world uses its tiles. A mod track is one map
  under its pick name, and a disc track one map under its town. `RideHud` carries it, and every
  view's `Shown` gets `map` and `around` from that view's rider position.
- `ride_hud.rs`: `map_names` draws the map's name centred under the radar, below the lowest a
  compass letter reaches, in CHALK. Each neighbour's name goes just past its compass letter, in
  smaller BONE type, and turns with the rose. The text is held inside the layout. If a name would
  then sit on its letter (at the view's left edge) or on the map's name (S at the bottom), it goes
  just inside the rim. `Drawn` records `map` and `around` for the tests.
- opentennis has nothing like this to take: a court is one place, with no maps.

## What the original does

Nothing. MX Unleashed rides one map, and its radar (`freestyleradar`) names no map. This feature
is the port's own (user request, MD29 split), as the worlds are.

## How it was proved

- `crates/openmx/tests/ride_hud.rs`, `the_radar_names_the_map_underfoot_and_the_map_each_way`:
  - at 1080p and 4K, facing 16 ways, the map's name is centred under the radar and below every
    letter;
  - each neighbour's name is nearer its own letter than any other, never on the map's name, and
    inside the view;
  - a rider walked east across a 2×2 world sees the name change from SW to SE at the join's
    middle, and the neighbours go from N NW / E SE to N NE / W SW.
- `worlds.rs` unit tests:
  - `a_worlds_radar_names_the_map_underfoot_and_each_neighbour` reads a 2×2 world mod from disk:
    the names come from mod.json, free06 is named Dallas, a blank or missing name falls back to
    the folder, and the rectangles are right. It checks `around` at points on, between and across
    tiles.
  - `the_neighbour_each_way_is_the_one_across_from_the_rider` covers uneven neighbours: of two
    maps east, the one across from the rider; in the gap between them, the map further on that
    riding east actually meets; and a lone track has no neighbours.
- By eye, `--headless` shots at 1280x720:
  - `world_mxu` at its spawn: Dallas under the radar, with nat17 N, nat12 E, nat01 S (inside the
    rim, clear of Dallas) and Rochester W (inside, off the view's edge).
  - `free09` alone: Bridgeport under the radar, with no neighbour names.

## Not verified / not 1:1

- Wrapped axes (MD29): MD29 has not landed, so there are no seams. `toward` only looks inside the
  world's rectangle, so on a wrapped axis it would name no map past the last tile. MD29 has to
  make `MapNames::toward` look across the seam (the plan's MD29 line already says so).
- No original to match: the feature is the port's own, so there is no proof against the PS2. The
  layout is placed by eye. In a split view the names shrink with the HUD and can be small. This
  was looked at in a full view only.
- A world tile's display name is whatever its `mod.json` says. The openmx-mods importer writes
  bare map stems there (`nat01`, `MC01`). Only free-ride stems get a town name (from
  `menu::TRACKS`), so national maps read as `nat01`. A nicer name needs the importer to write one.
- Long neighbour names are not shortened. A very long one inside the rim can cross the middle of
  the radar.

## Next

-
