# 1 — plants on the ground (FINAL)

Task V49. 2026-10-22.

## What I did

- **The cause was the snap bit, not the matrix.** Every plant of free06's `.esb` (5600: ScaggBushA/B,
  DeadBushA, YellowBush, YellowGrass) has bit 7 set on its species byte, and its file y is up to 54 ft
  above or 12 ft below the terrain under it. V48 ignored the bit, so free06's bushes floated. The
  original stands those plants on the terrain as it reads them.
- `crates/game-render/src/eco.rs`: added `Ecosystem::stand(extent, ground)`. It moves every plant whose
  bit is set onto the ground and stores the height the original's way: `(u16)(int)(y·65536/extent)`,
  then times `extent/65536`. Also added `eco::ground_under`, the original's terrain point query
  (`game_sim::contact_search::ground` over `game_sim::world::Ground::field()`, S43's port).
  `Grove::load` now takes the scene's `TerrainLayout` and the `Terrain` instead of an extent. It works
  out the extent itself and stands the plants. The module doc no longer says the bit is never honoured.
- The two callers, `crates/openmx/src/main.rs` and `web.rs`, pass the layout and the terrain.
- world.rs and gpu.rs needed no change. Both renderers and both paths (model and billboard) take every
  plant's matrix from `Grove::draws`, so they all get the stood position.
- Checked the brief's other candidates and ruled them out:
  - **The model's scale about its origin.** Every free09 and free06 model has its lowest vertex at or a
    little below its origin (−0.0015 to −3.0 ft, the trunk's foot), so scaling about the origin cannot
    lift one.
  - **The quantised y.** free09's file heights put the median plant 0.04 ft off the ported query's
    ground, so they are fine.
- Took nothing from opentennis: it has no ecosystem.

## What the original does

- **Plant reader** (`./kit fn 0x12b470`). The species byte's bit 7 is cleared and kept. The snap runs
  when `(param_3 != 0 || bit) && eco+0x90 != 0`. V48 read only `param_3`, which the scene loader passes
  as 0, and concluded the snap never happened. The bit alone triggers it. `eco+0x90` is the loader's
  sixth argument (`./kit fn 0x12acb0`, the world), and it is non-null in the biplane state
  (`0x019c91e0`). The snap builds the point `u16·(eco+0xd4)` for x, y and z and calls the ground query
  (`./kit fn 0x1dc960`), which moves the point onto the terrain. It then writes back
  `y = (u16)(int)(y·(eco+0xd8))`, where `+0xd8 = 65536/extent`. x and z are unchanged.
- **The ground query** (`./kit fn 0x1dc960`) uses the same cell read and the same two-triangle height
  arithmetic as the terrain point query S43 ported (`0x1dce80`, `contact_search::ground`): d00 against
  d11, split one way or by parity, and step 2 when the terrain is coarse. It is a near duplicate on the
  terrain object, and the port uses `contact_search::ground` for it.
- **Model matrix** (`./kit fn 0x1e04b0`), confirmed as V48 had it. The rows are
  `(c·k, 0, s·k)`, `(0, (+0x14c)·h, 0)`, `(−s·k, 0, c·k)` and `pos`, with
  `k = (+0x150)·w·(1/32767)`, and (c, s) the plant's shorts at +0x14/+0x16. The origin is the plant's
  spot, with no offset to the bounds' centre or base and no sink. The `param_3` branch rotates the same
  rows by a quaternion: that is the trees' bend (S97).

## How it was proved

All in `crates/game-render/tests/ecosystem.rs`. Each test needs the disc and a save state's RAM image
from `context/states/`, and skips with a message when one is missing.

- **Save-state RAM, every plant.** `free06s_plants_stand_on_the_ground_as_the_originals_do`: the port's
  x/y/z words equal the original's own for all 5600 plants of free06 in
  `biplane.p2m2_SaveState.eeMemory.bin` (eco object at `*0x46a5f8`, plants at +0x68, 0x1c each). The
  state's +0xd4 equals the port's extent/65536. Every model and billboard base sits within −0.105..0.0 ft
  of the ground under it.
- `free09s_plants_keep_the_files_heights_as_the_originals_do`: none of free09's plants asks for the
  snap. All 726 RAM positions in `first_recording.p2m2_SaveState.eeMemory.bin` equal the file's. Model
  and billboard bases run from −1.65 to +1.11 ft against the ported query, and ≥98 % are within 0.5 ft.
- `near_plants_matrices_are_the_originals`: 766 near plants of free06 in the biplane state (fade < 0x80
  and a non-zero turn). For each, the original's matrix is rebuilt from the state's numbers (species
  +0x100/0x104/0x10c/0x114/0x14c/0x150, size bytes, shorts, position words) by the formula above. The
  port's `model_at` matches it to 1e-5 relative, first plants 0, 2, 17, 42, ….
- I ran the tests with a forced failure to read their output (a passing `./kit check` shows no log), so
  none of the three was skipping.
- **By eye**, in the shell's GPU renderer: free09 BushB from `--trace-at 4506.65,3411.48,18` stands on
  the ground. free06 from `--trace-at 2790,2060,0`: the YellowBush/ScaggBush plants whose file heights
  were 13–25 ft up now stand on the slope. Shots are in the scratchpad and were not kept.

## Not verified / not 1:1

- **No PCSX2 screenshot side by side.** The brief asked for screenshots from the same spots. I proved
  placement against the original's own RAM words for every plant instead, which is exact where a
  screenshot is by eye. Getting PCSX2 to the same spot would need a teleport script. Not done.
- **The software renderer was not shot.** It draws from the same `Grove::draws` matrices as the GPU,
  and the test reads those matrices, so the positions are the same. Only the GPU picture was looked at.
- **free09's few off-ground plants stay off the ground.** Up to 1.65 ft into it (a TreeE) and 1.1 ft
  above it. The original keeps the same file words (RAM check above), so they match it. What the
  original's picture shows there was not looked at.
- **free07/free10** were measured but not proved against RAM (no state of them). They have no snap bit.
  Their file heights against the ported query run from −1.97 to +9.88 ft on free07 (a rescuetreeH) and
  −1.68 to +2.48 ft on free10. If the original's picture differs there, the cause is not this bit.
- **The ground query is the S43 point query, not a port of `0x1dc960` itself.** I read its height
  arithmetic and it is the same. Its terrain object's flags (+0x760 split, +0x8a8 coarse) are taken as
  `Ground::field()` has them. The 5600-word RAM match covers that on free06.
- **The snap's float steps run on host f32.** That is `u16·scale`, then `y·(65536/extent)`, then the
  cast. They match the PS2 for these normal values, and the RAM match shows no word off by one.
- **The billboard's quad build** is still V48's assumption: bottom edge on the spot, scaled w/W and h/H.
  `Vu1EcoSystem2D` was not read. Its base is on the ground because it stands on the plant's spot.

## Next

Nothing for V49.
