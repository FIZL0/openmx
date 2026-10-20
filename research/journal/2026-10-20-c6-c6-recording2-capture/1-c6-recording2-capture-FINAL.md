# 1 — c6 recording2 capture (FINAL)

Task C6. 2026-10-20.

## What I did

- Played `context/recordings/recording2.p2m2` (ryha, 3,788 frames, Free09, Race125) back on the original
  in PCSX2 copy 4. I drove Tools → Input Recording → Play over AT-SPI, as S8 did, and played a
  **copy**. The worktree's recording is chmod 444 and its md5 is still 1e5091236f40915ad1e5942bce20287d
  after every run.
- Captured it with watch/acceptance.toml, which now has 20 more fields on the rider actor
  (`@game_manager -> 0x34 -> 0x34 -> 0x8 -> 0x8 -> 0x0`, the object S10's air step and S11's trick code
  are handed):
  - air step: `on_ground` +0x574, `both_wheels` +0x578, `airborne` +0x57c
  - backflip: `flip_on` +0x4fc, `flip_arm` +0x500, `flip_grace` +0x50c, `flip_torque` +0x520 (vec),
    `flip_torqued` +0x4f4, `flip_ramp` +0x4f8
  - trick: `trick_latched` +0x864, `trick_group` +0x9fc, `trick_dir` +0xa00, `trick_count` +0xa04,
    `trick_running` +0xa0c, `trick_safe` +0xa10, `trick_armed` +0xa18, `trick_elapsed` +0xa1c,
    `trick_id` (running) +0xa78, `trick_req` (requested) +0xa7c
  - `fresh` +0x750, set when the rider has just been put back on the bike
- The capture is `context/capture/acceptance/recording2-freeride.csv` (+ `.json`), next to the first
  one: 1,894 rows × 77 columns, ticks (frame_counter) 2374..4267 in steps of 1, vsync 7232..11018 in
  steps of 2, and not one empty cell. bike_type 4, bike_ctl_src 1, rider_type 2 and rider_ctl_src 1 hold
  on every row, and dt is 0.03333 on every row. md5 42810e140cef351ae7b749fd62e4681b. It is git-ignored,
  like every capture.
- The recipe is in `playback/` beside this file and in the header of watch/acceptance.toml:
  `./kit pcsx2 with -- bash research/journal/2026-10-20-c6-c6-recording2-capture/playback/play_capture.sh
  PID context/recordings/recording2.p2m2 2374 1894 OUT`. `playback/index.py` prints the index below
  from the capture.

### How the playback has to be driven (what I learned getting it to work)

1. **Play leaves the VM paused once its state is loaded.** An AT-SPI press on System → Pause at that
   point *resumes* the VM. That is why my first try started capturing at 7256. The script presses
   Pause only if the item is checked (`resume_if_paused.py`).
2. **`./kit record` cannot follow a backward jump of the counter.** So the script presses System → Reset
   first. The counters then restart near 0, the state load is a forward jump, and the `start` trigger
   (`frame_counter` within FIRST..FIRST+26 **and `rider_type == 2`**) arms on the first tick after the
   load. `KIT_FRAME_STALL=120` covers the file dialog and the load.
3. **The rider guard in `start` is required.** Without it, a VM that has been booted long enough
   counts frame_counter past 2374 in the menus and arms there. One run did exactly that: 1,894 rows of
   empty game columns.
4. **The copy has to be writable.** `cp` of a 444 file keeps the mode, PCSX2 opens the movie for
   writing, and a read-only copy never played. The script now runs `chmod u+w`.
5. **The movie ends on tick 4267**, and PCSX2 pauses the VM there. A capture asked for 1,895 rows stalls,
   so 1,894 rows is the whole playback.
6. The capture's `required = true` on rider_type had to go in the run copy: the guard fired on the
   null tick of the load before `start` had armed. I checked afterwards that no cell is empty.

## What the original does

