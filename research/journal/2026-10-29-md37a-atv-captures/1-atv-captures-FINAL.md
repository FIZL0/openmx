# 1 — atv captures (FINAL)

Task MD37a. 2026-10-29.

## What I did

- Made SLUS-21104 save states with an ATV in Desert Oasis Free Ride, two quads × two states, and recorded
  15 short rides at every tick (30 Hz) with `./kit record`: the chassis, the four wheels, the engine and
  the pad. Each ride aims at one of research/mxvatv-pc.md's ATV branches.
- research/md37a/ (git-tracked):
  - atv.toml: the watch spec. "ATV + " is replaced by the object's address.
  - record.sh: one capture.
  - throw.py: a mid-air state.
  - summary.py: decoded rows.
  - *.pad: the rides.
- Wrote the field map and `atv_phy_path` into research/oracle.txt (`# MD37a`).

### Recipe

1. **Config.** A scratch kit.toml is this slot's, with `[game]` serial `SLUS-21104`, iso
   `~/repos/openmx-mods/iso/MX vs. ATV Unleashed (USA).iso`, elf `SLUS_211.04` and vsync_addr `0x3fa6d8`.
   `export KIT_CONFIG=<it>` for every kit command; memory: mxvatv-runs-in-your-own-copy.
   - Copy 2 was started with
     `WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 ./kit pcsx2 start`.
2. **Menus.** Load M21's state 04 (a Free Ride bike) → Start → up from Resume to Return to Main Menu → Yes.
   Then Single Player → Free Ride (3rd) → Track Selection.
   - Desert Oasis is 8 downs; the locked tracks are not skipped.
   - Player 1 Options: Vehicle right → ATV. Physics type is greyed RACE. Start Event.
   - Menu presses: one `./kit pad send "down X" "sleep 200" "up X"` per call, 1–2 s apart, checked by
     `./kit shot`. Faster ones are dropped, and so are presses sent during a load.
   - `./kit shot` fails while a pad script still plays; retry it.
3. **The atv_ (Race) quad**, model ATV_R_450F_250RTHQ1 (the only ATV this memory card has).
   - Saved at the spawn: copy 2 slot 1 → `context/mxvatv-ps2/md37a/atv-r-spawn.p2s`.
   - Player ATV at 0x15e0690.
