# 1 — the wreck clock and put-back (FINAL)

Task MD71. 2026-11-02.

## What I did

- Read the tail of `atv_bike_wreck_test` (`./kit dis 4e2c68 --len 0x170 --program atv`) and the
  bike's unwreck it calls through vt `+0x36c` (`./kit dis 4e2380 --len 0x120 --program atv`, whose base
  half is `./kit dis 4a8500 --len 0xe0 --program atv`).
- Ported it as `game_sim::handling::step::machine`, which works on the same `state::Ride` as MX
  Unleashed's `Ride::step`. It returns the same `state::Step` and also whether this was the arming frame,
  where the bike posts event (8, 12).
- `rider.rs`: an MX vs. ATV ride (`ride.mxvatv` is set) takes its verdict from `machine`, and so does
  the ragdoll's copy of the machine that asks "does this frame put back". The wreck-test closure is
  unchanged. It was only moved out of the call so both machines can take it.

## What the original does

Driven, after the test, with the reason re-read (`+0xc80`):

1. Reason 0: nothing.
2. `+0xc84` clock += dt, stored, **first** (`add.s` then `swc1`).
3. Not counting (`+0xc8c`) and settled (`+0x920`): the window is 2.5 for reason 13, else 0.5. If
   `c.ole clock, window` is false (the clock is past it), then counting = 1, clock = 0, and event (8, 12)
   is posted on the bike's source (`+0x674`). The arming frame's `dt` is spent.
4. Counting and `c.ole clock, +0xc88` false: vt `+0x36c` (bike, 1). There is no reset request, no page
   question and no owner test, so a rider no player owns is put back too. There is no cannonball limit.
   The unwreck stubs the trick, sets the stunt ids to 255, clears the flip torque and the throw vector,
   places the bike (vt `+0x3a8` with kind 1), resets the engine and sets the race record's riding bit
   (`0x40000` at `*(+0x680)+8`). Then its base half runs: a cannonball's gravity is divided back (MD63f),
   and the reason **and the clock** are zeroed, with the latch left alone. It also posts (8, 13).
5. Not driven: MX Unleashed's reason 14, the same as before (MD63c's proof).
6. Both paths: if the reason is set, the riding bit is cleared. A reason that went from 0 to set
   counts `+0x6c4`.

Against `state.rs` (MX Unleashed):

- MX Unleashed tests the window *before* adding. MX vs. ATV adds first, so it arms one frame earlier, and
  a clock that crosses the window on its add arms on that same frame.
- MX Unleashed's cannonball waits for `CannonballWreckTime`. MX vs. ATV's waits for MaxWreckTime.
- MX Unleashed pages every reason but 14, and does not put back a rider with no owner. MX vs. ATV
  unwrecks at once.
- MX Unleashed's respawn leaves the clock alone. MX vs. ATV's zeroes it.

## How it was proved

- `tests/mxvatv_step.rs` `mxvatv_wreck_clock_is_the_originals`: 3,000 runs of 1 to 120 frames (about
  95,000 frames) through the real `atv_bike_wreck_test` and the real unwreck with its base half. Only
  the unwreck's calls out (vt `+0x4a8`, `+0x4b0`, `+0x490`, the placement, the engine reset) are
  stubbed, and they are logged. The wreck entry is hooked as in MD63c. Each run starts from a
  generated reason, clock (sometimes exactly 0.5, 2.5 or the limit), latch, limit (sometimes −0.1 or 0)
  and wreck count. Each frame gets a generated `dt` (1/60, 1/30 or random), a driver, both wrecks-on
  words, the settle flag and an out-of-bounds answer (the one clause the harness lets wreck, 13). On
  every frame these must be the port's: the reason, the clock's bits, the latch, the wreck count, the
  riding bit (and the rest of the race record untouched), the entries, the put-back with its
  placement kind, and the events (8, 12) and (8, 13). The original may write only the bike, its body,
  the race record and the stack. All 13 paths are taken ≥ 124 times (armed and put back on the same
  frame, the cannonball armed and put back, no control put back, re-wrecked while down, and so on).
  MX Unleashed's `Ride::step` answers differently on 3,326 frames.
- Mutations, each of which reds the oracle: no add before the window; the clock kept on the put-back;
  the window tested before the add; a cannonball limit of its own; no arming post.
- `tests/mxvatv_ride.rs` `an_unowned_mxvatv_rider_is_put_back_and_the_originals_is_not`: the wiring.
  A rider nobody owns, put down for no control, is put back (clock 0, latch on) on its first driven
  frame on MX vs. ATV, and stays down on the original's handling.
- `./kit check -- -p game-sim --test mxvatv_step --test mxvatv_ride --test mxvatv_air --test
  mxvatv_mass --test state --test ride --test air_wrecks --test hash --test snapshot --test
  wreck_thresholds --test rider_contact`: 139 passed. `cargo build -p openmx` builds.

## Not verified / not 1:1

- No MX vs. ATV ride capture, so no whole wreck-to-put-back is replayed against the game.
- The events (8, 12) on arming and (8, 13) on the put-back are reported (`machine`'s second value),
  but nothing in the port listens to them. The entry's (8, 11) is not ported either.
- The unwreck's own writes besides the machine's are left to `rider.rs`'s existing put-back, which is
  MX Unleashed's placement, unwreck and trick reset. That covers the stunt ids set to 255, the flip
  torque, the throw vector, `+0x13e4`, `+0x1650`, `+0x119c/+0x11a0`, `+0x1784` and the engine reset.
  None of them were compared against the port's. The cannonball's gravity divided back is MD63f's.
- MX vs. ATV keeps no stored put-back kind that I found (kind 1 is passed straight to the placement),
  so `machine` leaves `Ride::kind` alone. `Ride::fresh` and `Ride::wrecking` are set and cleared as MX
  Unleashed's respawn does. MX vs. ATV's copies of them (perhaps `+0x13e4` for fresh) were not identified.
- The pad chord and the wreck page's own put-back (`rider.rs`) are MX Unleashed's on MX vs. ATV rides
  too. SLUS-21104's chord was not read.
- The oracle never lets the wreck test's chain run (the bike is zeroed, so the out-of-bounds is the
  only entry on a riding bike). The chain and the hits are MD63c's proof, and `machine` takes them as
  the same closure.

## Next

Nothing for this task.
