# 1 — pc per frame virtuals (FINAL)

Task R11. 2026-10-28.

## What I did

- Paired MX vs. ATV PC's per-frame bike virtuals with the SLUS-20767 boot by a call-graph walk out from
  R10's sure `pc_` rows: 31 new `pc_` rows in `research/oracle.txt` (`# R11`), and the section "The bike's
  per-frame virtuals (R11)" in `research/mxvatv-pc.md`, which replaces its "Not paired" paragraph.
- Method per row: a callee is a twin if it is called at the same place, in the same order, by a function
  already paired, and its float constants and branch shape agree (`./kit fn <addr> -o pc` beside
  `./kit fn <name>`). The rows say which evidence they rest on, and the weak ones say so.
- Extracted MX vs. ATV PS2's boot ELF (SLUS_211.04, 2,965,120 bytes, no symbols) and its overlays
  GAME_REL.BIN / UI_REL.BIN off `~/repos/openmx-mods/iso/MX vs. ATV Unleashed (USA).iso` into
  `context/mxvatv-ps2/` with `ps2kit.disc`. The first analysis of SLUS_211.04 alone had no bike code: the
  game is in GAME_REL.BIN. New `tools/mxvatv_overlay.py` folds it into the ELF (`atv_game.elf`), which was
  analysed as `-o atv` and laid beside both (below): 39 `atv_` rows after the pc block. `kit.toml.example` has the
  `[other.atv]` block. The slot's kit.toml is not read here (KIT_CONFIG points at the main checkout's), so
  the analysis ran with a scratch KIT_CONFIG: the main kit.toml plus `[other.atv]`.

## What the original does

The PC runs the bike's frame over five entry points, not one:

- `./kit fn 0x005fab90 -o pc` (`MXVehicleMotoShared::vf14_at_12`) = `bike_vehicle_update`: the ghost step
  (vt +0xd8) first, then for local kinds the wreck test vf90(dt, 0) and the stunt idle vf103; kinds 3/4
  (remote) clear the inputs and run the chassis update, vf104, vf101, vf91 — the PS2's `+0x80c == 2`
  branch (0x00259150, `stunt_clocks(…, 1)`, +0xf0).
- `./kit fn 0x005f6900 -o pc` (PreProcess, `vf0_at_48`) = the ground step's pre-loop: the input limiter
  (vf36, 0x005ff8b0), front steer at +0x8ac, the rear fade with ±π/2, the 14.6666/29.3333 lerps, the
  attitude peak vector cleared, the auto shift 0x005fb490 (7.3333, 9.549296 = 60/2π, calls
  `pc_bike_set_gear`) and the clutch 0x00601180.
- `./kit fn 0x005f9950 -o pc` (Force, `vf0_at_60`) = the substep body, in the PS2's call order: stand
  upright, balance, roll/pitch, preload, backflip, 0x00600790, air drag, airborne control, the pitch torque,
  wheel contacts, the peak hold. The physics world substeps, so there is no 55 Hz loop here.
- `./kit fn 0x005f9ab0 -o pc` (PostProcess, `vf0_at_52`) = `bike_air_state` (the wheel-down flags, airtime
  +0x117c, the 10.0 heading slip) plus the ground step's 0.25 tail, the no-hander once a frame, and an
  out-of-world wreck (`PhysicsObject::vf15 == 0` → event (8, 0xd), vt +0xdc(1, 1)).
- Wrecks: vf90 0x005fe9c0 = `bike_wreck_test`, the same reasons through vt +0xac (13, 4, 1, 2, 3, 12, 7,
  11, 8, 5, 6) plus 14 (no rider/state) and 15 (0x005fbb40). vf43 0x005fe7c0 = `bike_wreck_entry` with
  `player_wreck_entry` merged (three `rand()/32768` draws for reason 13, the shown flag +0x14b4, the
  count +0x12f8 += 1). vf44 0x005fe8a0 = `bike_unwreck` + `player_respawn` (peak vector cleared, action ids
  0xff, fresh +0x1404 = 1, race bit 0x40000 back, vt +0xdc(0, kind)).
- Ghost: vf53 on (alpha 0.5, DYNAMICOBJECT/PHYSICSBODY/PHYSICSOBJECT masked, state 2), vf52 off (alpha
  1.0), vf54 the step (counts down while +0x126 bit 0x20 is clear, alpha lerp 0.1 → 1, vf52 at 0);
  `MXVehicle::vf55` 0x005d1330 is the put-back that picks one by kind (PS2 0x0027b650 via vt +0x138).
- Stunts: vf100 0x00600180 = `stunt_request` (overlap take-over, lead capped at 0.3 s), vf103 0x005fb370 =
  `stunt_idle`, vf101 0x006000e0 = the clocks, Motocross::vf47 0x005f6f30 = the network snapshot.
- Wheel: 0x004726d0 = `wheel_integrate` (surface drag, spin, then steer integrate or the inlined steer
  stop: +0x8b0 = 0, +0x60 = 0); 0x0046fe60 = `wheel_steer_integrate`, fields 0xC lower.

## SLUS-21104

- GAME_REL.BIN is a CodeWarrior "MWo3" image, header {magic, version, load 0x0045c580, text 0xc8ac0, data
  0x7d00, bss 0x16080, …}. A first fold that dropped the 64-byte header put every function 0x40 low: 2198
  of the main ELF's 6922 vtable words into the slot were start + 0x40. Loaded whole, they all land on
  starts, and header + text + data + bss = 0xe6880 is exactly the second empty LOAD segment's size.
