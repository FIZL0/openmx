# 1 — untamed monster pack (FINAL)

Task MD161. 2026-11-14.

## What I did

- Ported the two of MD81c's four live changes that MD81d had not, behind `atv_car::Load::untamed`, which is off as read:
  - **Body integrate, per-time angular damping:** `atv_body::per_time_damping` and `Body::integrate_untamed`. `Body::integrate` is now `integrate_on(h, false)`, so the old path runs the same code.
  - **Car PostProcess yaw hold by two compares:** `atv_car::hold_yaw_rate_untamed`, `post_process_untamed`.
  - Both reach the ride through `atv_car::frame_untamed`. The `Frame` gains `untamed`, and `Ride::frame_on` picks the frame by `update.untamed`, which `Load::untamed` already set.
- The other two were already in:
  - The preload cue while firing is `Keys::untamed` (MD81d).
  - The input step's clutch `<= 0` is `input_untamed` (MD81d).
  - The unbound-command guards need no code. The shell hands the port per-command strengths, and an unbound command's strength is 0, so it reads as not pressed.
  - The hand brake's id change (0x18 → 0x19) is only Untamed's command numbering. The port's `HAND_BRAKE` is tested `<= 0` in both.
- **Denormals in the compares.** The yaw hold oracle found that the original keeps a denormal rate under a denormal limit, because the FPU reads both as 0. `ps2_float::F` derives a plain IEEE `PartialOrd`, so its compares do not flush denormals, even though its doc says they do. `hold_yaw_rate_untamed` and `per_time_damping` therefore read their operands through `F::from_bits`, and a denormal `h` takes the `h <= 0` path, as the original's does. The integrate oracle now generates a denormal `h` too. I filed the mismatch in `F`'s doc as a follow-up and did not change ps2-float.
- Shell switch:
  - `machine_packs::CarFiles` gained `step`; `"step": "untamed"` makes `CarFiles::untamed()` true.
  - `vehicle::MxvatvCarRide::read` reads a `.bpb` phy through `game_sim::bpb::ini`, since Untamed's `.phy` files are BPBs.
  - `MxvatvCarRide::on_untamed_step` sets `Load::untamed` with `Arc::make_mut`.
  - `ride_mxvatv_car` takes `--mxvatv-car CAR@PACK`. A bare word now prefers a pack listing the car on Unleashed's step, so an installed Untamed pack never takes over `--mxvatv-car baja`.
- Split out:
  - MD161a: the replay of MD160's drives.
  - MD161b: the openmx-mods pack.
  - MD161c: the bike's damping.
  - The ATV's routing went to MD144 as a note.

## What the original does

- **Body integrate** (`./kit dis 0x004bc670` on the Untamed scratch config):
  - With `h > 0`, `k = 0.333f / h`. If `abs(k − 1) > 0.1`, each of the three damping lanes becomes `powf(d, k)`; otherwise the lanes are kept as they are.
  - Then the rates become `d·rates + h·accel`, with SLUS-21104's per-axis clamps.
  - On the raised path the damping quad is built in a stack slot whose `w` is never written, and the vector multiply carries that stack word into the rates' `w`.
  - With `h <= 0` the damping is a rodata vector of ones.
- **PostProcess yaw hold** (`./kit dis 0x004e9790`):
  - `r > L → L`, `r < −L → −L`, compared as written.
  - SLUS-21104's form is `!(abs(r) <= L) → L·sign(r)`.

## How it was proved

All by function oracle over `context/untamed/ram/nat_or.bin`, in `crates/game-sim/tests/untamed_truck.rs`:

- `untamed_body_integrate_is_the_originals`:
  - Runs 20,000 generated bodies through the original's integrate and end-of-substep. It compares the whole 0x230-byte body bit for bit against `integrate_untamed` + `end_substep`.
  - Damping lanes: 1, 0.999, 0.99..1, 0, −0, denormal, above 1, negative.
  - `h`: 1/120, 1/115, 1/60, 1/240, inside the near-1 band (0.3027..0.37) and at its edges, 0, −0, negative, up to 2.
  - The test tallies the raised path, the near-1 path and the `h <= 0` path, each with at least 30 cases. It also counts the cases where SLUS-21104's integrate gives a different body.
  - This also proves Untamed's powf against `ps2_float::libm::powf` on the inputs the integrate hands it.
- `untamed_wreck_damping_falls_as_captured`: at the stored 0.999 damping, 4 substeps give ×0.852214 a frame (0.999^(4·0.333·120)). That is within 3e-5 of the ×0.85219 read off MD160's wreck rows. The old integrate gives ×0.996.
- `untamed_yaw_hold_is_the_originals`:
  - Runs 20,000 limits and rates (zeros, negatives, denormals) through the original's PostProcess as a stretch: the vehicle post hooked to return, the shift hooked to end the call.
  - Compares the rates bit for bit.
  - At least 100 of the cases part from SLUS-21104's hold.
- `crates/openmx/tests/machine_packs.rs` `a_cars_step_word_picks_untameds_car_step`: the manifest word parses.
- Off by default: every flag is false unless a pack's `step` says otherwise, and `integrate`/`post_process`/`frame` run the code they ran before. The gate's existing replays are the hash check.

## Not verified / not 1:1

- **The rates' `w` lane on the raised damping path.** The original's comes from a stack word its integrate never writes, whatever the caller left there. The port carries the damping's own `w`. The oracle plants a word and checks that the original copies it, so the lane is known but not reproduced. Nothing reads the rates' `w`.
- **MD160's drives are not replayed.** The task's test, "a captured monster-truck drive replays bit-exact", is not met. It needs an Untamed car frame harness at Untamed's offsets, seeded from MD160's images and driven by the tapes' input on Rocksbury's terrain. That is MD161a, shared with MD163's baja.
- **No openmx-mods pack.** The importer for cars is on the unmerged openmx-MD39c branch, and nothing builds a pack from Untamed's `monstertruck.phy.bpb` yet (MD161b). The shell switch is only proved as far as the manifest parse. No ride on a real Untamed pack was run, and the `.bpb` phy read was not exercised on Untamed's file.
- **Bike and ATV rides do not take the damping yet.** Routing it is MD161c (bike) and MD144 (ATV).
- **The input step's guards, controls-block reads and pad word are not oracled.** They are argued away above (strengths, not pad bits) and left to MD163's harness.
- **The yaw hold was proved only as a stretch.** The vehicle post was stubbed, so the PostProcess's other effects were not compared by this oracle. MD81c found them the same code, fields moved.
- **Untamed's powf** was proved only through the integrate oracle's inputs, not by a standalone oracle over its whole domain.

## Next

MD161a: the Untamed car frame harness and the replay of MD160's drives. Then MD161b (the pack), MD161c (the bike's damping) and MD161d (`F`'s compares and denormals).
