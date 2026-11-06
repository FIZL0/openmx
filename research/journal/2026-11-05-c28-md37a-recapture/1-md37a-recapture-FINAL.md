# 1 — md37a recapture (FINAL)

Task C28. 2026-11-05.

## What I did

MD37a's (research/journal/2026-10-29-md37a-atv-captures) four SLUS-21104 states and 15 ATV rides were
on no disk any more. I re-made them by its recipe, with one change: `/dev/uinput` is root-only again
(`crw------- root`, no ACL, since the 11-01 reboot), so `./kit pad` and `./kit shot` cannot run. Every
press, menu and ride, is a generated from-savestate `.p2m2` (C15's `research/c15/tape.py`) played in
PCSX2 copy 2 over AT-SPI, and screenshots are PCSX2's System → Screenshot over AT-SPI.

New, git-tracked, research/c28/:
- tape.sh: play a tape from a .p2s with no capture, then a screenshot (menus).
- shot.py: System → Screenshot over AT-SPI, copied out of the copy's snaps/.
- throw.sh + throw.py: MD37a's throw.py with the gas held by a tape instead of the pad.
- record.sh: MD37a's record.sh as a tape: research/md37a/PAD → `context/recordings/c28/NAME.p2m2`,
  played from the state, research/md37a/atv.toml recorded from the state's own vsync on (C15's trigger:
  `vsync` in [state's, +26] and the ATV's vptr 0x003ce250 at +4; two columns added, `vsync`, `vptr`).

### The states

1. Scratch kit.toml (this slot's, serial SLUS-21104, the MX vs. ATV iso, elf SLUS_211.04, vsync
   0x3fa6d8), `./kit pcsx2 copy 2` (the copy had gone), M21's state 04 copied in as copy 2's slot 7.
2. Menu tapes from slot 7 (each press `down X / sleep 200-300 / up X`, 1.2-1.5 s apart):
   Start → up → Cross (Return to Main Menu) → up → Cross (YES; NO is the default) → 25 s load → saved
   slot 10 at the main menu. From slot 10: Cross (Single Player), down, down, Cross (Free Ride), 5 s,
   eight downs (Desert Oasis), Cross, down, right (Vehicle ATV), 5 s rest (the ATV loads; a press
   in it is dropped), up → START EVENT highlighted. Saved there as slot 12, and with
   `./kit pine write 0x39f4e5 0x52 --as u8` ("ATV_S_" → "ATV_R_") as slot 13.
3. From 12 and 13: Cross, 40 s, saved at the spawn → slot 1 `atv-r-spawn.p2s`, slot 3 `atv-s-spawn.p2s`.
4. `throw.sh` (gas to 45 ft/s, +30 ft/s on body +0x84, save the next frame) → slot 2 `atv-r-air.p2s`,
   slot 4 `atv-s-air.p2s`.

Checked from each state's RAM: the player ATV is at 0x150a990 in all four (vptr 0x003ce250 at +4; the
second, idle ATV at 0x1581a70). Final drive (engine +0x34) and the wheels' +0x778: r 4.558 / 150,325,150,150,
s 10.538 / 150,325,325,325, as MD37a read for atv_ and atv_s_. The air states' body velocity y is 30.15
(r) and 28.88 (s). The old atv-r-spawn.ee.bin reads the same 4.558 / 150,325,150,150.

### The rides

Copy 2's NominalScalar is 0.12 (inis/PCSX2.ini): at 1.0 the recorder kept 15 rows a second against the
game's 30 under a load average of 140, and race-steer had 9 gaps; at 0.25, 1 gap; at 0.12, none.
The tapes are timed in fields, so the speed does not change a ride. The ini is back at 1.0 now.

Every csv is 300 rows, every row 2 vsyncs after the last (`summary.py`: `steps {2: 299}`). Rows start
on the state's vsync + 2: r-spawn 21369, r-air 21459, s-spawn 20961, s-air 21033.

| csv | state | pad | what happens (vsync), against MD37a's |
|---|---|---|---|
| race-steer | r-spawn | steer | gas to 74.8 ft/s, full left, centre, full right, no wreck; one air 21541-21567 (MD37a: one 0111 hop) |
| race-steer-slow | r-spawn | steer-slow | right then left lock from 26.5 ft/s to a stop, ends rolling back −0.86 |
| race-stoppie-short | r-spawn | stoppie-short | touch 1100 and rear +0xb4 0011 from 21523 at 40 ft/s, brake 0.25, pitch to −1.15, rides out, no wreck |
| race-wheelie-launch | r-spawn | wheelie-launch | fronts off again and again from rest, wreck 21529 at ~80 ft/s, tumbling to ~21693 |
| race-jump | r-air | jump | air pitch −0.47..1.20, lands 1111 at 21561, wreck 21591, put back ~21653 |
| race-jump-side | r-air | jump-side | air yaw −2.08 → −0.97 and back, clean landing 21559; **then a wreck at 21825** (MD37a: none) |
| race-still | r-spawn | still | gas to 32 ft/s, brake to 0, stands, 0.13 ft/s at the end |
| race-probe | r-spawn | probe | gas, brake, Circle/R2/L2/Triangle: +0x308/+0x30c stay 0 |
| s-steer | s-spawn | steer-early | gas to 74 ft/s, full left, centre, full right, no wreck; a short air 21119-21139 |
| s-steer-slow | s-spawn | steer-slow | as race-steer-slow, from 31.7 ft/s |
| s-stoppie-short | s-spawn | stoppie-short | 1100 / +0xb4 0011 from 21115 at ~45 ft/s, pitch to −1.01, **rides out, no wreck** (MD37a: over the bars, wreck) |
| s-wheelie-launch | s-spawn | wheelie-launch | fronts off repeatedly, wreck 21127 at ~79 ft/s, put back ~21279 |
| s-jump | s-air | jump | air pitch control, lands 21133, wreck 21183, put back ~21337 |
| s-jump-side | s-air | jump-side | air yaw, clean landing 21131 (1100/0011 for one row), rides on |
| s-still | s-spawn | still | gas to 40 ft/s, brake to 0; ends at 0.16 ft/s (MD37a: kept rolling back at 0.6) |

