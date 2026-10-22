# 1 — one level terrain size (FINAL)

Task MD12. 2026-10-22.

## What I did

- `game_sim::world::SAMPLES_PER_WIDTH = 256`: `Ground::new` makes a field `TerrainWidth × 256` by
  `TerrainBreadth × 256` samples whatever the tree's depth, instead of `width × 16^levels`. A one-level
  tree (MX vs. ATV's supercross, freestyle and three of its motocross maps, `TerrainWidth=1`) was 16
  samples a side, so its rider started off the map; it is now 256, as the track standard's table says
  ("256-sample units a side. The map edge is width × 256 × scale").
- The same rule in the two other places that counted `16^levels`: the shell's `openmx/src/ground.rs`
  (drawn ground and its log line) and game-render's `eco::extent` (the `.esb` positions' scale), both
  now reading the one constant. `TerrainLayout::width`'s doc says what the unit is.
- On a two-level tree 256 = 16², so every MXU disc track (all two-level) sizes exactly as before.
- Test: `crates/openmx/tests/track_mods.rs` `a_one_level_terrain_is_256_samples_a_side` — the generated
  flat terrain now comes in one or two levels (`flat_terrain_of`); the one-level tree parses, has cells to
  (254,254) and none at 256; `Ground::new` gives 256×256 for both depths with ground under the spawn and
  at (1019,1019) ft; and the shell rides a one-level mod (`md12_onelevel`) from the spawn on a throttle
  tape: 256 × 256 samples, 1020 ft, starts at x 512, ends > 400 ft up +z on the ground. The disc was
  present in this slot, so the ride part ran (it skips without one).
- Nothing taken from opentennis: it has no heightfield terrain tree.

## What the original does

The standard's table cites the game's map-edge computation as width × 256 × scale (address in the
standard, `OPENMX-TRACK-STANDARD.md` §2's scene table). The port's `biplane::Area` already used the scene's
numbers for the edge; only the sample count disagreed.

## How it was proved

- The generated one- and two-level terrains in the test above (no disc data committed).
- A one-off scratch crate (not committed, as the mods files are other games') over all 56 MX vs. ATV
  `.res` in `~/repos/openmx-mods/extracted/mxvatv/Env`: every scene's terrain parses, `Ground::new` gives
  256×256 for the 26 one-level maps (FS01–08, MC06–08, SX01–18; SX07 256×512 from its `TerrainBreadth=2`)
  and 1280×1280 for the two-level ones, and **every** scene's `DefaultStartPosition` now has ground under
  it (`Ground::at` is `Some`). Before, a one-level map's field ended at 64 ft (16 × 4 ft) and every spawn
  (75–804 ft) was off it. The tree's own extent matches width × 256 on each map except the scenes the
  standard already lists as disagreeing (Nat04, SX07).
- `./kit check` passes (the disc's free06/free09 rides and replay hashes are unchanged: 16² = 256).

## Not verified / not 1:1

- Not compared against the original game running: MXU's disc has no one-level terrain and MX vs. ATV
  is not the emulated game here, so "256 per width" is the standard's statement plus the fact that every
  MX vs. ATV spawn lands on it, not a capture.
- Where a scene disagrees with its tree (Nat04 `TerrainWidth=5` over a 1024-sample tree; SX07
  `TerrainBreadth=2` over 256 samples) the field follows the scene, so the port's field is larger than the
  tree and the excess has no ground (`Terrain::cell` answers `None`, so `Ground::at` is `None` there).
  What the original does on those cells is for T7, as the standard says; not checked.

## Next

Nothing; done.
