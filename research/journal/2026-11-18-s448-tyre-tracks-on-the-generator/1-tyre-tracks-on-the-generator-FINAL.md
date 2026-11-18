# 1 — tyre tracks on the generator (FINAL)

Task S448. 2026-11-18.

## What I did

- New `crates/game-sim/src/tire_track.rs` (`#![deny(clippy::float_arithmetic)]`, all F/Vf):
  - `Params`, `Tyre`, `Snapshot`, `Row`.
  - `Strip::end` and `Strip::add`: the row decision. The alpha byte, the restart from ended, the draw, the anchor test, `u`, the 2 ft and `u + 1 < 8` tests, the edges, the first row, the break angle on the game's acos table (`air::AcosTable`), the new segment.
  - `Track::update`: the on/off, the fades and the strength, then add or end.
  - `RiderTracks::switch`: the manager's pass. It handles the zone change (`None` holds a track off), then switches each track on or off against `Disable`.
  - `Tracks::render`: the render walk, players last to first.
- `crates/game-sim/src/lib.rs`:
  - A new field, `Sim::tracks: Option<Box<Tracks>>`. It is `None` by default, so nothing changes for rides without it.
  - `tracks_switch(i)` runs beside each rider's `dust_frame`, in both the solo frame and `step_all`.
  - `tracks_render()` runs at the end of both, before the tick count moves. It draws on the race's generator, or the lane's, as the dust does.
- `crates/game-sim/src/snapshot.rs`: two lines. The field is skipped on the wire, like dust. This file is outside the brief's list, but the struct's destructure would not build without them.
- `crates/game-sim/tests/ai/race4.rs`:
  - `c33_tracks_seed` seeds every rider's tracks from the image: params, fades, last point, the strip's words, the frame-before snapshot through the manager's feed, and the tyres' last zones.
  - It asserts each rider's tracks are entries 12 and 13, and that every strip has segment 8, lay distance 2 and u scale 1.
  - Each tick, it asserts which tracks drew equals `c33_tracks(tick)` through 63.
  - The 17–28 draw count is now 2 per dust emitter + 1 per track.
  - `C33_AGREES_THROUGH[4]` (the generator) went from 16 to 84.
- `research/oracle.txt`: an S448 note covering the track vtable slots, the draw per call, the alpha byte and the strip end.
- Follow-ups: V135 (the shell draws the Sim's rows) and S482 (the generator parts at 86).
- opentennis has nothing for tyre marks or strips (grepped earlier in this task, and in S328d), so nothing was taken.

## What the original does

- **Manager, in the sim half** (`./kit fn 0x003d5aa0`). It reads the snapshot of the frame before (`./kit fn 0x00258d50` writes it after the manager).
  - First, for each tyre whose zone byte moved, it calls vt+8 on that tyre's tracks (`./kit dis 0x3d7f60`): a zone that reads 0xff gets the zone-off slot, any other gets zone-on.
  - Then each track switches on or off from `Disable` against the tyre's flags and the wreck flag.
  - Off ends the strip and sets out = 0, in = FadeIn.
- **Render walk** (`./kit fn 0x003d5a00`). It visits each entry whose flag byte is exactly 1 and calls vt+0xc, the update (`./kit dis 0x3d7ff0`).
- **Update.** It works out the fades and the strength. Strength ≤ 0.005 ends the strip. Above that it calls the strip's add (`./kit dis 0x3d8f10 0x3d9600`).
- **The draw is on every add call** (`jal` at 0x3d903c), before any early return. S328d said "one draw per row laid"; that is inexact. A call can draw and lay nothing:
  - on the anchor;
  - with an alpha byte of 0;
  - less than 2 ft from the last row while `u + 1 < 8`.
- **Edges** (`./kit dis 0x3d9150 0x3d92f0`): the VU normalize, opmula/opmsub for the side, FPU neg.s for the left side, then vmulx and vadd.

## How it was proved

- A save-state capture: C33 (`free09-ai4`), the image plus the agreed csv, through `crates/game-sim/tests/ai/race4.rs` `s12e_free09s_four_racers_ride_as_the_capture`.
  - The Sim is seeded from the image's tracks.
  - On every tick 4–63, the tracks whose update drew equal `C33_TRACKS`. That table came from running the original's render in Ee (S328d).
  - On 17–28, the Sim's draw count is exactly 2 per dust emitter + 1 per track.
  - The generator is bit-equal to the capture's on every agreed row through tick 84 (torn row 48). Before this task it was 16.
- `./kit check -- -p game-sim --test ai s12e_free09s_four_racers` passes. The full gate runs in `kit done`.

## Not verified / not 1:1

- **Row values not compared.** Only which tracks draw on which tick, and the generator, are checked. The edges, `u`, alpha byte, restart flags and the strip's words after each tick were never compared with the original's strip or ring. The edge arithmetic was ported from the disassembly, but no oracle has run it.
- **Only C33.** One scene: free09, four bikes, ticks 4–84, no wrecks. These were never exercised:
  - zone changes (no tyre changed zone in the window);
  - a break on the angle;
  - `MinSpeed > 1` speed scaling;
  - the disable-mask path.
- **Zone index ≥ 16** counts as on (the entry's next bytes are never 0xff), the same guess as dust. Not checked.
- **The flown-riders path (B9a, `step_all` with a plane)** calls `dust_frame` but not `tracks_switch` or `tracks_render`. A Sim with both tracks and a plane gets no track draws. No test has that mix.
- **The plane, car and quad lanes** have no tyre tracks here.
- **Walk order.** The original's is ai1, ai0, ai2, human. The Sim walks players last to first: human = 0, ai2 = 1, ai0 = 2, ai1 = 3. That gives ai1, ai0, ai2, human, the same order for C33. For other seatings it is reverse player order, which is assumed to match the original's actor list. Order changes only which row gets which jitter, not the count.
- **Not hashed or sent.** `Sim::tracks` is not in the state hash or on the wire, like dust and notes. Online, the generator would differ if one machine has tracks and the other does not.
- **The shell still uses its own dice.** `crates/openmx/src/tire_tracks.rs` was not switched over (V135). Nothing seeds `Sim::tracks` outside the test.
- **The ring is not ported.** The batch layout, fading by age and the GS side stay the shell's.
- **The generator parts at tick 86** (S482). The cause was not looked into; ticks 64+ have no per-site draw table yet.

## Next

Nothing for S448 beyond the gate. V135 and S482 are the follow-ups.
