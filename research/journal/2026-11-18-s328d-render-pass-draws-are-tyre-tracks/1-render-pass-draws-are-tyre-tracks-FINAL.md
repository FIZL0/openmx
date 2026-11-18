# 1 — render pass draws are tyre tracks (FINAL)

Task S328d. 2026-11-18.

## What I did

- Took the brief's first option and taught ps2-oracle (`crates/ps2-oracle/src/ee.rs`) what the render
  needs:
  - `vclip`: x, y, z against |w|, six bits shifted into a 24-bit clip flag register.
  - `cfc2`/`ctc2` for the clip flags (vi18) and vi0–vi15 (new `Ee::clip`, `Ee::vi`).
  - MMI `madd`/`maddu` and their pipeline-1 forms (the render's distance test uses R5900 3-operand
    `mult` then `maddu`).
  - `Ee::io`: opt-in. Hardware registers (0x10000000–0x14000000) act as storage: a write is kept, and a
    read returns it. A DMA channel's CHCR start bit reads clear at once. Off by default, so every other
    test still panics on a stray hardware access.
  - Three new tests in `crates/ps2-oracle/tests/interpret.rs`: vclip shifting and 24-bit wrap, madd
    with carry into HI, io storage with the CHCR bit.
- `crates/game-sim/tests/ai/draws.rs`:
  - `render_setup`: io on; every kernel call stub (`addiu $v1, n; syscall`) returns 0 (only FlushCache,
    100, is reached); three hardware waits hooked to return 0.
  - `render`: calls the application's vt+0x38 (`app_render`) after each whole frame.
  - The S328 test now runs the render every tick and says where its draws come from. New site
    `Site::Track` (track_strip_add).
  - New table `C33_TRACKS`: which tyre tracks lay a row on which ticks, like `C33_DUST`.
  - `c33_entries` lists each rider's manager entries; `c33_emitters` now uses it (same output).
  - The old "capture steps at least as many" check is now exact equality per tick. The generator is
    held through the window's end (63) instead of 16.
- oracle.txt: new names `app_render`, `dma_channel_sync`, `dma_vif1_drain`, `vblank_wait`,
  `vehicle_fx_tracks_walk`, plus an S328d note.
- Filed S448: put the tyre tracks' rows on the Sim's generator.
- opentennis has no vclip or register model to take (grepped its crates).

## What the original does

- The render (`./kit fn 0x0016a7c0`, app vt+0x38, run by engine_tick after the second marker) draws
  rand at exactly one place: track_strip_add's row height, h = HeightAboveGround + rand()·2⁻³¹·0.005
  (`./kit dis 0x3d9000`, return 0x003d9044). It's already described under track_strip_add in
  oracle.txt.
- Tyre tracks are updated **in the render pass**, not the sim half. The vehicle fx manager's render
  walk (`./kit fn 0x003d5a00`) calls vt+0xc (vehicle_fx_track_update) on each entry whose kind byte
  +8 is 1. In C33 those are entries 12 (front tyre) and 13 (rear) of each rider's +0x1490 manager. The
  walk order is ai1, ai0, ai2, human.
- One draw per row laid. Per tick, each track lays 0 or 1 row (a row each 2 ft of tyre travel). C33:
  - ai0's and ai2's rear tracks from 17;
  - ai1's rear from 21;
  - ai0's front at 57–60;
  - gaps on slow ticks (e.g. ai0 rear at 27, 36, 43–61);
  - the human never lays one (no throttle).
- In this window the sim half draws nothing for tracks, and the render draws nothing else.

## How it was proved

- A save-state capture. C33's image (`free09-ai4.ee.bin`) is run whole in the interpreter for 60 ticks:
  sim half, then render. It is checked against `free09-ai4-agreed.csv`. The test
  `s328_c33s_other_draws_are_looks_notes_and_dust` asserts, every tick:
  - all of the S328/S328a–c checks still hold, with the render run between frames;
  - every render draw comes from track_strip_add, and no sim-half draw does;
  - each track's update draws 0 or 1 time, and the tracks that draw are `C33_TRACKS`' list;
  - the frame's total draws equal the capture's generator steps, exactly, on all 60 ticks;
  - the generator state is bit-equal to the capture's through tick 63.
- `./kit check -- -p game-sim --test ai` and `./kit check -- -p ps2-oracle` pass.

## Not verified / not 1:1

- **The render is run with hardware faked.**
  - DMA transfers complete at once and move no data.
  - GS, VIF and timer registers read back what was written (0 before any write).
  - The three waits and all kernel calls return 0.
  - So the render's DMA/GS side effects are not modelled. The draw counts are right on every tick,
    which suggests nothing that draws depends on them. That is not proven for other scenes, for
    example something that reads a GS or VIF status to choose a path.
- **Only ticks 4–63 of C33, one track (free09), four bikes.** Other tracks, wrecks, machines and
  split screen were not run through the render. Their render passes may draw elsewhere (e.g. other
  effects whose update lives in the render).
- **The track's row values are not compared.** The test checks which tracks draw, and that the
  generator matches. It does not check the row positions or heights against the original's ring. That
  is S448's job and is render-side anyway.
- **vclip edge cases:** NaN/inf/denormal operands go through `vu::operand` (max-float and zero), as
  the VU's other inputs do. No oracle or PCSX2 check of vclip on those values was made. The test only
  covers ordinary values and the on-bound case (on the bound is inside).
- **The Sim still parts at 17.** No product code changed. The tyre tracks' rows are not drawn in the
  Sim (S448), and openmx's tire_tracks.rs keeps its own noise.

## Next

S448: the tyre tracks' rows on the Sim's generator, after its sim half, checked against `C33_TRACKS`.
