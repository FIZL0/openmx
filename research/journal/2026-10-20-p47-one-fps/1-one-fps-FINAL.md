# 1 — one fps (FINAL)

Task P47. 2026-10-20.

## Why it ran at a frame a second

The cause was drawing, single-threaded, at the window's own pixels. The simulation, input, menu
logic, logging and file reads were all ruled out.

- **The cause.** P18 draws at the window's own pixels: 1890x1018 here, 6.7 times the PS2's
  640x448. V25 then gave the ground its splat paint: eight `.psm` passes plus the baked light map, so
  nine bilinear samples, a `sin_cos`, eight `rem_euclid` pairs and libm `floorf` calls for every
  ground pixel. All of it ran on one core of 32. The ground alone was 430–560 ms a frame at that
  size, the sky 32–43 ms and the props 24–31 ms. The result was 0.8–1.9 fps windowed, the user's
  "about 1 fps".
- **The menu had the same disease.** P36/P37's menu stretches a full-frame background (bilinear,
  libm `roundf`/`floorf`) and two full-height column textures on every frame, on one core: 146 ms
  a frame on the main menu at 1890x1018, about 7 fps.
- **Ruled out, with the numbers from `--timings`:**
  - The simulation: 0.14–0.22 ms a frame for 1P, 0.6–0.7 ms for 4P.
  - Animation and skinning: 0.03–0.3 ms. Skinning is redone only when the level of detail changes.
  - Input, including P35's device scan: 0.1 ms in the old window, 0.02–0.04 ms now. The scan runs
    every 0.5 s and only reads sysfs for js nodes not yet open.
  - Logging: 0.00–0.01 ms.
  - HUD: 0.0–0.4 ms.
  - Present: 4–9 ms. It is an X11 PutImage of 7.7 MB through the socket, with no vsync wait and
    nothing blocking.
  - Per-frame file or disc reads and texture decodes: none. The new counters in
    `game_render::tally` and `/proc/self/io`'s `rchar` both stand still after the first frame, in
    1P and 4P. `front.save()` and `picks.save()` write only when something changed.
  - The build profile: the shell's own docs (`main.rs` header) say `cargo run --release`. P1 and
    P2's plan lines said `cargo run -p openmx`, which is the dev profile at opt-level 1. Measured,
    the dev build is not the cause: at 640x448 it now takes 19.7 ms against release's 16.9. It does
    now say on stderr that it is a debug build.

No `perf` here: `perf_event_paranoid=2` and `ptrace_scope=1`. Profiles came from a sampler that
starts the shell with `PR_SET_PTRACER` and runs `eu-stack` on it 60 times. Stage times came from the
new `--timings` flag.

## What I did

- **`--timings`** (`crates/openmx/src/stages.rs`, new). The flag times every stage of a ride frame
  and prints it as a line a second, plus a session line:
  `input menu sim anim camera sky ground scene hud present log pace, all`. The session ends with
  `after the first frame: N disc reads, N archive reads, N texture decodes, N bytes read`. The front
  end prints `front end: N frames, X ms a frame before the pace`.
- **`game_render::tally`** (new). Counters for image reads (`disc::read_at`), `.res` member reads
  (`Res::read`, `Res::read_suffix`) and `.pst` decodes (`Texture::parse`).
- **Banded, threaded rasteriser** (`raster.rs`). This is the main fix. `Batch` takes triangles
  clipped and set up as they are queued. `Frame::draw(batch, paint, fog)` bins them into row bands
  and fills the bands on every core, each band in queue order. Every pixel therefore sees the same
  triangles in the same order with the same arithmetic as `triangle_with`, and frames come out
  byte-identical. `triangle_with` lost its per-triangle `Vec` (the near clip is `clip_near` on the
  stack). `OPENMX_THREADS=n` overrides the core count, and 1 is the old sequential path. A wasm
  build gets 1 from `available_parallelism`.
- **Ground** (`openmx/src/ground.rs`). Cells go into one `Batch`, tagged painted, baked or
  unpainted, and are drawn by one `Frame::draw`.
