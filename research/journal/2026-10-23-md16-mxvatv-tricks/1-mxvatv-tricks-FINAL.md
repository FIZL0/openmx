# 1 — mxvatv tricks (FINAL)

Task MD16. 2026-10-23.

## What I did

MX vs. ATV Unleashed's tricks, laid over the original's as a setting (Settings → Ride → **Tricks**:
Original, the default, or MX vs. ATV; `--tricks original|mxvatv`; `tricks = mxvatv` in `settings.txt`,
written only when not Original). Source: openmx-mods M19 (`notes/tricks.md`) and the pack its
`tools/import-tricks` writes, read from `mods/tricks/<id>/` (`mod.json` kind `tricks`).

- **game-sim** (commit e7ea184, plus this session's fixes):
  - `trick_pack.rs` (new): reads a pack's stunt table (`[StuntNN]`, MXU's six keys plus
    `DefaultKeyMapping`, `HasAnimation`, `PointValue`, `HoldPointValue`), picks the **extras** (rows the
    rider's 24 lack, not backflip/wheelie/stoppie/360s/taunt/nothing, gesture-mapped first, at most 8),
    reads `[Three Sixty]` into `air::SpinTuning`, and lays the pack's `UnrollGain`/`UnyawGain` over the
    bike (`Phy::tricks = Some(Moves)`). `phy_name_for` maps `race125p.phy` to `mx_race125p.phy`.
    `EXTRA_CONTROLS` = report R2, L2.
  - `trick.rs`: the gesture past its 24 slots. Two extra trick buttons; Cross alone is group 7,
    Square alone group 8, with the stick's four directions → `Trick(32..39)`. New ids: `FRONTFLIP` 29,
    `THREE_SIXTY_LEFT/RIGHT` 30/31. An empty extra slot asks for nothing.
  - `air.rs`: `FRONT_FLIP` (a `Rotation` row, the backflip mirrored: arms on stick back, fires on
    forward, positive pitch torque; `FRONT_FLIP_TORQUE_SCALE` 1.0 is the port's own), the 360
    (`Spin`, `three_sixty`, ported from MX vs. ATV's step as notes/tricks.md reads it: lip test, arm/fire
    on ±0.7, torque from the flick, 0.9 s grace, decay only with no trick button and the stick against
    the spin, 35° clip state, ends on landing/contact/|yaw torque| < MinY).
  - `rider.rs`/`ride.rs`/`score.rs`: `Ride::tricks: Option<Box<trick_pack::State>>` (None on every
    Original ride), 360 landing test (clip state ≠ 0 at the end → wreck Tail), flat scores for the front
    flip (MXvATV backflip's `HoldPointValue`) and the 360s (their rows' `HoldPointValue`), the spin treated
    like a flip by the wreck test and air control, the front flip asks the body for the backflip's pose.
  - `lib.rs`: `Sim::hands` reads report R2/L2 as the extras only for a bike with a pack and not on the
    plane.
- **openmx**: `trick_packs.rs` (new: finds the first trick pack, reads `riders[0].table`, lays each bike's
  `mx_<bike>.phy`, logs what it could not use, names the clips it has none of); `main.rs` (`--tricks`,
  menu → ride, online forced Original, the pack applied to the stock `.phy` so a mid-ride modifier change
  keeps it, extras' stunts after the 24); `pad.rs` `Scheme::ride_tricks` (new layout: the pad's Cross
  and Square become the report's R2/L2, bit and pressure; original layout: R2/L2 let go, so no extras;
  tricks off = `Scheme::ride` exactly); `menu.rs`/`menu_pages.rs` the Tricks row and its help.

### How R1's inputs are told apart

- **R1 alone** (no other trick button, `Step.modifier`) is MX vs. ATV's arm condition (`+0x14ec & 0xf ==
  4`, notes/tricks.md). MX Unleashed's R1 tricks are R1 *combos* (gesture groups 3–6, R1 with another
  trick button); those never set the modifier, so every one of them runs as before
  (`md16_r1_tricks_and_the_backflip_stay_the_originals`).
- **Without R1** the stick is exactly MX Unleashed's: no new move can start
  (`md16_the_pump_without_r1_is_the_original`: THE_FLIP's preload pump on a pack bike keeps the same
  per-tick state bits as on an Original bike, no new move starts, and it is still a backflip — also on
  the pack's whip gains).
- **The backflip and the front flip read the same stick y.** The backflip needs no button (MX
  Unleashed's, kept: MXvATV's R1-gated backflip was not adopted), arms on forward and fires on back; the
  front flip arms on back and fires on forward. Whichever arms first on the lip wins — the backflip from
  a centred stick pushed forward then pulled back is a backflip with or without R1 (the original wins).
  The front flip needs R1 *and* the stick pulled back first.
- **The 360** reads the stick's x, which no MX Unleashed lip move reads, so it never collides.

## What the original does

MX vs. ATV Unleashed (SLUS_211.04), as openmx-mods notes/tricks.md (M19) traced it: the 360 step
`0x4e53d0`, the backflip `0x4e4f20`, `.phy` `[Three Sixty]` read by `0x4df740`, the whip is the
airborne control with weaker `UnrollGain`/`UnyawGain` (Pro 250: 1.9 / 2.3 against MXU's 3.5 / 3.5).
The front flip is in neither game (notes/tricks.md "Front flip: not in either game"). Recipes:
`./kit fn` reads MX Unleashed only; MX vs. ATV's functions were read in openmx-mods' own Ghidra project.

## How it was proved

- `crates/game-sim/tests/ride.rs` MD16 section, on a generated pack (MD16_TABLE / MD16_PHY: the real
  pack's numbers for Pro 125): `md16_extras_start_from_cross_and_square` (the generated pack's three free rows
  become extras, mapped first; Cross up and Cross left start the first and third and score; an empty
  Square slot starts nothing; on an Original bike Cross and Square change no hash), `md16_the_pump_without_r1_is_the_original` (above),
  `md16_r1_tricks_and_the_backflip_stay_the_originals`, `md16_front_flip_goes_round_and_lands` (pitch
  −5.5 to −7 rad, lands, FRONTFLIP node, 10000 points), `md16_three_sixty_comes_round_or_wrecks` (comes
  round and lands with R1 let go; held, it lands past 35° and wrecks Tail).
- Original's replay hashes unchanged: every existing hash/replay/acceptance test passes untouched
  (`./kit check`). Size pins moved (Sim 11328→11336, Ride 2424→2432) and the snapshot wire grew one
  Option tag a rider (RIDERS_WIRE_LEN 13323, digest updated) — the hashes did not.
- `crates/openmx/tests/menu.rs` `the_tricks_row_picks_mx_vs_atv_and_is_saved`, pad.rs
  `the_new_layout_s_face_buttons_are_the_extra_tricks`.
- Shell smoke run on the real imported pack (scratch `mods/tricks/mxvatv_tricks`, `--headless --frames
  300 --tricks mxvatv`): pack found, `mx_race125p.phy` laid, extras named, no panic; without `--tricks`
  no line and nothing read.

## Not verified / not 1:1

- **No bit-exact oracle for MX vs. ATV.** Nothing here was compared frame by frame against MX vs. ATV
  running; the 360's step is ported from notes/tricks.md's reading of its disassembly. No PCSX2 capture of
  MX vs. ATV is in this repo's tooling.
- **The flick speed the 360 fires with** (`+0x12b4` in MXvATV) is measured on the stick's x with the
  port's own flick ring (`ride::Flick`, the fore-aft one's), not traced to MXvATV's sampling.
- `ThreeSixtyTorqueRateIncrease` is read and unused (as notes/tricks.md says the step never reads it).
- The pitch decay is ported as read (`−= dt × x × RateDecrease`), unchecked against a run.
- **The 360 tumbles on MXU bikes**: pitch torque as large as the yaw (−1100 / 1100 on a hard flick)
  corkscrews MX Unleashed's 125, which has its own inertia; MXvATV's bodies were not compared.
- **The clip state without a clip**: the 35° state machine runs, but MX vs. ATV's `.mot` files (v6/v7)
  are refused by the port's clip reader, so no 360, extra or front flip clip plays. The extras with no
  clip are named in the ride log (`tricks: no clips from …`): with the real pack 9oclockindianair,
  9oclocknac, cliffhanger, rocksolid, stalefish, mcmetz, switchblade, nofooter, threesixtyL,
  threesixtyR, and the front flip.
- **The front flip's pose** is the backflip's request (`Ask::backflip`), as the brief says, until a
  clip exists; the port's body never shows a backflip-specific pose today either, so on screen it is the
  airborne lean. The front flip itself is the port's own (`FRONT_FLIP_TORQUE_SCALE` and its scoring are
  invented, said so in the Settings help).
- The 360's wreck variant `0xf` (contact past the wreck limits, `0x4e8370`) is not ported; every
  mid-clip landing is wreck Tail (11, MXvATV's `0xb`).
- How MX vs. ATV's air control and wreck test treat a running 360 was not traced: it is treated as the
  backflip is (control held off while `spin.ramp`).
- The 360 and front flip flat scores are inferred from the rows' `HoldPointValue`; the extras score at
  MX Unleashed's stunt rate, `PointValue` unused.
- `GasGyroTorque` (MXvATV's whip's new gyro term) is not applied; its two conditions were not traced.
- The key for the extras is the port's (Cross/Square under the new layout), not MXvATV's
  `DefaultKeyMapping` gestures, which use MXvATV's own buttons.
- Only `riders[0]` (the male MX rider's table) is read; MXF/ATV tables and Untamed's BPB are not.
- **R1 held through THE_FLIP's preload pump** (back, forward, back) gives a front flip, not the
  backflip: the pump's back arms the front flip first. Without R1 it is the original's.
- **Online** always rides the original's tricks until MD6's same-pack check (filed as MD19).
- A `--play` tape made with MX vs. ATV tricks must be played with `--tricks mxvatv` and the same pack;
  the tape does not record the setting.
- Old snapshot wire files differ in length (one Option byte a rider), as P97's did.

## Next

Done unless the follow-ups are taken: online agreement on a trick pack (after MD6); real clips once the
port reads MX vs. ATV's `.mot`.
