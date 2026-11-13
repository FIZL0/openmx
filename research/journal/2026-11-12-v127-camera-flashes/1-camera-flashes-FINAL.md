# 1 — camera flashes (FINAL)

Task V127. 2026-11-12.

## What I did

- `crates/game-sim/src/camera_flash.rs` (new): `CameraFlashes`, the race activity's flash object ported
  word for word. It handles the brightness climbing and falling over a `PodiumFlashes` burst and resting at
  the floor, the ring queue of lit flashes and their times, a batch of `brightness × hidden` picks every
  0.18 s (swap-removed from the hidden pool), the lit ones' fade, and the done ones going back.
  `#![deny(clippy::float_arithmetic)]`, so every operation goes through `ps2_float::fpu`.
- `crates/game-sim/tests/camera_flash.rs` (new): holds it against S13d's live sx01 race, every tick.
- `crates/openmx/src/camera_flashes.rs` (new): the shell's `Stadium`.
  - On a race on a disc track whose name holds `sx`, it takes the scene's `cameraflash` props out of
    `look` (before anything holds an index into it).
  - It reads the `.toy`'s first `[CameraFlash]` into `FlashLevels` and the floor. It uses the truncating
    reader `game_sim::phy::number`, so `0.3` comes out as 0x3e999999, as in the original's world words.
  - It ticks the flashes once per simulated tick and draws the lit ones as actors at their fade (GPU), or
    solid (software).
  - `podium()` gives `Supercross::race_over` its `PodiumFlashes`. `player_wins()` calls `race_over` as the
    human winning a single race.
  - Two unit tests: the levels as bits, and an sx scene giving up its flashes and bursting after a win.
- `crates/openmx/src/main.rs`:
  - The `Stadium` is built after `race_start` and carried through the loader's tuple. It is ticked beside
    the track's particles, and drawn after the toys (GPU) and in the software extras.
  - It prints `race: N camera flashes, floor F`.
  - New `--race-win SECS` flag. It implies `--race`, and calls `player_wins()` that many seconds in. When
    the answer says fireworks, it calls `particles::TrackFx::finish()`, which is no longer dead code. This
    is a stand-in until S301 calls `race_over` from the real cards' finishes (noted on S301).