- The bike: primary vptr at +4 (table 0x003d0450), listener vptrs +0x30..+0x40 whose tables hold one thunk
  each: Pre 0x004de0c0 → 0x004d8b20 (−48), Post → 0x004d9290 (−52), Collision → 0x004ddf10 (−56),
  Force → 0x004d90f0 (−60), and the slot-14 thunk 0x004de0a0 → 0x004da210 (−12), which calls
  0x004e1b90 (`bike_vehicle_update`) first, as PC 0x005f7380 calls 0x005fab90.
- The Force body 0x004d90f0 makes the PC's calls in the PC's order, the 360 function 0x004e53d0 included,
  so that one is MX vs. ATV's, not the PC port's. The update's vt calls (+0x3a4 ghost step, +0x49c wreck
  test (dt, 0), +0x4f0 stunt idle; remote: +0x4f8, +0x4e8, +0x4a0) place the slots; the rest came from a
  Needleman-Wunsch of the PC's MXVehicleMotocross table against 0x003d0788 by offsets (shifted −0x20),
  floats and size.
- `./kit fn 0x004e6a60 -o atv` (clutch) ends writing the lerp to the engine's +0x7c and has no chassis
  +0x198 term: the PC's "missing" term is MX vs. ATV's change.
- `./kit fn 0x004e2260 -o atv` (wreck entry) draws `rand·4.656613e-10` (2⁻³¹) as MX Unleashed does; the
  PC's /32768 is its libc. The unwreck uses ids 0xff and race bit 0x40000 as the PC does.
- `./kit fn 0x004a2cb0 -o atv` (steer integrate): the PC's fields, MX Unleashed's clamp order, and
  `madd.s`/`msub.s` (Ghidra's `(x + 0.0) − y·z`) where MX Unleashed has separate mul/sub.

## How it was proved

It was not, and cannot be: the exe is a reference, never an oracle (`research/mxvatv-pc.md`). Each pair
rests on call position plus shared float constants/branch shape, read in Ghidra on both sides. No port code
changed, so no replay hash moves; `./kit check --all` is the gate.

## Not verified / not 1:1

- Weak pairs: `pc_stunt_clocks` (145 bytes against the PS2's 2716 — only the elapsed/progress clocks),
  `pc_stunt_state_pack` (by purpose: both pack the network state), `pc_bike_put_back` (vf55 ↔ 0x0027b650,
  by role), and PS2 0x00252270 ↔ PC vf91 0x005fad80 (not written as a row).
- PC 0x00600790 (35°, ±0.7, 5, 30) has no PS2 twin; that it is MX vs. ATV's 360 is a guess.
- Not paired: PS2 0x00259150 (state unpack; PC vf104 0x005f7b40 places the wheels instead), 0x00252280,
  0x00252fe0; the helicopter models (out of R11's list).
- `bike_clutch`: the PS2's last term (chassis +0x198 less +0x6cc·lerp(+0x6a0)) is absent on the PC and
  in SLUS-21104 — read as MX vs. ATV's change; where its effect went was not traced.
- SLUS-21104 rows: placed by call site and alignment score; only the clutch, wreck entry, unwreck, ghost
  slots, steer integrate and the three listener bodies were read side by side. `atv_stunt_clocks` and
  `atv_stunt_state_pack` (0.37) are weak as their PC rows are; `atv_bike_wreck_test` was not compared
  clause by clause (so the 2.9/4.5 retune is not confirmed in SLUS-21104's numbers).
- `atv` vf91 0x004e2de0 / vf102 0x004e4810 / vf104 0x004dab50 were seen but not written as rows.
- No `./kit ghidra name -o atv` either: the rows are in oracle.txt only. The atv analysis needs the
  scratch KIT_CONFIG (or a kit.toml with `[other.atv]`) and the folded ELF in context/.
- `bike_input_limiter`: the PS2's stick squaring was not found on the PC side (maybe `MXVehicle::vf36`).
- `bike_wreck_test`: PS2 constants 0.4, 4, 6, 100.3, 1497.6 are not in the PC's; its 2.9/4.5 are new. Read
  as MX vs. ATV's retune, not checked against SLUS-21104's numbers clause by clause.
- `wheel_steer_integrate`: the clamp order differs (PC `max < a`, then `a < min`; PS2 `a <= max`, then
  `a < min`) — equal but for NaN — and the multiply order differs, so x87 rounding aside it would not be
  bit-identical even on one CPU.
- Ids renumbered (stunt 25/28 → 0x2e/0xff, action 0x1c → 0xff, race bit 0x80000 → 0x40000): read from the
  code, the tables mapping them were not compared.
- `./kit ghidra name -o pc` was not run: s2 has no `context/mxvatv-pc` and no `pc.gpr`, so the new rows
  are not yet names in a Ghidra project. Run it where the exe is.

## Next

- `./kit ghidra name -o pc` / `-o atv` where the exe and `atv_game.elf` are, to carry the rows into the
  projects (filed as a plan follow-up).
- Compare `bike_wreck_test` clause by clause across the three, if a wreck threshold in the port is ever in
  doubt.
