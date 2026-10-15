# 1 — v1 first draw (FINAL)

Task V1. 2026-10-15.

One frame of a supercross track, with a bike standing on its start line, drawn from the user's own disc at
runtime: no extracted assets, no conversion step, no dependency outside the workspace. `crates/game-render`
opens the ISO, pulls the track and the bike out of `MX.PSR`, decodes four of the disc's formats, and
rasterises the result into a PNG.

```
$ ./target/release/first-draw --track sx19
disc: Tracks\sx19.res, Bike\250sx.res
scene: Supercross19 — 1480 props, start V3 { x: 210.78, y: 90.06, z: 318.26 }, terrain SX19.trn
terrain: 257x257 samples 3 apart, ground at 90.06, 27.6 feet of relief
height band: a word is 1/560 of a foot, fitted to 169 props that rest on the ground, half of them within 0.08 feet
loaded: 55 of 47 models, 54 textures, bike
camera: Race Cam 1 — 13 feet back, 25 degrees up, 85 degrees across
drew 56856 triangles from V3 { x: 214.178, y: 95.554, z: 329.541 }
```

That is 0.07 s of work from a cold start on a release build, including the disc reads.

## What I did

### The chain

`disc.rs` reads ISO 9660 directly, finds `MX.PSR` and a `.res` member of it (D5's two formats), and hands
out bytes. Everything above it is a parser:

- **`.psg` — geometry.** A header, a node tree (named transforms: `Bike`, `Handlebars`, `Wheel_Front`…), a
  material table, then per-LOD groups of VIF streams. The streams are the PS2's own: `UNPACK` commands
  carrying positions (V4-32), normals, UVs and vertex colours, with the strip's cut bits in the top bits of
  the position's `w`. Decoding them gives triangle strips in model space, which is what the VU1 microprogram
  the material names (`Vu1StripDiffuse.vuo` for nearly everything) would have transformed.
- **`.pst` — textures.** 8-bit indices into a 256-colour palette, plus a mip chain. Two quirks, both
  confirmed across all 143 textures of a track: the palette is in the GS's order (the middle two blocks of
  eight in every 32 are swapped — `(i & 0xE7) | ((i & 0x08) << 1) | ((i & 0x10) >> 1)`), and alpha is the
  GS's, where 0x80 means fully opaque. The header stores the *smallest* mip and a count, so the full size is
  `last << (mips - 1)`; that reproduces the exact file length of every texture on the disc, which is the
  check that the header is understood.
- **`.scn`, `.psm`, `.sub`, `cpresets.ini` — text.** One INI-ish parser in `ini.rs` serves all of them. A
  scene is a start position, a terrain file, fog, two lights and then ~1500 `[ModelN]` sections, each a model
  name and three vectors; a material names a texture; a `.sub` says how a bike is built; `cpresets.ini` is
  the follow camera.
- **`.trn` — the ground.** Below.

`world.rs` matches them up by name (`SLT=bale_single_02` → `geom\bale_single_02.psg` → material
`bales_01_03` → `tex\bales_01_03.psm` → `bales_01_03.pst`), `raster.rs` draws: near-plane clip,
perspective-correct interpolation, depth on 1/w, per-vertex Lambert against the scene's sun plus its
ambient, linear fog to the scene's fog colour between its own two distances.

### `.trn`, the terrain

```text
0     "PLN\0" '7' "ps2\0" "UnCompressedPSXPal8\0"
45    256 x RGBA palette, in the same GS order as a .pst
1069  f32 spacing (3.0 on every supercross track, and the scene's TerrainWidthScale agrees)
1073  u32 1, u32 size (near but not equal to the file length — unexplained)
1081  u32 columns, u32 rows   (257 x 257 on all 24 supercross tracks)
1089  (patches + 1) x u32 offsets, relative to the END of this table
...   patches x 2129 bytes: a 16-byte header, then 17x17 u16 heights at 16 + 40*r + 2*c
...   1 MiB ground texture (256 px wide), then 24220 bytes of 388-byte records
```

The one thing worth writing down is **what the offsets are relative to**. Taking them from the start of the
table puts the patches 1028 bytes early, which still parses — the height field looks like noise but a
plausible noise. Taking them from the *end* of the table (byte 2117 on a 257×257 track) gives, over 256
patches, exactly one distinct 16-byte patch header and 36% of all height words equal to a single value; the
wrong base gives 147 distinct headers and 0.1%. That one value is the editors' "flat ground" word, and it is
per-track (`0xAAAA` on sx01, `0xC3AC` on sx19), which is the first hint that the height band is per-track
too. Every offset is then `patch * 2129`, and the parser checks that for all 257 entries and refuses a file
that disagrees rather than guessing.