### Where a wreck reason is written, and what a "bail" writes (for S29)

- The reason setter is the virtual at +0xfc on rider+0x48 (`./kit fn 00252ca0`; the player's override
  is `./kit fn 0027b810`). The code writes a reason in only two places:
  - **the wreck test**, `./kit fn 002522c0`, reasons 1–13:
    - 5 and 6: hard contact (> 6, or > 4 with +0x588 set). 5 is with another object. **6 is with no
      other object, which means the world.**
    - 7: joust.
    - 8: nose dive.
    - 0xb: tail.
    - 0xc: no-hander, i.e. landing a trick that is not yet safe.
    - 0xd: gated by +0x570 and a position query, `./kit fn 001a6e50` at 150.0. It throws the rider up
      100.312 with random spins.
  - `./kit fn 0027bc90`: reason 14, for a rider that is **not** a local human.
- **No code path turns a button into a wreck reason**, and the game has no "bail" string. As far as
  the decompilation shows, the original has **no pad-driven bail**. A player who wants to bail rides
  into one of the wreck test's conditions. In this recording the likeliest deliberate one is tick
  3015's nose dive: square (brake) was held from before takeoff and through it.
- The port's `air::Wreck` has 1, 2, 3, 4, 5, 7, 8, 0xb and 0xc. **It has no 6** (world contact), **no
  0xd and no 14**, and the recording hits 6.

### The index (tick = frame_counter; tape tick = tick − 2375; csv row = tick − 2374)

**Wrecks.** There are three, and none is button-driven. A wreck holds `wreck_reason` non-zero for its
whole recovery.

| Ticks | Length | Reason | State the tick before | Input | Back on the bike |
|---|---|---|---|---|---|
| 3015–3137 | 123 | **8**, nose dive | in the air since takeoff at 2990 (speed 46.4), speed 44.4, no trick, no flip | square (brake) held 2975–3051, stick y 45 | `fresh` and landed at 3138 |
| 3562–3585 | 24 | **12** (0xc), no-hander | in the air, taunt running since 3522 and **never safe**, speed 52.6; the wreck is written on the landing tick | cross | no `fresh` tick; trick_running stays 1 to 3585 |
| 4124–4202 | 79 | **6**, world contact | in the air mid-backflip 3 (flip 1/1, torque −1368.7), speed 37.8 | cross, stick y 255 | `fresh` and landed at 4203 |

The recording has no reason 5 or 7, so it never wrecks against another rider. Contact that does not
wreck cannot be seen in these columns.

**Tricks.** Every direction came from the left stick, and no d-pad was ever pressed. Group 1 is circle
and group 4 is circle + R1. Every trick was armed (`trick_armed` 1) on the tick before it started.

| Running | Id, name | Gesture | Safe at | First ground | Result |
|---|---|---|---|---|---|
| 2543–2592 | 0 taunt | g1 d1 (circle 2542–2549) | 2588 | 2595 | clean |
| 2651–2695 | 2 cancan | g1 d3 (circle 2649–2674) | 2690 | 2715 | clean |
| 3300–3385 | 1 lazyboy | g1 d2 (circle 3299–3367), during backflip 2 | 3382 | 3417 | clean |
| 3522–3585 | 0 taunt | g1 d1 (circle 3519–3530) | never | 3562 | **wreck 12** at 3562 (id 28 from 3563) |
| 3753–3810 | 15 stalelin | g4 d4 (circle+R1 3751–3787) | 3803 | 3859 | clean |
| 3927–3978 | 2 cancan | g1 d3 (circle 3926–3958) | 3973 | 3988 | clean |

