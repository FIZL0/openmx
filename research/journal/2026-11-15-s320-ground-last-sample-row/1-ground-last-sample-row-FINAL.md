# 1 — ground last sample row (FINAL)

Task S320. 2026-11-15.

## What I did

- Decided: yes, `world::Ground` answers the field's last sample row and column. `Ground::at` (and so
  `owns`, `ground`, `keeps`, `surface`) now refuses a cell only when `sx >= samples_x` (was
  `sx + 1 >= samples_x`).
- Why: `samples_x × samples_z` counts *cells*, not samples. Each leaf block is 17 × 17; its 17th row and
  column hold the far corners (`Terrain::cell` reads `lx + 1 < 17`). So the strip from the last
  sample to the field's box (`Ground::field`, `samples × spacing`, which is also the edge test's
  `Area`) is a real cell. The terrain object's query reads it; `Ground` dropped it.
- `tests/race.rs` (S296a's prop-raise test): the snap through `Ground` must now equal the snap
  through the field query bit for bit, last row included. Before, it allowed `Ground` to leave
  last-row points at 0.
- `tests/ground_cap.rs`: two new tests.
  - `the_last_sample_row_and_column_are_ground`: 2000 points in the last row, column and corner. Each
    one: `Ground::ground` = `contact_search::ground`, bit for bit, and `keeps` and `at` answer.
    At and past the box: no ground, not kept.
  - `a_ride_in_the_last_strip_stands_on_it`: a bike (`Ride`) dropped into the strip and thrown along
    it for 180 ticks rides it like a flat floor at the same height (within 1e-3 ft). Both tests fail
    with the old check.
- Tiled worlds (`tiles.rs`, `Tile::local`, `holes`, `seam_walk`, `every_cell`) keep their own
  "no last cell" rule, unchanged. They are the port's own (mods), and their seams and holes are
  built on it. A tile's `Ground::at` is only asked after `Tile::local` says yes, so they answer the same.
- Nothing from opentennis applies: it has no heightfield terrain.

## What the original does

The terrain object's ground query (`contact_search::ground`, S64) reads the cell under a point with no
bound but the tree itself. Its box is a sample wide past the last sample. A point in the last strip
reads the leaf's 17th row and column. sx11's race file has a node point at z 765 of a 768 ft field. The
original stands it at 89.97 (S296a's RAM image).

## How it was proved

- Against the original: `race.rs::the_race_files_raised_onto_their_props_are_the_track_objects_in_the_images`
  on S296a's five RAM images (sx06, sx08, sx11, sx20, nat03). The snap through `Ground` now equals,
  bit for bit, the field-query snap that matches the image. That covers sx11's one last-row point
  (the test prints `1 points in the field's last sample row or column`).
- Port against port: `ground_cap.rs`'s two tests on a generated 256 × 256 terrain (see above).

## Not verified / not 1:1

- A ride in the strip is checked against a flat floor in the port, not against a capture of the
  original. No capture rides a stadium's last 3 ft row (it is behind the stadium walls), and a
  capture there would need a hand-driven ride to the field's edge.
- Off the field's box the port still answers no ground (`NO_GROUND`), where the original's query
  leaves the point as it was (S64's known choice). Not changed here.
- The ride test's bike matches the flat floor to 1e-3 ft, not to the bit. The field's triangle
  arithmetic and the floor's plane round apart by one ulp from tick 26. Both are the port's own
  ground for this test: no claim about the original either way.
- Tiled worlds still refuse each terrain's last cell (see above). This is the port's own rule for mod
  worlds, and no disc map uses it.
- The shell's render-side `openmx/src/ground.rs` still has its own `samples - 1` extent. That is drawing
  only, so it is not the simulation's concern.

## Next

Nothing.