A 16-quad patch does not divide the off-road tracks' grids (538, 794, 842 wide), so those are refused
outright: they are laid out some other way, and V1 only needed a supercross track.

### The height band, and why it is fitted

A height word is a `u16`; a world height is `raw * scale + base`. Neither number is in the file. I looked
for them in the `.trn` header, in a whole-file f32 scan, in the `.scn` (`TerrainWidthScale = 3.0`,
`DetailMapTilingMultiplier = 8`, and nothing vertical), in the `.toy` (terrain *zones* — friction, not
height), and in the decompilation (the EE image is stripped; nothing keys off 43690). It is not there.

So `Terrain::calibrate` recovers it from the scene:

- **The slope** comes from the props. A track is dressed with ~170 bales, and a bale rests on the dirt, so
  each one is a `(height word under it, its own y)` pair. Least squares over those pairs, then three passes
  that throw out the ones that disagree (the banners and the gantry, which hang in the air), converges to a
  median residual of 0.06–0.20 feet on every supercross track. The slope is genuinely per-track: 1/540 of a
  foot per word on sx01, 1/560 on sx19, 1/627 on sx02, 1/699 on sx09.
- **The offset** comes from the start line. A bale's origin is not at its feet, so the fitted line sits
  about 1.3 feet above the ground; the scene's `DefaultStartPosition` is the one point whose world height
  the game itself states, so the band is shifted to pass through it.

That second step is checked against the running game, below.

### The bike is a kit, not a model

`bike.psg` is the shared chassis. `250sx.sub` bolts on seven sub-objects — engine, exhaust, plastics and
frame at the node `Bike`, front fender, number plate and crossbar at `Handlebars` — so each part's transform
is that node's place in the chassis followed by the chassis's place in the world. It also carries a
`[Material Changes]` section (`CR-250_* = 250SX_*`, `CR-250_wheels = RM-250_wheels`): without applying those
renames, half the bike's meshes ask for a material no archive on the disc has, because the chassis is the
CR 250's geometry wearing another bike's paint.

Two smaller things the disc taught me: the menus keep a second copy of each bike one directory deeper
(`UI\Bike\250sx.res`, the spinning showroom model), so the shortest matching member name is the one a race
wants; and a track is *two* archives, `sx19.res` and `sx19_2.res`, the second holding the bike's animation
set, its physics and the follow cameras.

### The camera is the game's own

`cpresets.ini` ships with each track: eleven follow-camera presets, plus a copy of each for every
split-screen layout. The first is "Race Cam 1" — `AngleY = 180` (straight behind), `AngleX = 65`,
`FollowingDistance = 13`, `LookAheadDistance = 20`, `FOV = 85` horizontal at 4:3. Reading `AngleX` as
degrees away from straight up puts the camera 5.5 feet up and 11.8 feet back, which is what the real
game's frame looks like; reading it from the horizon would hang the camera nearly overhead, which it
plainly is not. The game filters that camera (`PosFilters`, `CameraSwing`); a still frame does not.

## What the original does

Not read. The formats here were recovered from the bytes, not from the loader: the EE image is stripped and
R1's 8,558 functions are unnamed, so finding the five parsers would have been a task of its own and the task
said to stop at "recognisably the same scene". What the original does that this does not: run the geometry
through VU1 microprograms named by each material, light it with the GS's own blend modes, and draw a rider,
a sky cube, shadows and particles.

The one place the original's behaviour was consulted is the ground height, through RAM rather than through
code (`./kit pine read <addr> 3 --as f32`, address found by scanning a 32 MB EE dump for the scene's start
position): with the live game parked at the sx19 start gate, the bike's world position reads
`(210.734, 89.999, 323.819)` where the scene's start says `y = 90.06`. The game puts the rider on the dirt
at the height the scene states, which is what `calibrate`'s offset step assumes.

## How it was proved

- **By eye against `./kit shot`, at the same place on the same track.** The emulator was on Bridgeport; I
  identified it as `sx19` by dumping EE RAM and finding the one resolved terrain path in it
  (`Tracks\SX19\SX19.ps2.trn`), then restarted the race from the pause menu to get a frame at the start
  gate, which is the viewpoint `first-draw` renders. Real and rendered side by side: same stadium (roof,
  stands, the banner wall across the far end), same brown rutted dirt, same bike from behind at the same
  distance and roughly the same angle. Differences, all explained below: the real frame has a rider on the
  bike, a detail texture of tyre tracks on the dirt, and a brighter exposure. The pair is in
  `context/shots/` (`v1-sx19-render.png`, `v1-sx19-real.png`) — git-ignored, as all game-derived images are.
- **All 24 supercross tracks parse and draw**, each with a height band fitted to 115–188 of its own props
  with a median residual of 0.06–0.20 feet. A contact sheet of the 24 start lines was eyeballed: all 24 are
  a stadium with dirt in it.