- **Cheaper splat paint, same numbers** (`splat.rs`, `pst.rs`):
  - Each pass's `sin_cos` is taken once, at parse.
  - `sample_smooth_parts` wraps power-of-two textures with a mask instead of `rem_euclid`.
  - It uses an inline floor, `pst::floor`, an integer cast for |x| < 2^23, with a test against
    `f32::floor`.
  - It mixes only the channels the pass's blend reads. `Splat::reads` derives these from the GS
    blend inputs: rgb when A, B or D is Cs or there is no blend, alpha when C is As or the next pass
    reads Ad.
  - Single core, the ground went from 386 to 294 ms at 1890x1018.
- **Sky after the ground** (`sky.rs`, `main.rs`). The sky already filled only pixels with no depth.
  Drawn after the opaque ground, it skips about two-thirds of the frame, and the bands the ground
  covered are skipped whole. The rest is filled on every core. The picture is the same: the ground
  is opaque with depth-write on, and the sky only fills depth 0.
- **Props** (`world.rs`). `draw_scene` queues every placement and extra into one batch. The vertex
  work (transform, light and fog per corner) is spread across cores a run of models each, and the
  runs are joined back in order (`Batch::append`). In a four-way split that serial vertex work was
  most of the scene stage. `draw_model` keeps its signature.
- **Menu** (`menu.rs`):
  - `blit_as` fills row bands in parallel for any stretch over about 32k pixels.
  - `bilinear`'s `floorf` became a cast. Both coordinates are clamped to 0 or more first.
  - `roundf` became an exact `round` for 0..=255, with a test against `f32::round`.
  - The big stretches are memoised (`Art::blits`). The key is texture, source, box, tint, put
    function and the exact pixels under the box. The same stretch over the same pixels gets last
    time's pixels back, so the background and columns cost a compare and a copy. The cache holds 8
    and drops the oldest. Over the pause the ride differs every frame, so it never hits there and
    only costs the copies.
- **Tests** (`crates/openmx/tests/one_fps.rs`):
  - `a_headless_ride_at_the_default_frame_keeps_to_its_budget`: 120 headless frames at 640x448 in
    under four 60 Hz frames each (66.7 ms; it was 129). The ground must be under 50 ms (it was 111),
    the sky under 8 and the props under 20.
  - `nothing_is_read_or_decoded_after_the_first_frame`: a 4P headless ride must print
    `0 disc reads, 0 archive reads, 0 texture decodes, 0 bytes read`.
  - Both were seen running, not skipped: with the budget flipped to 0.01 ms the first failed at
    16.8 ms a frame in the test build.

## Before and after, one run (19:08, load average 7–11 of 32 threads)

`openmx-before` was built from this branch with only `--timings` added. ms a frame, `--track free06`:

| case | sky | ground | scene | present | all | fps |
|---|---|---|---|---|---|---|
| headless 640x448, before | 4.76 | 60.92 | 5.88 | – | 71.74 | 14 |
| headless 640x448, after | 0.55 | 6.71 | 1.68 | – | 9.1 + 7.8 pace | 60 |
| headless 640x448 4P, before | 4.86 | 74.32 | 24.04 | – | 104.05 | 9.6 |
| headless 640x448 4P, after | 1.36 | 19.80 | 8.79 | – | 30.84 | 32 |
| headless 1890x1018, before | 32.31 | 433.35 | 24.08 | – | 490.02 | 2.0 |
| headless 1890x1018, after | 2.06 | 27.54 | 3.13 | – | 32.98 | 30 |
| headless 1890x1018 4P, before | 32.74 | 441.75 | 71.06 | – | 546.75 | 1.8 |
| headless 1890x1018 4P, after | 3.39 | 50.29 | 16.84 | – | 71.92 | 14 |
| window 1890x1018, before | 34.26 | 459.74 | 26.73 | 5.63 | 526.74 | 1.9 (17 frames in 8.4 s) |
| window 1890x1018, after | 2.51 | 37.91 | 3.94 | 3.57 | 48.24 | 21 (167 frames in 8.0 s) |
| window 1890x1018 4P, before | 34.62 | 476.46 | 79.74 | 5.41 | 597.70 | 1.7 (15 frames) |
| window 1890x1018 4P, after | 3.42 | 53.46 | 17.02 | 4.13 | 79.54 | 12.5 (102 frames) |
| window `--scale 2` (945x509), after | 0.79 | 12.91 | 2.36 | 2.61 | 19.48 | 52 (310 frames in 6 s) |