The tapes play each pad's sleeps as exact ticks. MD37a's live pad seemingly did not: its stoppie
started at 47 ft/s where the same 2.2 s of gas now reaches 66 ft/s (the r quad, 21509), so its
emulator probably ran slower than real time and its rides were shorter in ticks. One extra try,
stoppie-short with 1.3 s of gas (~47-51 ft/s on the s quad), stayed at pitch −0.58 and did not tip
either (and a load spike tore it: thrown away).

## What the original does

Nothing new about the ATV. About the menus: on the WARNING (quit) box NO is the default; in Track
Selection the first down after entering can be dropped unless it waits ~5 s; after Vehicle → ATV
a press within ~1 s is dropped while the quad loads.

## How it was proved

A save-state capture again, 15 PINE recordings at every tick. `python3 research/md37a/summary.py NAME`
on each: every row steps by 2 (table). The first row of each holds the ATV's vptr (record.sh checks it).
Each targeted branch was looked for in the rows (touch, +0xb4, wreck, speed, pitch, yaw), as in the table.

The gate's ATV tests read these csvs, and they had been skipping without them. To show they ran, I made
`ps2_oracle::skip` panic for one run (then put it back): atv_ground (2), atv_search (3), atv_wheel (7) and
atv_quad (65, the three Desert Oasis tests among them) ran on the new csvs and passed. Only these skipped:
atv_gather's five mrider tests (no mrider.phy) and atv_quad's the_quad_images_wheels_are_their_phy_files
(no phy/). Neither file is in any slot (MD113).

Backed up: ~/repos/openmx-slots/kept-context/md37a/ mirrors context/ (capture/md37a, mxvatv-ps2/md37a,
recordings/c28; README.txt). Copy 2's savestates keep slots 1-4 (the four states) and 12/13 (START
EVENT, plain and poked). All six are byte-identical in context/, kept-context and the copy (md5).

## Not verified / not 1:1

- **Not MD37a's rides.** The states are new (another session, another heap) and the input is a tape
  played from the state instead of a live uinput pad, so no csv equals MD37a's row for row. The ATV sits at
  0x150a990 instead of 0x15e0690 (r) / 0x15e0b90 (s). A test that pins MD37a's addresses or rows does
  not hold for these.
- **The old atv-r-spawn.ee.bin stays.** It is MD37a's image, which the gate's ATV tests read at pinned
  addresses (QUAD_IMAGES in tests/atv_quad.rs, atv_gather/atv_search/atv_spatial's tree). It is not the
  image of the new atv-r-spawn.p2s. The new states' images are `c28-atv-{r,s}-{spawn,air}.ee.bin`, so no
  test reads them by the old names (MD112 filed to stop the tests pinning the addresses).
  `atv-{r,s}-spawn-late.ee.bin` were not re-made.
- **s-stoppie-short no longer tips over**, so the set has no tip-over wreck now. Its wreck was MD37a's
  only one. The faster entry (66 vs 47 ft/s on r; see the timing note) is the likely reason, but the
  1.3 s-gas try did not tip either. Why MD37a's did was not found.
- **race-jump-side wrecks** 266 ticks after its clean landing (MD37a's did not). The row data is fine.
  It just rides on further, probably into the palm (not checked on screen).
- **s-still's roll-back** is 0.16 ft/s at the end, not MD37a's 0.6.
- **The menu tapes are not deterministic** from one play to the next: the same tape from slot 10 once
  stopped on Little Rock, Desert Oasis or Killington (a dropped or extra down). The pads in research/c28
  have longer rests and played right, but each step was checked by a screenshot, not proved to repeat.
- **The tests that read the csvs prove the port against them generically** (ground query, wheel, search,
  Desert Oasis contacts); nothing that pinned MD37a's rows exists any more, so nothing compared old rows
  with new ones.
- Everything MD37a's own section lists still holds: the atv_s_ states are the poked R model, the
  jumps are thrown, the crashes are mostly the palm, the 0.987 hold and stand-upright fields unread.

## Next

MD37b and the ATV tests can use the csvs again (copy kept-context/md37a into a slot's context/).
MD112: find the quad in each image so the c28 images are tested. MD113: re-extract mrider.phy and phy/.
