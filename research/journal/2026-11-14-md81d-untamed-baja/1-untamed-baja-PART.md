# 1 — untamed baja (PART)

Task MD81d. 2026-11-14.

## What I did

- Diffed SLUS-21701's car step against SLUS-21104's statically. The baja shares the buggy's class, so I
  covered its world step, the update, the class update and the input step, plus every tire, engine,
  vehicle-base and ground callee they reach. I paired functions by layout order. Each span runs to the
  next function's start, because Ghidra's sizes were short and cut off the controls' branches.
  `research/md81d/opdiff.py` is the mnemonic-only diff I wrote. Plain `sdiff` drowned in register
  renames and frame shifts.
- I found that MD81c (the monster truck, `task/MD81c`, same day, not merged) had done the same diff.
  It agrees with mine function for function. It also found the body integrate's per-time angular
  damping, which my walk had not reached.
- Ported Untamed's live differences into `crates/game-sim/src/atv_car.rs`, each behind a switch that is
  off by default:
  - `Keys::untamed` — the preload cue posts while the push is on;
  - `Vehicle::untamed` — the vehicle force adds a zero quad after the gravity;
  - `Update::untamed` → `Call::UntamedHazardClock` — answered by MD81b's `handling::untamed::stuck` with
    the grounded word;
  - `input_untamed` — the clutch is pressed when its strength is over 0.
  `Load::untamed` turns all of them on together through `Load::keys/vehicle/ride` and `Ride::pad`.
  Nothing sets it yet.
- Added `crates/game-sim/tests/untamed_car.rs`, plus one `untamed: false` line in the `Keys` literal in
  `tests/mxvatv_car_load.rs`.
