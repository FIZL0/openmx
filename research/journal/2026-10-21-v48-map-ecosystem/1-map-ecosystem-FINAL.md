# 1 — map ecosystem (FINAL)

Task V48. 2026-10-21.

## What I did

- `crates/game-render/src/eco.rs` (new) reads `.esb` files in full: the species (model, billboard texture,
  size triples, atlas rectangle, trunk u, solid records) and the 9-byte plants, placed in feet. It also
  holds the original's distance rule (`eco::look`) and builds a billboard quad model for each species.
  `Grove` is a loaded ecosystem: per frame it lists what a view draws, a turned and scaled model near,
  a camera-facing picture far, and both between with the picture faded in.
- `scene.rs`: `Scene` reads `[Environment]`'s `EcosystemFile`, `EcoDistance2D/3D` and
  `ShrubberyDistLimit`, using the original reader's defaults (180/120/400). It also carries
  `ecosystem: Option<Grove>` once a shell has loaded one.
- `world.rs` `draw_scene` (software) draws the grove with the props. Pictures are drawn only once
  they are solid, because the CPU batch has no see-through pass. `gpu.rs` `Renderer::set_scene`
  takes the grove, and `render` culls and batches it per view; the fading pictures go through the
  existing ghost-alpha path.
- The desktop shell (`main.rs`) and the web start line (`web.rs`) load the grove after the props.
  `Assets::load_model` (in eco.rs) keeps a model built in code and resolves its materials from the
  archives.
- Tests: `crates/game-render/tests/ecosystem.rs` (disc) and two unit tests in eco.rs.
- Follow-ups filed: S94 (tree trunks as solids in the sim) and S95 (the nationals' terrain does not
  read).

## What the original does

The scene loader passes `EcosystemFile` and the three distances to the ecosystem loader
(`./kit fn 0x12acb0`; the `[Environment]` reader is `./kit fn 0x1a7b30`). The header, species and
plant readers are `./kit fn 0x12aa30`, `./kit fn 0x12a3d0` and `./kit fn 0x12b470`. The file layout
is in eco.rs's module doc. On free09 it parses to the last byte: version 2, 11 species in slots 1..11,
and 726 plants.

- **Positions.** u16 × scale, where scale = max(map x, map z) / 65536. The map side is
  samples × spacing, `width·16^levels·terrain scale`, which is 6912 on free09. RAM holds the same
  scale: 0.10546875 at the ecosystem object's +0xd4 (the object pointer is the word at 0x46a5f8). With
  the `(samples−1)·spacing` extent the plants miss the ground by up to 5.9 ft. With 6912 the median
  is −0.04 ft (least −1.65, most +1.10), measured against the sim's own terrain.
- **Species byte.** It is the file's slot number, not a compact index: free09 uses 1..11 and slot 0
  is empty. Bit 7 asks for snap-to-ground. The scene loader passes 0 for that parameter, and no
  free09 plant sets it.
- **Size.** `h = H + c1·(H−Hmin)/128`, `w = W + c2·(Wmax−W)/128`. The asymmetry, min for height and
  max for width, is the original's own (per-frame pass, `./kit fn 0x12bbd0`, the box it culls by:
  centre y + h/2, extents (w, h/2, w)).
- **Model matrix** (`./kit fn 0x1e04b0`): rows (c·sw, 0, s·sw), (0, sh, 0), (−s·sw, 0, c·sw), pos,
  with sw = w/half.x and sh = h·0.5/half.y of the model's root bounds. RAM has the species' 0.5/half.y
  and 1/half.x for TreeB at +0x14c/+0x150 (0.0157, 0.04106). (c, s) are two shorts written when a
  plant changes between flat and modelled: (fwd.z, −fwd.x)/|fwd_xz| of the camera, or (32767, 0) when
  fwd.z is 0. After that they stay fixed.
- **Distance rule** (`./kit fn 0x12bbd0`): d² across the ground in position units, times fov/77
  (camera +0x170, 85 in the biplane state), compared with D2² and D3². Past D2 the plant is flat only
  (fade 0x80, bit 0 set). Inside D3 it is the model only (fade 0). Between, the fade byte is
  `128·(d−D3)/(D2−D3)` and the plant is drawn both ways. The biplane state (free06, D3 300, D2 350)
  had bytes 30 at a scaled 312.1 and 108 at 342.5, which is the rule exactly. When it builds its lists
  the pass puts a fade of 0x61 and up in the solid billboard list, a lower non-zero fade in a third
  list, and every non-flat plant in the model list. A plant that went more than 3 frames unseen pops
  back at fade 0. Plants are bucketed by depth, and only the nearest 1500 are listed.