4. **The atv_s_ quad.** The four ATV_S_ models (KFX4001/2, LTZ4001/2) cannot be picked.
   - Customize Vehicle's Model cycles nothing. The profile's model strings (0x913c1c, 0x914134) can be
     poked, but Start Event rewrites them from the menu index.
   - So: `./kit fn 0x218ae0 -o atv` (`atv_phy_path`) picks the `atv_s_` prefix by
     `strstr(model, "ATV_S_")` at 0x39f4e1. Only it reads that string (`./kit xref 0x39f4e1 -o atv`).
   - `./kit pine write 0x39f4e5 0x52 --as u8` turns the needle into "ATV_R_". Then Return to Main Menu and
     go back into Desert Oasis as in step 2.
   - The new quad's engine +0x34 (final drive) reads 10.538 against the atv_ quad's 4.558. Its wheels'
     tire table +0x778..+0x7b4 reads 325 against 150. Both are the `atv_s_` values (MD37's .phy diff), so
     it rides the atv_s_ files with the R quad's look.
   - Saved at the spawn: slot 3 → `atv-s-spawn.p2s`. Player ATV at 0x15e0b90.
   - The poke is inside the state's RAM, so loading it keeps the patch (the `.phy` is already read anyway).
5. **Mid-air states.**
   - `./kit pcsx2 with -- python3 research/md37a/throw.py ATV SPAWN_SLOT 45 30 OUT`: gas to 45 ft/s, then
     +30 ft/s on the body's velocity y (body +0x84), then save the next frame.
   - Race: slot 2 → `atv-r-air.p2s`. atv_s_: slot 4 → `atv-s-air.p2s`.
   - Slot 4 had held my copy of M21's state 04; the original is untouched in its own context.
6. **Captures.** `research/md37a/record.sh ATV SLOT NAME PAD FRAMES` →
   `context/capture/md37a/NAME.csv` (+ .json). Read them with `python3 research/md37a/summary.py NAME [EVERY]`.
7. **RAM images.** The four states' `eeMemory.bin` are `context/mxvatv-ps2/md37a/atv-{r,s}-{spawn,air}.ee.bin`.
   The members are zstd (zip method 93), so `unzip` can't read them; Python's `zipfile` can.
   `atv-{r,s}-spawn-late.ee.bin` are live dumps taken a few frames after the spawn saves.

### Captures (context/capture/md37a, all 30 Hz rows: vsync +2)

| csv | state | pad | what happens (vsync) |
|---|---|---|---|
| race-steer | r-spawn | steer | gas to 72 ft/s, full left 35779.., centre, full right 35907.., held through the turns, no wreck; one 0111 hop |
| race-steer-slow | r-spawn | steer-slow | right then left lock coasting from 27 ft/s down to a stop; one step-4 gap |
| race-stoppie-short | r-spawn | stoppie-short | Square + stick back at 47 ft/s from 35781: touch 1100, rear +0xb4 = 0011, pitch to −1.11, brake reads 0.25, rides out to a stop ~35879, no wreck |
| race-wheelie-launch | r-spawn | wheelie-launch | stick forward + gas from rest: fronts leave several times (pitch ~0.15), then the palm at 79 ft/s, wreck 35793 |
| race-jump | r-air | jump | air pitch forward then back, lands 1111 at 35817, the palm ~35847 (wreck), put back ~35989 |
| race-jump-side | r-air | jump-side | air yaw −1.83 → −1.25 and back, clean landing 35815; one step-10 gap |
| race-still | r-spawn | still | gas, brake to 0 by ~35755, stands; creeps 0.12–0.17 ft/s with no input |
| race-probe | r-spawn | probe | gas, brake to a stop, Circle/R2/L2/Triangle: none moves +0x308..+0x314 |
| s-steer | s-spawn | steer-early | gas to 60 ft/s, full left 80823.., centre, full right 80951.., no wreck; a short air at ~80871 |
| s-steer-slow | s-spawn | steer-slow | as race-steer-slow |
| s-stoppie-short | s-spawn | stoppie-short | +0xb4 0011 from 80875, goes over the bars: wreck 80895, upside down, put back ~81051 |
| s-wheelie-launch | s-spawn | wheelie-launch | fronts off repeatedly, palm at 75 ft/s, wreck 80879, tumbling to 81031 |
| s-jump | s-air | jump | air pitch control, lands 80901, palm, wreck 80919 |
| s-jump-side | s-air | jump-side | air yaw, clean landing 80901, rides on at 80 ft/s |
| s-still | s-spawn | still | brake to 0 by ~80827; with no input it rolls back at 0.6 ft/s on the slope and keeps rolling |

The atv_s_ quad brakes from 36 ft/s to 0 in ~0.7 s (s-still), the atv_ quad from 27 ft/s in ~1 s
(race-still). The same stoppie pad that the atv_ quad rides out tips the atv_s_ quad over; that its 325
tire table is why was not checked.

## What the original does

- **The player ATV.** The ATV object holds the vptr 0x003ce250 at +4, so the object starts 4 bytes before
  the hit, as MD40's bike does. Its fields are in oracle.txt `# MD37a`.
- **A second ATV.** In both quads' states a second ATV object idles at 0x1657090 / 0x16573d0. Its role is
  not known.
- **Wreck word.** +0x6e4 goes 1 at the first wreck and stays 1 after the put-back.
- **The rear +0xb4 flag** reads 0011 exactly while touch is 1100 (fronts down, rears up), as the Force body
  says.
- **Physics files.** The game picks `atv_s_` files by the model name alone (`atv_phy_path`), the same rule
  as the PC's 0x00566480.

## How it was proved

A save-state capture: 15 PINE recordings at every tick, from 4 save states. Each was checked by
summary.py: the frame steps, and that each targeted branch's flags change in the rows. Two csvs miss one
tick each (step 4, step 10); all other rows step by 2. That the atv_s_ files loaded was checked from the
engine and tire constants in RAM, not from a file path.

## Not verified / not 1:1

- **The atv_s_ states are not a stock game.** The "ATV_S_" needle was poked to "ATV_R_" in RAM, so the R
  model rides the atv_s_ files. Every quad of that model would read atv_s_ too. A real ATV_S_ model (its
  mesh, collision, mass from `.sub`) was never ridden: none is unlocked on this memory card.
- **Which atv_s_ file loaded.** Whether it was `atv_s_race250a.phy` or `…p.phy` was not checked. The same
  holds for the atv_ quad, where the brief says atv_Race250A. `atv_phy_path` formats "Race250P"; where the
  A files are chosen was not read.
- **The jumps are thrown, not ridden.** The mid-air states come from +30 ft/s written into the body's
  velocity, not from a ramp. No Desert Oasis jump was found near the spawn.
- **The crashes are mostly collisions with one palm.** The palm stands straight ahead of the spawn. Only
  s-stoppie-short is a tip-over wreck; no airborne-angle wreck was captured.
- **The 0.987 hold** (mxvatv-pc.md) is ridden through in race-steer and s-steer (full lock held 1.5 s at
  speed), but the hold's own field was not found or recorded. Only the stick, the limited stick and the
  euler are in the csv.
- **The stand-upright under 10 ft/s.** race-still and s-still stand and creep below 10 ft/s, but whether
  the stand-upright torque ran was not read from a field.
- **One missed tick each** in race-steer-slow (step 4) and race-jump-side (step 10).
- **Unmapped fields.** The second idle ATV's role, +0x6e4's exact meaning, and the buttons Circle, R2, L2
  and Triangle on the ATV.
- **The `-late` RAM images** are a few frames after their states. The four state images are exact.

## Next

MD37b replays these: start from `atv-{r,s}-*.ee.bin` and step against `context/capture/md37a/*.csv`.