`trick_req`/`trick_id` are 25 on every airborne tick without a trick (stunt_idle's lean request) and
28 on the ground. During wrecks 8 and 6 they drop to 28 and `trick_armed` to 0 while still in the air.

**Backflips.** On/arm reads 0/1 armed (stick y < −0.8 on the ground), 1/0 fired (stick y > 0.8, in the
0.6 s grace), and 1/1 turning (left the ground).

| # | Armed | Fired (torque x, grace) | Left the ground | Torque ended | Landed | Result |
|---|---|---|---|---|---|---|
| 1 | 2500 (speed 36.8) | 2506 (−1400.0, 0.583) | 2512 | 2580 in the air (−786.3) | 2595 | clean, with the taunt |
| 2 | 3284 (speed 66.5) | 3292 (−1215.5, 0.583) | 3301 | 3379 in the air (−782.3) | 3417 | clean, with the lazyboy |
| – | 3519 (speed 58.2) | not fired | takeoff 3523 cancels the arm | – | – | none; the taunt behind it wrecks |
| 3 | 4069 (speed 52.5) | 4077 (−1360.3, 0.583) | 4086 | 4124, **wreck 6** in the air (ramp still 1) | 4203 | **wrecked** |

The "lip tick" is the fire tick. The original fires on the ground and the bike leaves 6–9 ticks
later: 2506→2512, 3292→3301 and 4077→4086.

**Airborne runs ≥ 6 ticks.** Each entry gives the ticks, the length, and the speeds at takeoff and at
landing:
2511–2594 (84, 36.7→31.0), 2651–2714 (64, 50.9→42.5), 2808–2887 (80, 36.5→32.6),
2990–3137 (148, 46.4→44.3, wreck 8), 3300–3416 (117, 47.4→37.3), 3466–3482 (17, 61.3→57.9),
3522–3561 (40, 57.8→50.4, wreck 12 on landing), 3623–3630 (8, 30.3→30.8),
3753–3858 (106, 54.0→42.0), 3924–3987 (64, 52.8→43.7), 4001–4029 (29, 50.7→47.1),
4085–4202 (118, 40.6→32.9, wreck 6).

**Plain riding.** This counts ticks on the ground with no wreck, no trick, no flip on or ramping and no
`fresh` flag, in runs of ≥ 30 ticks. An armed-but-unfired flip still counts as plain riding, so
2500–2505 sits in the second run.

| Ticks | Length | Speed start→end | Max | Gas / brake / steer ticks |
|---|---|---|---|---|
| 2374–2460 | 87 | 0.1→34.1 | 34.2 | 63 / 0 / 58 (standing still 2374–2388) |
| 2462–2505 | 44 | 34.4→36.7 | 50.0 | 44 / 0 / 22 |
| 2595–2650 | 56 | 31.0→50.9 | 60.0 | 52 / 0 / 34 |
| 2715–2807 | 93 | 42.5→36.5 | 54.8 | 93 / 0 / 68 |
| 2893–2989 | 97 | 35.7→46.4 | 79.9 | 82 / 15 / 54 |
| 3139–3291 | 153 | 3.5→50.0 | 73.6 | 153 / 0 / 74 |
| 3417–3465 | 49 | 37.3→61.3 | 61.9 | 49 / 0 / 33 |
| 3483–3521 | 39 | 57.9→57.8 | 59.2 | 33 / 0 / 38 |
| 3639–3752 | 114 | 37.9→54.0 | 74.4 | 108 / 0 / 57 |
| 3859–3923 | 65 | 42.0→52.8 | 62.0 | 65 / 0 / 65 |
| 4030–4062 | 33 | 47.1→60.4 | 61.2 | 33 / 0 / 23 |
| 4204–4267 | 64 | 13.8→16.3 | 16.3 | 0 / 0 / 0: no input, coasting to the end |

Over the whole tape:
- The top speed is 79.89, at tick 2961, and the bike travels 3,630.8 units.
- 1,565 ticks have something held. Per control: cross 1,298, circle 185, R1 114, square 77, L1 5,
  R2 5.

## How it was proved

- **The input is the tape, byte for byte.** Every row's pad half matches the p2m2 field 2k−1 (k = row,
  slot 0 skipped). That is TickPhase::Second with tape tick 0 = tick 2375. It holds on 1,893 of 1,893
  rows, and every other offset fails. Row 0 (2374) is the state's own pad.