- **Billboards.** Material `springtime2billboard.psm`: `Vu1EcoSystem2D.vuo`, alpha test 50, blended.
  Each species' atlas rectangle is two widths across, split at the trunk u: TreeB spans 19.77 left and
  20.23 right, 50 high, the same values RAM has.
- **Solid records.** Each tree species has one (kind 1, at 0, radius 1.5, height 30–50) and bushes have
  none. Some plants carry a bend state (an angle with a 2° and a 5° scale, states <2 and 4), so the
  original's trees appear to react to being hit. That belongs to the sim and was filed as S94.
- **Other scenes.** Every scene on the disc that names an ecosystem reads to the last byte: 8 free
  scenes (with their _ss twins) and nat01..nat22, which hold up to 20 560 plants each. The nationals'
  terrain does not read in `game_sim::track::Terrain`, so their plants cannot be placed (S95).

## How it was proved

- The format: the whole file parses with no bytes left over on all 30 scenes
  (`every_scene_that_names_an_ecosystem_reads_it_to_the_last_byte`).
- Placement: `free09s_ecosystem_places_the_originals_trees_and_bushes`. It checks 726 plants, the 11
  models and their per-species counts, every plant within ±2 ft of the sim's ground with the median
  under 0.1 ft, and the first plant's position and size bytes. It also checks TreeB's scales and
  billboard span against the RAM values, that every billboard has a texture with the material's
  cut-out, the matrix a near plant gets for a (3, 0, 4) heading, that a turned model stays turned
  while near, and, for every plant, that model / both / billboard follow `eco::look`.
- The fade rule: `the_distance_rule_is_the_originals` checks the biplane state's two fade bytes.
- Save states read: `first_recording.p2m2_SaveState` and `recording2` (free09, camera at the start;
  only 36 plants visited, all past D2) and `biplane.p2m2_SaveState` (free06, frame 751, the fade
  bytes). These were read with a scratch python reader of the eco object, not committed.
- By eye: `openmx --track free09` shots from the start line and from beside the first plant
  (`--trace-at 3432,3680,0`) in both renderers. The trees stand upright on their spots, the near ones
  as models and the far ones as cut-out pictures. Shots are not kept (scratchpad).

## Not verified / not 1:1

- **No side-by-side with the original's picture.** The frames were judged by eye only. PCSX2 was not
  started for this task, because placement and sizes were proved from the file and RAM instead.
- **Billboard look is by eye.** The original's `Vu1EcoSystem2D` program was not read. The port faces
  the picture to the camera's horizontal heading with world up, lights it as an upward normal under
  the material's diffuse path, and scales it by w/W and h/H per plant. The w/W, h/H scaling is assumed:
  the program that builds its quad was not decompiled.
- **The fade is by eye.** The original's three lists are known (solid billboard at fade ≥ 0x61,
  another list below that, the model throughout), but not how the 3D item or the third list use the
  fade byte. The port draws the model solid and the picture at alpha min(1, fade/97). The software
  renderer draws a picture only once it is solid.
- **Turn when the camera heads exactly along x.** The port follows the original's (32767, 0) when
  fwd.z is 0. Its "changes side" test is approximated: a plant is turned when it is first modelled
  after being flat or unseen, per grove and not per view. With two views the first view to bring a
  plant near turns it.
- **Not ported:** the 3-frames-unseen pop to fade 0, the nearest-1500 cap (no scene comes near it in
  one view), and the original's own box frustum cull (the port culls a sphere).
- **Unknown fields, read and dropped:** the species' three words (1, 1, 0x5a5b0b on free09); the
  version-2 nine floats (9.09, 0.222, 0.222, 0, −1.667, 0.5 ×4) and its optional string; the 12 bytes
  after each solid's position; the header's word before the probability map; the probability map
  itself.
- **Shrubbery** (`.ess`, `ShrubberyDistLimit`) is not ported. Its field is read, but no free scene's
  `.esb` names a shrubbery file. The per-frame shrubbery branch was seen but not followed.
- **Snap to ground** (bit 7) is kept as `Plant::snap` and ignored, as the scene loader ignores it. That
  rests on the loader's argument being 0, which was read in the decompilation and not traced live.
- **Solids / collision are not in the sim** (S94). That needs bit-exact work and is outside this
  renderer task.
- **Nationals:** their `.esb` files parse, but their plants are unplaced because their terrain does not
  read (S95). The shell cannot ride them yet either.
- **Other free tracks by eye.** free07's start-line shot shows its pines standing on the ground, as
  models near and pictures far, in the software renderer. free10's start-line shot shows no plant in
  view, and I did not look for its plants elsewhere. free06 was not shot. Their files parse and load
  the same way, but those models (DeadBushA, rescuetreeA, Choia, …) and their billboard atlases were
  not checked on screen.

## Next

Nothing for V48. S94 and S95 are filed.
