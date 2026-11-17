# 1 — steer bale (PART)

Task MD168b. 2026-11-17.

The quad on MD147 steer.csv's take-off is struck by a **knockable bale**: Rocksbury's `TripleBale02`.
The port has no quad-against-knockable-body contact, so the steer test stays at row 196. Filed as
**MD224**. On the way I fixed a regression that had made every MD168/MD168a replay red.

## What I did

- **MD144a broke the Untamed replays; the edge word is now seeded.** On the branch base,
  `untamed_quad_replays_md147_steer` was red from row 1: the body was thrown about 5 ft up each tick.
  - Cause: MD144a's Untamed out-of-bounds band is 300 ft. Rocksbury is 896 ft wide and the spawn is at
    z 159.8, so the spawn lies inside the band. The test's `Wrecked` came armed by default, so the edge
    throw fired every tick.
  - In the image, the armed word (SLUS-21104 `+0x308`, Untamed `+0x338`) is 0 in every MD147 state:
    Free Ride does not arm it.
  - `seeded` now sets `edge` from the image word. Steer is bit-exact to row 196 again, and both hazard
    tests pass again.
- **`bad_read` compares the body's own words only** (`+0x0..+0x224`).
  - steer's row 197 is row 196's body, but word `+0x260` (the record, past the body) is one higher.
  - So the whole-row compare missed this late read, and the row showed as off.
  - Row 198 is torn (its force is non-zero). Both rows are now skipped, and the hazard tests' skip
    counts stay under 4.
- **Rocksbury is SX_A.**
  - All 33 sampled blocks of `Env\SX_A\SX_A.ps2.trn` are in the spawn's RAM.
  - The terrain is 256 samples at a 3.5 scale, which is 896 ft: the image's area.
  - The port's `atv_ground::Terrain::of(SX_A.ps2.trn).ground` equals the interpreter's ground query
    bit for bit at 4 points (spawn, take-off, two far points). This was a scratch probe; nothing is
    kept in the test.
  - The quad's record already answers its heightfield rays through `World::ground`, so the terrain
    was never missing. (`World::mxvatv_terrain` is read by the plane only.)
  - The disc's `SX_A.res` is extracted into `context/untamed/sx_a/` (col, env, geo, part, skies), with
    `SX_A.ps2.trn` at its top.
- **What struck the quad: a bale.**
  - Rows 198–199: the left front wheel's probe reports something other than terrain (terrain_wins 0,
    compression 0.63–0.87), while the other three wheels read terrain.
  - The scene's `[Model3887] SLT=TripleBale02` stands at (437.99, 9.28, 474.69). The quad passes it
    at (441.4, 9.3, 472.3) on row 196.
  - `SX_A.toy`'s PhysicsCollections claim it (`TripleBale*`), so it is a knockable body:
    `TripleBale.pco` gives one RigidBody, Weight 253, SimulationHz 55. Its meshes are
    `TripleBalef.col` (the one struck) and `TripleBalem.col` (its rays).
  - In the spawn image the bale has a collision record of the same class as the quad's (scratch
    recipe below), holding:
    - body mass 7.857 (253/32.2);
    - inertia (3.44, 33.45, 33.45);
    - its own scenery/actor response keys (`+0x10c..+0x1e4`, e.g. `+0x118` 1265 against the quad's
      2520);
    - its node's world rows: side, up, look, position.
  - The port's `props::frame` of that model gives the same rows bit for bit.
- **A still bale does not reproduce the knock.**
  - I placed every `*Bale*` model's `f.col` as a static `World::solid` (232 meshes, placed with
    `props::frame`). Rows to 196 stayed bit-exact.
  - But the port's quad then gets a wheel-probe contact and a record contact, and spins: rates
    (−8.5, −0.2, 9.1), x velocity −4 → −7.
  - The original's barely spins: rates (0.36, −0.90, −0.47), then (0.18, 0.07, −0.44). Its velocity
    changes by about (+6.4, +0.9, −0.5) beyond gravity.
  - A near-pure shove at the centre like that fits a two-body contact with a 253 lb body (the
    record's hit naming another record, `atv_contact::resolve`'s `Other`). It does not fit a fixed
    wall. The experiment's diff is not kept.

## What the original does

On the take-off the quad's left front wheel and its record's rays reach `TripleBale02`'s mesh. The bale
is a knockable body with its own record. The strike pushes the quad +x, away from the bale, in one tick,
with all four wheels off, and adds almost no spin. How the contact is filed (the record's other or mover
words) and resolved is not read yet: that needs a capture of the quad's record list on rows 195–199
(MD224).

## How it was proved

- `crates/game-sim/tests/untamed_race_quad.rs`, all 4 tests green:
  - steer bit-exact to row 196 (rows 197 and 198 skipped as bad reads);
  - hazard-time to row 190 and hazard-on to row 369, as MD168a left them;
  - the seed test.
- Moving the steer test's last row to 199 fails on row 199 only. So the replay runs, and the two
  skipped rows are the only ones skipped.
- Scratch recipes (scratchpad scripts, not kept):
  - finding the bale record: scan the spawn image for words equal to the quad record's class word
    (the quad record is the pointer at quad `+0xf8`); the struck bale's is the one whose body holds
    the model's position;
  - comparing the two records: diff them word by word.

## Not verified / not 1:1

- **steer.csv past row 198 does not replay.** The quad-against-knockable-bale contact is not ported
  (MD224). The test stops at row 196, as MD168 left it.
- **That the bale is the only thing struck.** It is inferred from:
  - the wheel's non-terrain hit;
  - the bale standing in the path;
  - no static `COL` object near the jump.
  
  The quad's record list on the strike tick was not captured.
- **The bale's record as a two-body contact.** It is read from the spawn image, not from the strike
  tick. The bale is at rest until struck, but a sleep or reset step that changes it in between was not
  ruled out.
- **The Untamed edge throw in a race or Free Ride where the armed word is 1.** Only the MD147 states
  were read; all have 0.
- The terrain match is 4 points from a scratch probe, not a kept test. The quad does not read
  `mxvatv_terrain`, so nothing in the port depends on it.

## Next

MD224: capture steer rows 195–199 with the quad record's contact list and the bale's body, port the
contact, and extend the steer test to the end.
