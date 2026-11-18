# 1 — atv between frame callers (PART)

Task R44. 2026-11-17. Blocked: the callers run, but their draw counts need the IOP's voice status.

## What I did

- Added `between_frames` to crates/game-sim/tests/atv_quad/replay.rs, called by `sim_chain`
  (sim_frame.rs) before every replayed frame, on both machines. It runs two passes from the image:
  - the sound manager's update (the engine notes, the crash and landing picks, the list pick, the
    wreck clank);
  - the quad scene node's second pass (vt +0x48), which reaches the trail's sound objects and the
    effects group's dust emitters.
- Two kernel calls are stubbed so the sound code runs with no IOP:
  - the thread delay returns at once;
  - the RPC to the IOP is answered as done. Its end function's one flag store is decoded and made.
- It is off by default. `R44_CALLERS=1` turns it on and switches the fitted `RNG_LAG` off.
  `R44_RNG=<re-record csv>` also reports, per row, how many draws the replay is behind the re-record.
- The default path is unchanged; `RNG_LAG` stays, with docs saying why.
- Filed R46 to feed the replay the original's voice status.

## What the original does

- The sound manager's update: `./kit fn 103e20 --program atv`. The engine note update: `./kit fn 4a3210`.
- The engine note's loop chaining: `./kit fn 4a4740`. It plays the next loop (`./kit fn 4a40a0` /
  `./kit fn 4a4220`, one no-repeat pick each, `./kit fn 1972a0`) when the current voice is done, or
  its play position is near its length.
- Voice status: `./kit fn 14ede0` → `./kit fn 188550` reads the driver's table at
  `*(0x3dc5e0)+0x100`: status shorts +0x340, generation +0x1418, flags +0x1478, state +0x1840.
  `./kit fn 186b30` reads the play position at +0x1484. The IOP's RPC replies fill these.
- The list pick: `./kit fn 19a040` only picks (`./kit fn 19d020`, one draw) once the current item has
  stopped (+0x30 == 0), which again is voice status.

## How it was proved

- All 15 md37a rides, `*_replays_by_the_originals_frame`, with `R44_CALLERS=1`:
  - the 10 rides with no wreck pass (s- rides' ragdoll isn't asserted);
  - race-jump, race-jump-side, race-wheelie-launch fail on the ragdoll from the wreck row;
  - s-jump and s-wheelie-launch's ragdoll also differ from the wreck row (logged, not asserted).
- Draws before the wreck, callers vs. the fitted lag:
  - race-jump: 21 vs. 22. Against R41's re-record (`rng` field), the replay is 1 draw behind from row 62.
    R41's caller log shows the missing one: the list pick on row 61.
  - race-wheelie-launch: 47 vs. 50.
  - race-jump-side: 65 vs. 41. A caller log showed frames 105-132 each drawing one engine note pick,
    alternating the two loop functions: a new voice reads as done at once.
- The callers themselves match R41's log: engine note on row 8, crash pick + engine on 50, two trail
  draws a row from 56, the clank after the spin.
- Default path: `./kit check -- -p game-sim --test atv_quad race_jump` passes (4 tests).

## Not verified / not 1:1

- Voice status is frozen at the image's. The IOP's replies never come, so sounds never end or start
  in the driver's table. This is why the counts are off. Not fixed.
- The thread delay and the IOP RPC are stubbed, not run.
- The root and world-group passes are not run. They reach render code (COP0 / VU0 macro ops) that the
  interpreter lacks. R41's log shows no rand() draws from them on these rides; not checked further.
- `RNG_LAG` is still fitted, for all 7 entries. The s- idle variants are still fitted.
- The `R44_RNG` comparison was only run on race-jump.
- `*_replays_by_the_port` was not run with `R44_CALLERS=1`.

## Next

R46: re-record the rides with the voice table per row, lay it before each between step, then turn
`R44_CALLERS` on by default and drop `RNG_LAG`.
