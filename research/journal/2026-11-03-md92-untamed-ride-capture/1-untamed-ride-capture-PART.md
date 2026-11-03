# 1 — untamed ride capture (PART)

Task MD92. 2026-11-03.

## What I did

- **`research/md92/`** (git-tracked tools; no game data):
  - `bike.toml`: C15's spec at SLUS-21701's offsets (research/oracle.txt # MD81a). Bike 0x01be47f0, vptr
    0x003f1d10.
    - Decoded columns: vsync 0x410318, euler +0x390, speed +0xc28, ground +0x354, ctl +0x338..+0x344
      (+0x340 brake, +0x344 gas), stick +0x12b8..+0x12c4, flip +0x1184, wreck reason +0xc70, wreck count
      +0x708, and the body's pos/vel/quat/angvel.
    - Raw blocks: the object 0x0..0x1c00 (obj0/obj1/obj2), body (+0xe8, 640 B), node (+0x114, 256 B),
      collision record (+0xf8, 256 B), front/rear wheel (+0x17f0/+0x17f4, 0x920 B each) and engine
      (+0x143c, 512 B).
    - How the pointers were matched to SLUS-21104's: both wheels share one vtable and sit 0x920 apart,
      as in SLUS-21104 (+0x17b0/+0x17b4). The engine is the second of four pointers in the same order as
      SLUS-21104's +0x1428..+0x1434. Node and record are as in openmx-mods' air.toml and oracle.txt.
  - `play.sh`: research/c15/play.sh with the Untamed trigger. Rows arm on state 8's vsync 0x6b67 (27495)
    once the vptr is in.
  - `wreck.pad`: the ride, written in the language of research/c15/tape.py.
  - `check.py`: checks a capture against its tape, and one run against another.
- **The ride** (git-ignored, s1 only):
  - captures `context/capture/md92/wreck-{1,2,3}.csv` (+ .json);
  - tape `context/recordings/md92/wreck.p2m2` with `.ticks.csv`, made by
    `python3 -I research/c15/tape.py context/recordings/animandpump.p2m2 research/md92/wreck.pad
    context/recordings/md92/wreck.p2m2 --game "MX vs. ATV Untamed"`;
  - `_SaveState.p2s`, a byte-for-byte copy of state 8 (md5 ec3308a9…);
  - the start RAM image `context/untamed/ram/state08.ee.bin` (eeMemory.bin of state 8).

  The same files are also in `~/repos/openmx-slots/kept-context/md92/`, because C15's captures were lost
  from every slot.
- **Filed F31**: the SLUS-21104 bike ride replay the brief puts first. No task covered it. C15's captures
  and tapes are gone from all slots, so F31 starts by re-recording them.

### Recipe

1. Scratch kit.toml: this slot's, with name MX vs. ATV Untamed, serial SLUS-21701, iso
   `~/repos/openmx-mods/iso/MX vs. ATV Untamed (USA).iso`, elf SLUS_217.01 and vsync_addr 0x410318.
   Export `KIT_CONFIG=<it> KIT_PCSX2=1`.
2. `./kit pcsx2 copy 1` (copy 1 did not exist yet), then start it with `WAYLAND_DISPLAY=wayland-1
   XDG_SESSION_TYPE=wayland QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 ./kit pcsx2 start`.
   Wait for `/run/user/1000/pcsx2.sock.28012`.
3. `TMPDIR=<scratch> ./kit pcsx2 with -- bash research/md92/play.sh PID context/recordings/md92/wreck.p2m2
   "<s9 savestates>/SLUS-21701 (08FE7DFD).08.p2s" 665 context/capture/md92/wreck-N.csv`.
   - It needs no copy in copy 1's savestates; play.sh only uses the state as the tape's `_SaveState.p2s`.
   - A run takes about 30 s, because the 665-row budget ends before the movie does.
4. `python3 -I research/md92/check.py wreck-1 wreck-3`.

## What the original does (this ride)

State 8 is Rocksbury Free Ride, with six `Veh\mx_Race250P.phy` bikes (the name is at bike +0x137). The
player's bike holds Untamed's P limits, read at +0x13f8: front 20.533, back 24.933, side 21.267 and both
downs 79.2 ft/s (14 / 17 / 14.5 / 54 mph, as MD81a found in the files). Row 0 is vsync 27497, one tick
after the state.

| rows | what happens |
|---|---|
| 0–269 | gas from rest (0.6 ft/s) to 97 ft/s; the first lip at row 149 (air to 231), a second air 245–275 |
| 270 | gas off; a third air from row 301 |
| 340 | lands hard: wreck count 0 → 1, reason 1, speed −5 ft/s; the bike tumbles to rest |
| 340–488 | the wreck holds every input at 0, though the tape gives gas again from tick 450 |
| 489 | put back: reason cleared (inputs still 0 on this row) |
| 490–509 | gas and full left; 510–529 gas; 530–559 full right; 580–609 brake to 1 ft/s; gas; coast from 640 |

- **Tape to row:** tape tick *t* is read on row *t*, not on *t + 1* as in C15's SLUS-21104 captures:
  - row 0 already reads the gas, and tick 270's release reads on row 270;
  - after the wreck, steer ticks 480–509 read on rows 490–509, and brake ticks 580–609 read on rows
    580–609.

  check.py tests every row outside the wreck: 515 rows, all equal. A mutated check that reads tick
  *t − 1* (C15's offset) goes red with 8 problems.
- **Rows** step by exactly 2 vsyncs (27497..28825), with no gaps in any of the three runs.

## How it was proved

The same tape was played three times in copy 1 (wreck-1, -2, -3), each captured every tick.

- `check.py wreck-N` passes on all three runs: vsyncs, vptr, inputs against the tape, and one wreck.
- **wreck-1 against wreck-3:** all 35 decoded columns are equal on all 665 rows. Raw words differ in only
  39 places:
  - obj +0x74, a flag word (bit 0x400), on 15 rows;
  - obj +0x1a70 and body +0x260, a word that steps by 1, on 13 and 3 rows;
  - a contact/query block, on rows 366 and 647 only: obj +0x8a0..+0x998, +0xd28..+0xd5c,
    +0x1ad0..+0x1bd8. That is the contact list read mid-refill (memory
    a-duplicate-capture-row-is-a-late-read).

  The wheels, node, record and engine are identical.
- **wreck-2 is the odd run:**
  - its wreck count tears on row 340 and its reason on row 489; check.py's run compare flags both;
  - the same contact block differs on 66–80 rows, and about 50 snapshot words on 7 event rows (63, 340,
    483, 489, 497, 570, 580);
  - its pos/vel/quat/angvel still equal the other runs on every row.

  So the physics repeats run to run, and the recorder's read phase does not. Replay against wreck-1, and
  use wreck-3 to tell a tear from a real difference.

## Not verified / not 1:1

- **No replay.** The brief's prerequisites have not landed:
  - the SLUS-21104 bike ride replay (F31, new; C15's fixtures must be re-recorded first);
  - MD93a–d, Untamed's reworked backflip, spin in the air, vehicle update, and the air state /
    no-hander classification.

  So nothing here is compared against the port, and the pack-off hashes were not checked: no code
  changed.
- **The row's phase within the tick** is not established. Rows are read at the same point in most runs,
  but one run of three read late on its event rows.
- **The decoded `ctl` and `stick` offsets** are SLUS-21104's moved by +0x30 and openmx-mods' air.toml. I
  checked them only by their values against the tape: gas, brake and steer each read 1.0 or −1.0 exactly
  on the tape's ticks. ctl.x and ctl.y were never non-zero, and are not named.
- **The engine pointer** (+0x143c) is matched by pointer order and vtable shape, not read from code.
- **The object's size** (0x1c00) is a guess that covers the last pointer seen (+0x18f4).
- **The rider and the world are not captured.** Nothing outside the bike's blocks is read; it comes only
  from the start image.
- **No clutch, preload (d-pad), trick or air lean** is in the ride. The wreck is a plain hard landing
  (reason 1), so the reworked backflip/spin are only ever run idle.
- **The tape's last 5 ticks** (665..669) are not read: the 665-row budget stops first.

## Next

When F31 and MD93a–d have landed:

1. Seed an Untamed ride from `context/untamed/ram/state08.ee.bin` (bike 0x01be47f0, mx_Race250P off the
   untamed_tricks pack), the way F31 seeds SLUS-21104's from state04.ee.bin.
2. Feed `wreck.p2m2.ticks.csv` tick *t* on row *t*, and compare the bike, body, wheel and engine words per
   row with wreck-1. A word that wreck-1 and wreck-3 disagree on is a recorder tear.
3. Check that every replay hash is unchanged with the pack off.

The captures are in this slot's context/ and in `~/repos/openmx-slots/kept-context/md92/`.
