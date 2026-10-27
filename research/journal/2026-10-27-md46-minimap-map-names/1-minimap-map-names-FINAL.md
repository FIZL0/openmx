# 1 — minimap map names (FINAL)

Task MD46. 2026-10-27.

## What I did

- `ride_hud.rs` `map_names`: every map name is now drawn inside the radar disc. The current map's
  name is centred just under the player's arrow. Each neighbour's name sits along its compass
  letter's direction (so it turns with the radar), as far out as its box still fits inside the rim
  (2.5 radar units in). Each name tries 9 → 6 radar units (own name) or 7.5 → 5 (neighbours) in
  half-unit steps. At the smallest size it is cut short with a trailing `.` until it fits. A size is
  rejected if the box would leave the disc, come nearer the middle than 0.6 of the radius (no longer
  "by its letter"), or overlap the arrow, the map's own name or an earlier label. MD42's
  outside-the-rim placement and its `held`-in-the-layout fallback are gone.
- Wrapped worlds: this part had already landed with MD43 (cc549863). `hud::MapNames::around_in`
  with the drawn `Wrap` looks at the maps' copies one width off on each wrapped axis, so the label
  at a wrapped edge names the first tile across the seam. main.rs already passes `hud_wrap` to the
  HUD. So main.rs and worlds.rs needed no code change, only a test proving the whole path from a
  real world file.
- opentennis has no radar, so nothing was taken from it.

## What the original does

Nothing: the original's radar names no map (MD42). This is the port's own presentation.

## How it was proved

- `crates/openmx/tests/ride_hud.rs` `the_radar_names_the_map_underfoot_and_the_map_each_way`
  (rewritten from MD42's): at 1080p and 4K, at 16 headings, with a 21-character neighbour name, all
  four corners of every label's rect are inside the radar disc. The own name is centred under the
  arrow. Each neighbour is nearer its own letter than any other and beyond half the radius. No two
  labels overlap, and none touches the arrow's box.
- Same file, `a_wrapped_edge_names_the_map_across_the_seam`: three maps along x with a seam join,
  drawn through `sprites`. Wrapped, West's W label is East and East's E label is West. Unwrapped,
  those labels are absent. Every rect is inside the disc.
- `worlds.rs` `a_wrapped_worlds_edge_names_the_map_across_the_seam`: MD42's 2x2 world written with
  `"wrap": "x"` and read through `find`, `map_names` and `tiled().wraps()/extent()`, as main.rs
  does. The seam joins add no map. sw's W is se and se's E is sw (across the seam). S off the south
  row stays blank (z does not wrap). The same world without the switch is blank on both x edges.
- Checked by eye: rendered the radar on the software path at headings 0 and 45° (throwaway test,
  removed). Names sit inside the rim by their letters, and the own name sits under the arrow.

## Not verified / not 1:1

- Not looked at in a live ride (GPU, windowed). Only the software raster of `ride_hud::draw` was
  checked by eye. Both draw the same sprites.
- Dots, icons and chevrons may still be drawn over a name: they move with the world, so no fixed
  place clears them. The names are small and edged so they stay readable. The task's "clear the
  dots" is met only for the arrow, the one mark that stays put.
- A neighbour too long to fit at 5 radar units is cut short with a trailing `.`. The test accepts
  either the full name or such a cut (none of its names is cut at the tested sizes and headings).

## Next

Nothing.
