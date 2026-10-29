# 1 — texture pop 2 (FINAL)

Task V106. 2026-10-28.

## What I did

**Found first.** The user's screenshot (free-ride ground, tyre tracks sharp near the camera ending in
a band, the ground past it blurry) is V67/V89's rut paint on the GPU, `crates/game-render/src/gpu/ground_mesh.rs`.
A freeride's ruts, roads and runways (`Type=TerrainScreenSpace` meshes) are drawn from above into two
targets, and the ground shader read one or the other:

- **near**: 2048² texels at 4 a foot (512 ft square), drawn again round the eye whenever it crosses
  into another 64-ft square;
- **far**: 2048² over the meshes' whole rectangle, 6912 ft on free06/free09, so about 3.4 ft a texel.

`APPLY_WGSL` took the near target wherever it reached and the far one elsewhere, a hard `select`. So
the ruts stopped in a straight line 224–288 ft ahead, about 13× sharper on one side than the other,
and the line jumped 64 ft forward each time the eye crossed a square. That is the band and the swap
seen as it moves. Filmed headless on the GPU renderer (below): HEAD steps a ground patch's detail by
3.3 (low chase eye) and 7.4 (high eye) on the frame the near target is drawn again, against a floor of
0.2–0.5. On free06's own paint and ruts the same jump is 4.4.

