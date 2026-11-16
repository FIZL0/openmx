# 1 — untamed sandrail (FINAL)

Task MD81f. 2026-11-16.

## What I did

- **Which class.** SLUS-21104's MD39a sandrail image (`context/mxvatv-ps2/md39a/sandrail.ee.bin`, object
  0x01b158a0) holds the buggy vtable 0x003cec60 with kind 8. SLUS-21701's spawn image holds the baja's
  vtable 0x003f09d0 (class update 0x004e46c0 at +0x10c), kind 8. The sandrail is the buggy/baja class in
  both games, so MD81d's static diff of that class (research/untamed-machines.md, `# MD81d`) is the
  sandrail's step. Nothing new to compare and nothing new to port: `atv_car::Load::untamed` plus
  Untamed's sandrail.phy is all of it. No change to car.rs or vehicle.rs was needed
  (`MxvatvMachine::Sandrail` and the `sandrail` name were already there).
- **The pack.** openmx-mods branch `openmx-MD81f` (on `openmx-MD81e`, pushed): import-atv's
  `untamed_cars` pack now carries the sandrail beside the baja and the golf cart
  (`res\veh\VehicleType_8.res` whole, `"step": "untamed"`). Its disc test checks the new line
  (`sandrail: 43 nodes, tires read, Weight 2300`). Off by default: the shell offers it only with
  `--mxvatv-car sandrail@untamed_cars`. Installed into this checkout's `mods/machines/untamed_cars`.
- **Captures** in my PCSX2 copy 2 (scratch KIT_CONFIG = MD81e's kept slus21701-kit.toml; started with
  the two Qt accessibility variables, or the AT-SPI scripts find no app):
  - Menus by tape from MD81e's vehicle-golf.p2s: research/md81f/menu1.pad (right → SAND RAIL, checked by
    screenshot), menu2.pad (Start Event → spawn.p2s on Rocksbury).
  - research/md81f/play.sh + sandrail.toml (MD164/MD81e's, at the baja vtable), four drives, each 2
    vsyncs a row with no gaps: throttle and throttle-2 (370 rows: 3rd gear, over the whoops, a jump at
    rows ~200–260, a rough landing), steer (280: right, left, half right; hits a wall at row ~155 and
    sits on its rear tires), jump (370: the supercross track, in the air rows ~130–170, 4th gear, a
    hard stop).
  - Kept in kept-context/md81f (research/md81f/keep.py): states, spawn.ee.bin, the pack's sandrail
    files, captures, tapes, addresses.txt, the scratch kit.toml.
- **Tests:**
  - crates/game-sim/tests/untamed_sandrail_load.rs: the sandrail built off the pack's
    `sandrail.phy.bpb` with `Load::untamed` equals the spawn image word for word (keys, input keys, car,
    body, vehicle, part words), and holds the engine (5 gears 6/2.8/2.1/1.6/1.2, final 4.5), CofG
    (0, 1, 1), CofGTravelBack 0, no preload, and the file's retuned keys (Weight 2300, no
    CofGTravelBack key, front Spring 0.5, rear MaxDecompressionRate 7.5 / Spring 0.35 /
    ReboundDampingRatio 0.5).
  - crates/openmx/tests/untamed_sandrail.rs: the shell's sandrail, switched to and driven on the gas
    over free09, comes to the body words a Ride made off the same pack with `Load::untamed` reaches,
    bit for bit.
  - Both checked to really run (a deliberately wrong expectation failed each, then put back).
- Nothing taken from opentennis: no shared code is involved.

## What the original does

- The sandrail is constructed as the buggy/baja class (the PC exe's factory says the same: kinds 4/6/8
  share one constructor). Its step is the baja's.
- Untamed's sandrail.phy drops `CofGTravelBack`; the loader's default is 0, which is what Unleashed's
  file set, so the travel words do not change. The image's word is 0.
- `[Preload]` off: the preload words are 0 in the spawn image.

## How it was proved

- A save-state capture: the spawn RAM image against the sandrail built from the pack
  (untamed_sandrail_load.rs).
- Run to run: throttle and throttle-2 give the same state on every row but one (row 27), and that row
  equals the other run's next row: the recorder's late read (research/md81f/rows.py).
- The shell against a Ride stepped in the test (untamed_sandrail.rs), bit for bit.

## Not verified / not 1:1

- **No bit-exact replay of the captured drives.** The Untamed car frame harness is MD161a's, still open;
  filed as MD175 (wait for MD161a). So the task's test (a captured drive replays bit-exact) is not met here.
- **No new code diff was run for the sandrail itself.** Its class is the baja's in both images, so MD81d's
  diff stands for it. Whether any shared function branches on kind 8 was not searched; the port's car code
  has no sandrail-only branch (car.rs maps it to the buggy kind).
- The class update's `!(d <= 0)` distance clause is not ported (as MD81d's baja): it parts only on `-0`.
- The "every replay hash unchanged" check rests on no shipped code changing: only new tests and research
  files here; the gate's own replays are the check.
- The pack sits on unmerged openmx-mods branches (openmx-MD163 → openmx-MD81e → openmx-MD81f).
- The sandrail's look, sounds and cameras were not compared (MD157's).
- The steer drive's wall hit and the jump's landing are captured but not read closely.

## Next

- MD175 once MD161a lands: replay these captures.