- Wrote the verdicts into `research/untamed-machines.md` ("SLUS-21701's car step against SLUS-21104's
  (MD81d)") and the addresses into `research/oracle.txt # MD81d`. Both are appended at the end of their
  files, so they stay clear of MD81c's insertions.
- Filed MD162 (the capture) and MD163 (pack, shell and replay). MD160 and MD161 are taken on
  `task/MD81c`.

## What the original does

- **Preload** (`./kit dis 0x004e9210 --len 0x230` on the Untamed scratch config). The state logic is
  SLUS-21104's with the fields moved by −0x10. The cue is the difference:
  - Untamed: `if !cued && active` → cued = 1, post (8, 14). Otherwise, `timer <= 0` → cued = 0.
  - SLUS-21104 tests `!(timer <= 0)` instead of `active`.
  - With `PreloadTime` 0 (Untamed's baja has `[Preload]` off), a fired preload is active with its timer
    at 0. Untamed cues it and SLUS-21104 does not.
  - While the preload is armed, its timer winds up above 0. SLUS-21104 cues it then, and Untamed does not.
- **Vehicle force** (`0x004ba390`). It adds the gravity through body vt `+0x80`, then calls the same slot
  again with a zero quad on the stack. The add (`0x004ba450`) is a `vadd.xyz` into body `+0xa0`, then
  each listener. The zero add changes only −0 lanes (to +0) and denormal lanes.
- **Update** (`0x004e9eb0`). It calls the hazard clock `0x004ce3a0` directly, with a1 = sunk, a2 =
  grounded (any tire's `+0xa4`), f12 = dt and f13 = the longest surface time.
- **Input step** (`0x004e79c0`). It was reworked, and I agree with MD81c's reading:
  - The stick comes from the controls block's bytes `[0xdf]`/`[0xe0]`, and the strengths from that block
    too.
  - The pad word is player 0's entry for the preload command.
  - Every lookup except the gas reads an unbound command (−1) as not pressed.
  - The hand brake is the next command id and is tested `<= 0`.
  - The clutch is `s <= 0 ? 0 : 1`, where SLUS-21104's is `s == 0 ? 0 : 1`.
- **PostProcess** clamps the yaw by two compares. **The class update** has the distance clause
  `!(d <= 0)`. Both match SLUS-21104 except on inputs that never occur (a denormal limit, a −0 distance).

## How it was proved

- `crates/game-sim/tests/untamed_car.rs`, run against SLUS-21701's own functions on
  `context/untamed/ram/nat_or.bin`, with the car laid out at Untamed's offsets:
  - `untamed_car_preload_is_the_originals`: 20,000 generated cases. The event post and the body's
    add-force are hooked. It compares every preload word, the cue and the body's force by bits. Paths:
    - wrecked 1984
    - cued as SLUS-21104 1753
    - not cued as SLUS-21104 12567
    - cued where SLUS-21104 is not 52
    - not cued where SLUS-21104 is 3644
    - fired with PreloadTime 0 and cued 52

    Because both "parted" paths are covered, the old rule would fail this test by construction.
  - `untamed_vehicle_force_is_the_originals`: 20,000 cases with signed-zero and denormal lanes. The
    original's add-force runs unhooked. It compares the body force by bits, the flags byte and the
    record call. Paths: off 2518, same as SLUS-21104 12219, the zero add tells 5263.
  - `untamed_car_update_asks_the_untamed_clock`: port-only. The update asks Untamed's clock with the
    grounded word only when switched, and SLUS-21104's clock otherwise. The clock itself is MD81b's,
    proved in `tests/untamed_atv.rs`.
- `./kit check -- -p game-sim --test untamed_car`: 3 passed. The log has the tallies above, so nothing
  skipped.

## Not verified / not 1:1

- **No Untamed baja drive was captured, and none is replayed.** This is the task's own test. No Untamed
  car save state exists (copy 9's states are a Free Ride bike), and picking the baja in Untamed's menus
  by tape is a session of its own. → MD162, then MD163.
- **No pack, and no shell switch.** The cars importer (`import-atv cars`) is only on openmx-mods branch
  `openmx-MD39c`, not on its main. Nothing sets `Load::untamed`, so the Untamed step rides nowhere.
  → MD163.
- **The input step's other changes are not ported**: the controls-block stick and strengths, the pad
  word off the preload command, the unbound guards and the hand brake's id. In the port the caller
  supplies `stick`, `pad_word` and `value`. The bindings table already reads an unbound command as 0,
  but that has not been held against the original. `input_untamed` was not oracled either: it needs
  Untamed's controls block laid out. → MD163.
- **Body integrate's per-time angular damping (MD81c) is not ported.** The baja's
  `DefaultAngularRateDamping` and `ImpactAngularRateDamping` are `1, 1, 1` in SLUS-21104's file, and the
  51 changed keys do not include them, so `powf(1, y) = 1` leaves a plain drive unchanged. It does show
  once a collider or ragdoll lowers a lane below 1. Untamed's powf has not been proved against
  `ps2_float`.
- **The node world update** (MD81b's `Scene::update_untamed`) is not routed for the car. The scene's
  methods call `update` inside them, and `Scene` literals in tests stop a field being added. The quad
  has the same gap (MD144).
- **The PostProcess yaw clamp and the class update's distance clause are not ported.** They differ only
  on a denormal limit or a −0 distance. This was argued, not tested.
- **The preload oracle hooks the body's add-force.** The push's arithmetic is checked as
  `ps2_float::vu::add` per lane, not by the original's `vadd`. The vehicle-force oracle runs the
  original's add unhooked.
- **Callees not compared:**
  - node local / position `0x00195320`/`0x00194c20` (MD81c: same);
  - `0x00500820`, `0x00134940`;
  - quat-to-euler `0x001358d0` (MD93d: same clamps);
  - the sort/search functions at `0x0011xxxx` (MD81b: counts read `lhu`).
- **Untamed's `.phy` loader for the car** was not diffed by me. MD81c diffed it (same 239 keys and
  defaults) on its branch.

## Next

MD162 first: get an Untamed baja state and the drives. Then MD163.

For MD163:
- Set `atv_car::Load::untamed` from the pack.
- Build the Untamed car frame harness off `tests/mxvatv_car_frame` at the `# MD81d` offsets.
- Hold `input_untamed`, with the controls-block reads, against `0x004e79c0`.

MD161 on `task/MD81c` (the monster truck) can reuse these switches. It only needs the body damping
added.