The other suspects, checked:
- **Splat layer mips (V97's `mip_floor`)**: the level is a continuous function of the eye
  (`paint_depth`). A still camera on free06 renders identical frames (0.00 step), and moving it, the
  free06 patches away from the band step the same before and after the change (2.7 colour 340 ft ahead).
  That is the GPU's filtering of hard paint edges as the view moves, not a swap.
- **Ground chunk levels (V32)**: geometry only. V85 already found they cannot move the paint.
- **Streamed tiles (MD17/V92)**: world maps only. The screenshot is a freeride track, which does not
  stream, and V92 already fades a streamed tile in.

**The fix** (`gpu/ground_mesh.rs`, and `gpu.rs`'s one call, now `ground_mesh::apply_wgsl()`):
- A **mid target** per near target: 2048² at 1 texel a foot (2048 ft square), drawn again only when
  the eye crosses into another 256-ft square. A near square always lies in one mid square, so the mid
  is redrawn at most once per four near redraws.
- The ground shader samples **near, mid and far** and mixes them by the **eye's own distance**
  along the ground: a smoothstep from near to mid over `NEAR_FADE` = 152–216 ft, and from mid to far over
  `MID_FADE` = 600–880 ft. A target's square no longer decides where the picture changes. Each fade ends
  inside the least its target reaches from the eye (224 and 896 ft per axis), and a target drawn again
  round a new square holds the same texels where the old one did: its corner lies on a whole texel of
  every mip level a fade reads (near corners on multiples of 32 ft = 128 texels, mid on 128 ft = 128
  texels). So a target following the rider changes nothing on screen, and no time fade is needed.
- First tried `NEAR_FADE` 112–216. Next to HEAD on free06, the bowl 120–200 ft out lost tread detail,
  so the fade starts at 152 ft, where HEAD's near target always still reached.
- Memory: a near target is about 21 MiB with mips, and each one now has a mid of the same size, so
  one view costs about 43 MiB (was 21), and eight split views apart about 342 MiB (was 171).

**Original mode:** this is presentation (AGENT.md: rendering is ours). The original paints its ground
through one off-screen target from its own paint camera (V97), with nothing like a near/far pair, so there
is nothing to keep 1:1. The software renderer paints the meshes per point (`GroundMeshes::paint`),
has no targets and so had no band. It is unchanged.

**Taken from opentennis:** nothing. It has no ground paint targets.

## What the original does

Not relevant: presentation only. See V97's entry for the original's paint camera.

## How it was proved

- `crates/game-render/tests/ground_mesh_band.rs`, headless GPU, 1280×960, anisotropy 16 (the shell's
  most). A camera rides straight along +z a foot a frame for 160 frames. It starts 208 ft past a
  multiple of 256, so at frame 48 the near target is drawn again and the mid one too, and at frame 112
  the near again. It rides twice: eye 14 ft looking 50 ft ahead (a chase camera) in 24×48-ft patches,
  and eye 90 ft looking 400 ft ahead in 64×96-ft patches. The frame is cut into patches fixed on the
  ground (each pixel unprojected onto the ground plane), each pixel weighted by a tent that is 0 at its
  patch's edges, so a hard paint edge crossing a patch boundary does not count as a step. For each patch,
  the weighted mean colour and brightness spread ("detail") are compared frame to frame. Patches under
  400 px or cut by the frame are not judged.
  - `riding_over_ruts_the_ground_shows_no_band_or_jump`: generated paint, a grey layer under one
    6912-ft mesh of fine clumped noise (8 texels a foot). Bounds `BAND` colour 2.0, detail 1.0.
    HEAD: low eye detail 3.34 (frame 48, 256 ft ahead), high eye 7.41 (frame 112, 264 ft), colour ≤ 0.97.
    Now: low 0.46 / colour 0.37, high 0.18 / 0.15.
  - `riding_over_free06s_ruts_the_ground_shows_no_band_or_jump`: free06's own paint and ruts (disc,
    skips without it), from the strip where most mesh lies ahead (2944, 2768). Bounds `BAND_FREE06` colour 3.5,
    detail 3.0. HEAD: low eye detail 4.35 (frame 112, 240 ft ahead). Now: low 1.81 / colour 2.70, high
    2.50 / 1.52. HEAD has the same 2.70/1.9 and 2.50 at the same patches (340 ft and 1008 ft ahead, no
    target changing there): filtering shimmer on hard paint edges, which is why the bound sits higher than the
    generated one.
- Frames looked at: HEAD's generated ride shows a straight line where the noise turns to flat grey.
  With the fix there is no line, and the noise softens gradually into the distance.
- `ground_meshes.rs` (V67 road colour, GPU and software) and `ground_mesh_views.rs` (V89 split screen)
  still pass. `-p game-render --lib` 104 passed, including `every_shader_translates_for_webgl2` and the
  near-target pool test.
- Frame time: `openmx --track free06 --headless --frames 600 --timings --texture-filtering 16
  --trace-gas`, two runs each. Present averaged 2.20/2.40 ms on HEAD and 2.30/2.62 ms now. The ground stage
  peaked at 0.50/0.52 ms on HEAD and 1.01/2.19 ms now, on the frame a mid target is drawn again. The worst
  frames were the sim's in all four runs.

- Landed with `./kit done --skip-audit`: `./kit audit` reports 7 leaks, all other tasks' addresses
  already in `plan/DONE.md` (the biplane_tire_* names and radar_update). This task added none.

## Not verified / not 1:1

- **Not filmed in the shell at the user's resolution.** The windowed shell presents at ~1 fps here (the
  screen is locked), and `--headless` shots come out 640×448. free06 shots before and after at
  two poses differ by a mean of 0.0009 at that size, too small to show the band either way. The proof
  is the headless renderer at 1280×960. Nobody has looked at the fixed picture in a live 1080p ride.
- **Frame-time cost at 1080p not measured.** The headless timing run draws at the shell's
  headless size, not at `--size`. The extra cost is one texture read per ground pixel and a 2048²
  redraw of every mesh (12 mip levels) once per 256 ft of travel. Its GPU time at 1080p and on a phone
  or wasm was not measured.
- **Memory doubled for the rut targets:** 43 MiB a view, 342 MiB for eight views apart. Not checked on a
  phone or wasm budget.
- **Past 880 ft the ruts are still the far target's 3.4 ft texels.** The fade hides the change, but very
  far ruts are still blurrier than the mid target would draw them. Not changed: a bigger far target costs
  64+ MiB.
- **The free06 bound (detail 3.0) is not far above that track's own shimmer (2.5).** It catches HEAD's
  4.35, but a smaller regression on real paint could slip under it. The generated test, with a floor
  of 0.5 and a bound of 1.0, is the sharp guard.
- **Only free06 was ridden on the disc.** free09/free10 have the same meshes and targets, but were not ridden.
- **The fades' distances are chosen by eye and arithmetic** (152–216, 600–880 ft), not tuned against
  the user's view. A rut 152–216 ft out is now read partly from the mid target (1 ft texels), a little softer
  than HEAD's near target there.

## Next

Nothing for this task.