- **The capture repeats.** I ran three good playbacks:
  - Run 1 is byte-identical to the committed capture (run 2).
  - Run 4, from the final `play_capture.sh`, equals it on every cell of rows 1–1893, rng_state
    included. On row 0 only frame_counter (2375 instead of 2374) and stamp_b differ, and the rest of
    the row is identical.
- **Against the other slot's earlier capture of the same tape** (s3, base columns only), bike motion
  agrees except on 229 rows. 222 of those are its rows being one tick stale, and 7 look torn. It has 81
  moving rows that repeat the previous row, and this capture has 0. Its rng_state differs from mine on
  1,204 rows, while my three runs agree with each other on every row.
- **The trick, flip and air fields are read where the decompilation says**, and they behave the way it
  says:
  - `airborne` is the complement of `on_ground` on all 1,894 rows.
  - flip_on/arm move through 0/1 → 1/0 → 1/1 → 0/0 with stick y 0 → 255.
  - The grace starts at 0.583 (0.6 − one tick).
  - Trick groups appear exactly while circle (or circle+R1) is held.
  - The one trick that landed before its safe tick gave reason 12, and the five that landed after
    theirs did not.
- `./kit check`: the replay tests read every acceptance csv and every recording. Those are
  `the_human_recording_in_context_drives_the_port` and
  `every_acceptance_capture_is_the_original_at_a_step_the_port_can_match`, and both pass with this
  capture in place. The verdict is quoted in the commit.

## Not verified / not 1:1

- **Nobody watched the playback.** Every claim is from the columns. No screenshot was taken.
- **"No pad-driven bail" comes from reading the decompilation**, not from an oracle. I found no button
  path to the reason setter, but a path through a callback I did not follow cannot be excluded.
- **Reasons 6 and 8** were matched to the wreck test's branches by reading. Their exact conditions are
  neither ported nor proved, and the port has no reason 6 at all.
- **Row 0's frame_counter can tear.** Run 4 read 2375 on the first post-load tick, with the rest of the
  row equal to the committed one. A capture's first row can carry the next tick's counter, and the
  cause, reading during the load, is not proven.
- **The tape's last field is never consumed by a tick.** PCSX2 ends the movie and pauses at 4267, so
  the tape has one more field than the capture can show.
- **rng_state differs from the other slot's capture** on 1,204 rows even though the motion agrees. My
  own runs agree, so the cause is the other capture or its timing. That was not chased.
- **Trick names assume the default map**, where the id is the slot index. The player's trick remap was
  not read from RAM, and no display name was read either.
- **The lip window was not measured.** The flip gate's chassis +0x264 and the rider's +0x438 are not
  columns, so "lip tick" here is the fire tick.
- **Contact with other riders cannot be seen** unless it wrecks (reasons 5 and 7, which never occur).
- **The playback scripts live in this journal folder, not in the kit**, and they need the accessibility
  environment and `WAYLAND_DISPLAY` set when PCSX2 starts.
- **The capture's spec is a derived copy of watch/acceptance.toml.** It has no `state`, warmup 0, the
  `start` trigger, and no `required`, and the sidecar names that copy. The committed capture was made
  by an earlier hand copy (`start` without the rider guard). That copy is equivalent here, because the
  boot had not reached 2374 when it ran, as the 7232 vsync start shows.
- All measured on x86-64 PCSX2 2.9.127 only.

## Next

- S29: take the bail answer above. There is no button bail; reasons 6 and 0xd and 14 are missing from
  `air::Wreck`. Use wrecks 3015 (8), 3562 (12) and 4124 (6) as the fixtures.
- A replay of this capture through the port can now check trick ids, flip phases and on_ground tick by
  tick, beside the bike motion the first capture already covers.