- `crates/game-render/src/gpu.rs`: additive materials now output colour already multiplied by the texel's
  alpha, carried as bit 2 of the material uniform's `coverage` word; layout unchanged.
  - The plain Add pipeline blends with `One`, which gives the same `Cs·As + Cd` as before.
  - A see-through ("ghost") additive draw blends with `Constant`, so it gives `k·Cs·As + Cd`.
  - Before this, a ghost dropped the texel's alpha (`k·Cs + Cd`), and every fading flash drew as a flat
    grey square (found by eye: the flash texture is flat grey RGB, and its starburst is all alpha).
  - Any other see-through additive draw (a marker bike's additive parts) changes the same way, which is
    also correct for them.
- research/oracle.txt: V127 block. research/s13d/run.sh: a note on the `flash` field and how to take a look.
- Follow-up filed: S318, tick the flashes on the simulation's generator (see below).
- Nothing taken from opentennis.

## What the original does

The race activity's loader (`./kit fn 0x002faa50`) builds the flash object at +0x414 only when the track
name contains `sx`:
- It gathers every scene model named exactly `cameraflash` (sx01: 1200), hides each one, and pools them.
- Brightness starts at 0.2, the floor is the world's `ConstantFlashPercent`, and it starts ready.

Its tick (`./kit fn 0x002f95c0`) runs every frame until the race's outro:
- **Brightness:** it ramps at the burst's rate toward `race_over`'s level while more than half the length
  is left, then back down, clamped to [0, 1]. With no burst left it sits at the floor.
- **Queue:** dt is subtracted down the queue's times. Each entry it passes is done and makes the object
  ready. The first entry still left gives every lit flash its fade, `time / 0.18`, written into model
  +0x16c by `./kit fn 0x001a1b20`.
- **Picks:** when ready, it picks `cvt.w.s(brightness × pool)` flashes. Each pick is `rand() × 2^-31 ×
  pool`, swap-removed from the pool and shown. The first of the batch carries 0.18 s.

The draws are libc `rand()` (`./kit fn 0x0040fd40`), the generator everything shares.

The material (`cameraflash.psm`) is additive (A=Cs, B=0, C=As, D=Cd), uses the prelit VU program, has an
alpha test ≥ 120 with failing texels still written to colour, and is fogged. The 64×64 texture is flat
grey, and the starburst is in its alpha.

## How it was proved

- **Capture replay, bit for bit:** `crates/game-sim/tests/camera_flash.rs` against S13d's
  `context/capture/s13d/sx01-finish.jsonl`.
  - It starts from the loader's object on the first row and ticks on each row's previous dt. Every row is
    compared on 9 words: brightness, length, rate, half, floor, ready, lit count, head, hidden count.
  - Result: 2740 ticks, 454 batches, 2 bursts (the two podium finishes the capture holds, adopted from the
    row since `race_over` itself is S13d's and proved there), 3 late recorder rows (accepted one tick
    ahead), stopped by the outro on frame 2775, peak brightness 0.30963662.
  - Mutants: `pool.len` → `times.len` in the pick count, caught. `le(length, half)` → `lt`, caught.
    `lt(left, 0)` → `le` survived, because no queue remainder is exactly 0 in the capture (equivalent
    here; see below).
- **Levels:** a unit test reads the sx01 `[CameraFlash]` text and gets the capture's world words
  0x40000000 / 0x3e999999 / 0x3e999999 / 0x3c23d70a bit for bit.
- **By eye**, the images in `context/shots/v127/` (local, not committed):
  - The original: research/s13d/run.sh replayed on copy 1 with System → Screenshot over AT-SPI every
    ~6 s. `original-stands-before-after-win.png` is the top band of two native-resolution screenshots: at
    01:09.99, before any finish, few specks; at 01:17.06, just after "Bucher wins!", a scatter of tiny
    white twinkles across the crowd.
  - The port: `openmx --headless --track sx01 --race --race-win 0.5 --frames 50 --shot … --shot-frame 46
    --shot-pose 380,130,380,90,0,60`. `port-stands-at-burst-peak.png` shows the same kind of faint
    one-pixel twinkles scattered in the tiers.
  - Up close, `port-one-flash-gpu.png` (after the gpu.rs fix, fade 0.63) and
    `port-one-flash-software.png` (full) show the starburst behind a seat row.
  - `cargo test -p game-render` passes after the shader change.

## Not verified / not 1:1

- **The draws come from a separate generator.** The original's picks advance the shared `rand()`, about
  11 draws per batch at rest and more in a burst. The shell's `Stadium` draws on its own `Rng::new()`, so
  which flashes light differs, and the simulation's generator does not move as the original's does on sx
  races. Follow-up S318.
- **Which flash is picked is not compared:** the capture holds counts, head and times, not the queue's
  flash numbers. The test checks the words that do not depend on the draws.
- **The initial fade is not read.** The fade word a flash model holds before the object first sets it was
  not read; 1.0 is assumed. The first batch overwrites it on the tick it lights, so it only matters if
  something drew a flash before that.
- **The fade is drawn as alpha.** Model +0x16c is drawn as the actor's alpha, which weighs the additive
  `Cs·As`. That the original's renderer uses +0x16c as an alpha scale was not traced in the draw code. The
  software path draws lit flashes solid (no per-actor alpha there).
- **One pick edge is clamped.** A pick whose unit rounds to 1.0 (one draw in 2^25) indexes one past the
  pool in the original. The port takes the last flash instead.
- **The `lt`/`le` mutant survives** at the queue's remainder test: equivalent on the capture, since no
  remainder is exactly 0.
- **The outro's stop is not modelled.** The original stops ticking the object once the race's outro runs.
  The shell ticks it as long as the ride runs.
- **Distance brightness was not matched exactly.** The GPU samples the flash texture's mips, so a far
  flash's starburst averages down and its twinkle is fainter. Whether the original mips this texture was
  not checked. By eye the twinkles look alike at native size, but the original's look slightly brighter.
- **The fireworks were not compared by eye.** The original's race camera never framed them in the 14
  screenshots taken around the finish, and no port pose tried showed them. `TrackFx::finish()` switching
  every `FireworksNN` on is V45's, and the log shows the win answering fireworks = true.
- **The second `[CameraFlash]` section** in sx01.toy (floor 0.025) is ignored, as the original's first
  match is the one its world holds (the capture's floor is 0.01).
- **The stunt burst levels** (`PlayerStuntBurstPercent`, `AIStuntWinBurstPercent`, world +0x540 / +0x548)
  are not read: their callers were not traced, and nothing in `race_over` uses them.
- **Real finishes don't trigger anything yet.** `--race-win` is a stand-in: the shell does not count laps
  yet, so nothing calls `race_over` from a real finish. Noted on S301.
- **Only race rides have flashes.** A free ride on sx01 has none, as in the original, where only the race
  activity's loader builds the object.

## Next

Land. S318 and S301 carry what is left.

## Gate

The first full gate was red only on atv_quad's `the_sweep_that_hits_the_palm_is_the_ports` (MD119's,
merged at the branch's base, red there too: words at the triangle test's scratch). K51 (42d6c765e)
fixes it on main; it is cherry-picked onto task/V127 unchanged, not merged.