Earlier in the day, under heavier load (12–22), the same windowed 1P was 590–1290 ms before and
86 ms after.

The menu at 1890x1018, main screen, ms a frame before the pace:

| | ms a frame |
|---|---|
| before | 146 (measured with the same timing code before the blit changes; not back to back, see below) |
| threads only | 49 |
| plus cast floor and inline round | 32 |
| plus the memo | 12.4 (59 fps) |
| plus the memo, `OPENMX_THREADS=1` | 13.7 |

The other screens are 13–19 ms (`tracks` is the dearest, at 19).

Thread scaling of the ground alone, 1890x1018: 1 thread 382 ms, 8 threads 61, 16 threads 42,
24 threads 34, 32 threads 36. The box is shared, so the top end is whatever the other slots left.

## For P44 (GPU renderer)

- The software rasteriser now has a batch interface. `Batch::triangle(shades, tag, Mode)` queues,
  and `Frame::draw(&batch, paint, fog)` fills in parallel bands. `paint(tag, u, v) -> rgba` is the
  only per-pixel callback. The tag says which texture or paint a triangle uses, and `Mode` holds
  the cut-off, blend, additive and depth-write.
- What still costs, at 1890x1018:
  - The ground's splat paint: about 180–230 ns a painted pixel on one core, about 1.3M painted
    pixels a frame.
  - The edge scan: about 16M bounding-box pixels tested for 4.7M inside. Most of the inside ones
    are then rejected by depth.
  - Present: 4–9 ms of PutImage through the X socket. MIT-SHM would remove it.
  - Everything else is under 4 ms.
- The paint is a function of the world (x, z) alone. A GPU can run the same eight passes per
  fragment, or bake them once into a texture at a cost to 1:1.

## How it was proved

- **Pixels.** Every change was checked with `same.sh`: the same 90-frame tape
  (`context/p47/t90.tape`) through the baseline and the new build, comparing the `--shot` at frame
  80 byte for byte. Cases were 640x448, 960x540, `--players 4` and 1890x1018: identical every time.
  The menu was shot on all seven screens (`--screen main|tracks|garage|settings|controls|keyboard|pause`,
  1890x1018, frame 3, which is after the memo has hit) through both builds: identical.
- **The simulation.** The tape's ride hash is `0x274745007caf63bb` in both builds and both
  thread counts. The simulation code was not touched.
- **Tests.** `./kit check` passed with the new tests and the unit tests (`pst` floor, menu round,
  tally, stages).

## Not verified / not 1:1

- **The numbers are from a shared machine.** The load average was 6 to 22 while measuring, from
  other slots' agents and test runs, and the 32 threads were never mine alone. Before and after are
  back to back in each table, but a frame's cost moved by up to 2x between minutes. On an idle box
  both would be lower, the threaded "after" more so.
- **The menu's "before" (146 ms) is not back to back with its "after".** The old binary has no
  front-end timing line. It was measured with the new timing code, minutes before the blit
  changes, on the same screen and size.
- **Not ridden with a real pad in a real window by a person.** No keyboard or pad input reaches
  windows on this desktop. The windowed numbers are idle rides with `--seconds`.
- **Not 60 fps at the window's full 1890x1018.** That reaches about 21 fps for 1P and 12.5 for 4P
  on this loaded box. The remaining cost is the splat paint itself, which only fewer pixels
  (`--scale 2`: about 52 fps) or P44's GPU path removes. I did not change P18's default of native
  pixels, because that is a design choice and not a bug.
- **The wasm build was not timed.** It gets `threads() == 1` and so the sequential paths, which
  produce the same pixels as before at the single-core speed-ups only (the paint, about 25%). The
  web shell was not run.
- **The new parallel paths only ever ran at 1 and 32 threads, plus 8/16/24 for the ground.** The
  "bytes read" check holds only headless: a window's socket and an open pad read every frame by
  design, so windowed rides print a non-zero count.
- **The ride-test budget (66.7 ms) is loose on purpose.** A loaded box and the test profile
  (opt-level 2) have to fit under it. It catches the old cost (129 ms at this size) but not a 2x
  regression.

## Next

Nothing left for P47. P44 has the notes above. If native-size 60 fps is wanted before the GPU path
lands, the splat paint is the one place left to spend effort (SIMD over a row of pixels).