- **The formats carry their own checks.** Every `.pst` on the disc reproduces its exact file length from its
  mip header; every `.trn` patch offset must equal `patch * 2129` from the end of the table; a `.trn` grid
  that is not a whole number of patches is refused.
- **81 tests pass** (`./kit check`), 21 of them in `game-render`: the palette order and its involution, the
  mip-size rule, the strip-to-triangle expansion, the INI quirks, the scene and material and `.sub` and
  camera-preset parsers, the rasteriser's depth and clip, the PNG writer, and four on the terrain — the
  patch grid, flat ground and its normals, the refusal, and a calibration that recovers a synthetic band
  through props that stand a foot and a half above it.

## Not verified / not 1:1

- **Every format here was read from the bytes, not from the game's loader.** `.psg`, `.pst`, `.trn`, `.sub`
  and the INI dialect all parse every file of their kind on the disc and draw something that looks right,
  which is evidence but not proof. Where a field is still unexplained the code says so instead of naming it.
- **The terrain height band is a renderer-side calibration, not the game's constant.** Slope from a least
  squares over the track's bales, offset from the scene's start position. It is per-track (1/540 to 1/699 of
  a foot per word) and I could not find where the game keeps it. Two tracks (sx09, sx24) come out with 94
  and 98 feet of relief against 10–28 feet elsewhere; the fits are just as tight, so either those tracks
  really do have a mountain in them or their bales mislead the fit. Nothing checks this against the game.
- **The ground texture's layout is unknown.** The 1 MiB block after the patches is 256 px wide; whether its
  sixteen 256×256 blocks are sixteen ground types indexed by the patch header's bytes (`0c0c0c0c` on sx01,
  `02 00 00 00` on sx19 — never decoded) or one tall image is unresolved, and so is the per-quad UV mapping.
  The first draw repeats the first tile every `spacing * DetailMapTilingMultiplier` = 24 feet. The real
  game's dirt carries a second, finer detail texture that this does not draw at all.
- **Unexplained fields, all skipped:** the `.psg` 9-byte trailer; the `.pst` first eight bytes (identical in
  every texture); the `.trn` words at 1073 and 1077 (the second is near but not equal to the file length);
  the 16-byte patch header; the 1,535 bytes that follow each patch's 578 bytes of heights; and the 24,220
  bytes of 388-byte records at the end of the file.
- **Off-road tracks do not draw their ground.** Their `.trn` grids (538, 794, 842 across) are not a whole
  number of 16-quad patches, so `Terrain::parse` refuses them rather than guess at a second layout. Their
  props and the bike still draw.
- **No rider.** The task asked for one actor and the bike is it. The rider lives in `UI\Rider\*.res` as
  skinned geometry that needs the skeleton in `bike.mcf` and the motions in `*.mot`, none of which are read.
  The real screenshot has one and the render does not, which is the largest visible difference between them.
- **No sky, shadows, particles or animation.** `CubeFile = sky43.cub` is ignored (the sky is the fog
  colour), `CastsShadows = T` is ignored, the eleven `.emt` emitters are ignored, and every model is drawn
  in its rest pose at its finest LOD — no LOD switching, so a distant prop costs what a near one does.
- **The shading is not the original's.** One directional light plus ambient, Lambert, nearest-neighbour
  texture sampling, no mip selection, no alpha blending beyond a cutout, no vertex-colour convention beyond
  "0x80 means unchanged", and linear fog. The PS2 would have run the material's VU1 microprogram and the
  GS's own blend and texture modes. "Looks right" is the whole of the standard met here.
- **The camera's angles are my reading of the preset**, not the game's maths: `AngleX` as degrees from
  vertical and `FOV` as the horizontal field at 4:3 both match the real frame by eye, and nothing else
  checks them. The preset's filters, swing and speed terms are not implemented, because a still frame has no
  history to filter.
- **The by-eye comparison is one pair of frames on one track.** The other 23 supercross tracks were compared
  only against themselves (the contact sheet), not against the game.
- **Environment, not the port:** PCSX2 copy 5's `[Pad1]` was bound to `Keyboard/*` while the kit disables
  the keyboard source, so no virtual pad input reached the game until I rebound that copy's `inis/PCSX2.ini`
  to `SDL-0/*`. That is the emulator copy's own config, outside the repo, but the next agent to use copy 5
  inherits it. In the game's menus, Triangle is Back, not Circle.

## Next

Nothing in V1 is left open. The two threads worth pulling, both for a later task: the patch header's bytes
against the ground texture's sixteen tiles (that is the ground's real painting, and it would also explain
the 1,535 bytes after each patch's heights), and the terrain height band in the game's own code, which
would retire `Terrain::calibrate` and let the off-road tracks be read properly.
