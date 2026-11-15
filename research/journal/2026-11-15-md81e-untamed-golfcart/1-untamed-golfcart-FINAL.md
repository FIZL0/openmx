# 1 — untamed golfcart (FINAL)

Task MD81e. 2026-11-15.

## What I did

- **Static diff, SLUS-21701's golf cart against SLUS-21104's** (research/oracle.txt `# MD81e`, research
  /md81c/fp.py and keys.py, side-by-side listings). The golf cart class's own slots are the same code
  apart from fields moved by the layout rule. The one real change is the class update: Untamed's golf
  cart uses the baja's class update, so it carries MD81d's `!(d <= 0)` distance clause (parts only on a
  `-0` distance; not ported, as for the baja). So the cart needs no new step code: `atv_car::Load::untamed`
  (MD161/MD81d's switches) and its `.phy` are all of it. No change to car.rs or vehicle.rs was needed
  (`MxvatvMachine::GolfCart` and the `golfcart` name were already there).
- **The pack.** openmx-mods branch `openmx-MD81e` (on `openmx-MD163`, pushed): import-atv's
  `untamed_cars` pack now carries the golf cart beside the baja (`res\veh\VehicleType_7.res` whole,
  `"step": "untamed"`). Its disc test checks both lines (`golfcart: 47 nodes, tires read, Weight 1000`).
  Off by default: the shell offers it only with `--mxvatv-car golfcart@untamed_cars`.
- **Captures** in my PCSX2 copy (Free Ride Rocksbury, menus' GOLF CART; kept-context/md81e via
  research/md81e/keep.py; addresses in kept-context/md81e/addresses.txt):
  - throttle (370 rows, with a hop: all four tires off around rows 276–330) and throttle2, the same tape;
  - analog: Cross pressure stepped through bytes 21..49, then full;
  - steer (full right, full left, half right) and jump (a 16 ft jump off the supercross track, rough landing).
  - research/c15/tape.py learned `down <btn> <0..1>` (a pressure byte); old tapes give byte-identical output.
- **Tests:**
  - crates/game-sim/tests/untamed_golfcart_load.rs: the cart built off the pack's `golfcart.phy.bpb` with
    `Load::untamed` equals the spawn image word for word (keys, input keys, car, body, vehicle, part words),
    and holds MaxAnalogThrottle 0.1, preload keys 0, 3 gears 2.8/2.6/2.2, final 3.8.
  - crates/openmx/tests/untamed_golfcart.rs: the shell's cart, switched to and driven on the gas over
    free09, comes to the body words a Ride made off the same pack with `Load::untamed` reaches, bit for bit.

## What the original does

- Under `DigitalThrottleSpeed` (36.67 ft/s) or sliding, the gas is Cross's strength over
  `MaxAnalogThrottle`, clamped to 1. With Untamed's 0.1, any press the pad counts is the whole gas.
  The analog capture shows it: gas 0 through pressure byte 23, then 1.0 from about byte 25. A strength of
  (25+1)/256 is already over 0.1, so there is no partial gas on this pad.
- `[Preload]` off: the preload words are 0 in the spawn image and stay 0.

## How it was proved

- A save-state capture: the spawn RAM image against the cart built from the pack (untamed_golfcart_load.rs).
- Run to run: throttle and throttle2 give the same state in every row; the 23 rows that differ are the
  recorder's late reads (one run repeats the row before, then they line up again).
- The shell against a Ride stepped in the test (untamed_golfcart.rs), bit for bit.

## Not verified / not 1:1

- **No bit-exact replay of the captured drives.** The Untamed car frame harness is MD161a's, still open;
  filed as MD169 (after MD161a). So the task's test (a captured drive replays bit-exact) is not met here.
- The class update's `!(d <= 0)` distance clause is not ported (as MD81d's baja): it parts only on `-0`.
- Partial analog gas is not seen anywhere: on PCSX2's pad the button only counts from about byte 25, which
  is already full gas. The shell's cart also did not move on bytes 0x0f or 0x16, so its press threshold was
  not tested against the original's either; the shell test drives at full pressure.
- The exact pressure byte where PCSX2/the game starts counting the press (24 or 25) is not pinned: the
  rows and tape ticks are about a tick apart.
- The "every replay hash unchanged" check rests on no shipped code changing: only new tests and research
  files here; the gate's own replays are the check.
- The pack sits on unmerged openmx-mods branches (openmx-MD163 → openmx-MD81e).
- The golf cart's cue/sound, camera presets and look were not compared.
- The jump drive's landing (rough, gear changes in the air) is captured but not read closely.

## Next

- MD169 once MD161a lands: replay these captures.
