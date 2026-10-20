# 1 — camera that follows (FINAL)

Task P14. 2026-10-20.

## What I did

- **Found why the camera lost the rider.** It was not the preset. A per-frame trace of the shell's
  camera along the human's recording (`--play first_recording.p2m2 --track free09`, a throwaway
  `eprintln`, not committed) showed the **look-at point running 400 to 3000 ft up the track** — about
  speed × 40 ft — so by frame 90 the bike was off the bottom of the screen, and at frame 300 off it
  altogether. The race camera's look-ahead is `speed / speed_reference × pull² × 1.8(1 − d/600) ×
  LookAheadDistance`, with **no cap at one** (S24: the base class capped the ratio, the derived one does
  not). `Follow::new` leaves `speed_reference` at the constructor's 1.0, which S7 recorded as never
  written. **The original's cameras hold 117.3328** (80 mph in ft/s, `0x42eaaa65`) at `+0x14c`: both
  cameras (`0x0174d7b0`, `0x0174e820`) in both freeride save states on hand
  (`first_recording.p2m2_SaveState.p2s`, Free09, and `context/states/rec.eeMemory.bin`). The shell now
  sets `speed_reference` to that (`SPEED_REFERENCE` in `crates/openmx/src/main.rs`). With it the lead
  is ~20 ft at 60 ft/s and the bike stays in frame through the whole ride (`--shot` frames 90, 150,
  300, 450, 600, looked at by eye).
- **Reverse threshold.** The shell fed `Race::new` the first preset's `ForwardReverseSwitchSpeed`,
  which free06's file does not have, so it was 0. The race camera's setup (`./kit dis 0x21e270
  0x21e2c0`) hands the base initialiser `5.0` in `$f14`, the initialiser stores it at `+0x148`
  (`./kit dis 0x1f9c10 0x1f9d00`, `swc1 $f20, 328($21)`), and RAM holds 5.0 there. Now `REVERSE_SPEED`.
- **Preset by name.** `Preset::find(presets, name)` (case-insensitive, first match), and the shell's
  `--camera NAME`, default `Race Cam 1`. A name the file lacks is an error listing the names it has.
- **Which preset a freeride starts on — the brief's guess was wrong.** The brief expected
  `Freestyle Cam` for a freeride. The original's camera in the human's Free09 freeride state is on
  preset index `+0x158` = **0 of 11**, and record 0's name (`+0x30`) is **`Race Cam 1`**; the second
  camera and the other state agree. So the default stays `Race Cam 1`, now by name. All 11 live records
  were dumped from that state: Race Cam 1, Race Cam 2, Freestyle Cam, Slot Cam, Lookback, 1P Stunt
  and five attract cams. Only the first four have `CycleThruThisPreset`.
- **L1 walks like the original.** `Follow::next_preset` / `previous_preset` port the camera's vtable
  slots 0xe8 / 0xec (`./kit fn 0x1fa2e0`, `./kit fn 0x1fa3a0`): they skip presets that don't cycle
  and wrap. L1 used to step to index+1, which went through Lookback and the attract cams. Now it goes
  Race Cam 1 → 2 → Freestyle → Slot → Race Cam 1. Each switch is printed (`camera: …`).
- **HUD.** A fifth line in the right column: `camera <name>`.
- Also found the camera's `+0x1a8`, the look-back preset's index. The race camera's setup writes it:
  the first preset with `IsLookBackCam`, which is 4 on these discs. It is not used here: the shell
  has no look-back button.

## What the original does

- `camera_next_preset` (`./kit fn 0x1fa2e0`): with one preset, `set_preset(0)`. Otherwise it searches
  from current+1 upwards, wrapping to 0, for the first record whose `+0x70` is set, and switches to it
  through slot 0xfc. The search can come all the way back round to the current preset. With no
  cycling preset it never ends.
- `camera_previous_preset` (`./kit fn 0x1fa3a0`): the same downwards, wrapping to count−1. No
  one-preset case.
- The camera owner's button handler (`./kit fn 0x200330`, `param_2 < 0`) calls slot 0xe8 and plays a
  sound named by the record's name (`+0x30`). The look-back toggles (`./kit fn 0x2008b0`,
  `./kit fn 0x2006e0`) switch to `+0x1a8`, and `./kit fn 0x200820` switches back to the saved index.

## How it was proved

- `walking_the_presets_is_the_games_own` (`crates/game-sim/tests/camera.rs`): 4000 generated cameras,
  half walked forward and half backward, through the original's routines and its own `set_preset`,
  compared bit for bit over the camera, the filters and the scratch. 1047 were one-preset cameras and
  1269 walks came back round to where they started.
- `a_preset_is_found_by_name`: a hand-written five-preset file (quoted names, as the reader needs for
  spaces), `find` by exact name, by other case, by the unnamed section's name, a miss and a prefix.
  Then the walk from Freestyle Cam.
- The starting preset, the speed reference and the reverse threshold: read from save-state RAM
  (`./kit state extract …first_recording.p2m2_SaveState.p2s --addr 0x174d8e0 --len 0x30 --as f32`).
- That the camera now follows: `--shot` frames by eye along the human's tape, before (bike missing at
  90 and 300) and after (bike in frame at 90, 150, 300, 450, 600). Also on free06 with `--hud`, and
  with `--camera "freestyle cam"`.

## Not verified / not 1:1

- **Who writes `speed_reference` (117.3328) is not found.** Nothing in the camera's own code range,
  the race camera's ctor, its setup (`0x21e270`) or its frame stores to `+0x14c`. It may be a vector
  store over `+0x140..+0x14f`, or a writer through the secondary base pointer. So the value comes from
  RAM, not code, and I can't tell whether it varies by bike or mode. Both states are freerides on the
  125 class, and 80 mph also appears as a literal in several unrelated objects. A 250 or a race could
  hold something else.
- **The starting preset is from two states, both at the start of a freeride.** That shows what the
  game sits on at that point. It does not show that no mode ever switches to `Freestyle Cam` (the
  stunt mode might). Which code picks the starting index was not read: the base init sets 0 and
  nothing after it was seen changing it.
- **Which pad button is the original's camera button** was not established. The shell keeps L1, and
  the port's walk is the original's routine whichever button calls it. The button handler also plays
  a sound per preset, which the shell does not.
- **No look-back button.** The original toggles to the `IsLookBackCam` preset (`+0x1a8`) on a held
  control. The shell does not do this, and the walk never reaches Lookback because it does not cycle.
- **The two ground-probe and `Ridden` suspects from S24 are still unproven.** The heights
  `Follow::probe_ground` gets from `world.place`, and the shell's `grounded`/`speed` readings, were
  not checked against the original. The trace showed the probe's lift between 0 and ~1 ft, with no
  visible swim in the frames looked at. The camera's problem was the speed reference; these two don't
  show by eye. Proving them needs a live capture of the camera's `+0x18c` lift and the actor's two
  fields next to the port's.
- **The lags and limits feel right by eye against the tape**, but not against the keyboard's ramped
  stick: that needs a person at the keyboard, and this run is unattended. I left no number in
  `camera.rs`'s arithmetic changed.
- **`next_preset`/`previous_preset` with no cycling preset do nothing**, where the original hangs.
  The disc's files always have one.
- **The HUD clips a camera name to 19 characters.** Only `--camera` can pick the long attract names.

## Next

Nothing for this task. If a 250 or a race state turns up, read camera `+0x14c` in it to settle whether
the speed reference is a constant.
