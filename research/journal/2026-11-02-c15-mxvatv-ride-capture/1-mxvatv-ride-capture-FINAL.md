# 1 — mxvatv ride capture (FINAL)

Task C15. 2026-11-02.

## What I did

- research/c15/ (git-tracked tools, no game data):
  - `bike.toml`: the watch spec. Decoded columns (vsync, vptr, euler, speed, ground, ctl +0x308..,
    d-pad/stick +0x12a0.., clutch/reverse/steer/front brake +0x12c0.., trick byte +0x14ec, backflip words
    +0x1194.., wreck count +0x6e4, body pos/vel/quat/angvel), then raw blocks: the bike object
    0x0..0x1800 (`obj0`, `obj1`), its rigid body (+0xb8, 640 bytes), node (+0xe4, 256), collision record
    (+0xc8, 256), front and rear wheel (+0x17b0/+0x17b4, 0x920 each, the gap between them) and engine
    (+0x142c, 512). The bike is 0x00e714e0 in state 4 (research/oracle.txt `# MD40`).
  - `tape.py`: writes a from-savestate `.p2m2` from a `./kit pad`-language script timed in game ticks
    (`sleep Nt`), two fields a tick, pad 2 blank, plus `OUT.ticks.csv` (the pad held each tick).
  - `play.sh`: C8's play.sh (C6's recipe) with this spec; rows arm on the state's own vsync (0x3333) once
    the bike's vptr 0x003d0450 is in.
  - `ride.pad`, `flip.pad`: the two rides. `summary.py`: decoded rows. `same.py`: two runs compared.
- Captures (git-ignored, s1 only): `context/capture/c15/{ride,flip}-{1,2}.csv` (+ .json), tapes
  `context/recordings/c15/{ride,flip}.p2m2` with `.ticks.csv` and `_SaveState.p2s` (byte-for-byte state 4
  of ps2-decomp-kit-slots/pcsx2/s9, md5 73389cd1…). The start RAM image is `context/mxvatv-ps2/state04.ee.bin`.

### Recipe

1. Scratch kit.toml: this slot's, with serial SLUS-21104, iso `~/repos/openmx-mods/iso/MX vs. ATV
   Unleashed (USA).iso`, elf SLUS_211.04, vsync_addr 0x3fa6d8. `export KIT_CONFIG=<it> KIT_PCSX2=1`.
2. Copy `SLUS-21104 (95BB1901).04.p2s` from ps2-decomp-kit-slots/pcsx2/s9/PCSX2/savestates (removed again
   afterwards; play.sh only needs it as the tape's `_SaveState.p2s`).
3. `WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
   ./kit pcsx2 start`; wait for /run/user/1000/pcsx2.sock.28012.
4. `python3 -I research/c15/tape.py context/recordings/animandpump.p2m2 research/c15/ride.pad
   context/recordings/c15/ride.p2m2`.
5. `TMPDIR=<scratch> ./kit pcsx2 with -- bash research/c15/play.sh PID context/recordings/c15/ride.p2m2
   <state.p2s> 459 context/capture/c15/ride-1.csv`. When the movie ends PCSX2 stops ticking, and the
   recorder waits out KIT_FRAME_STALL (300 s) before it flushes, so a run takes ~6 min. The "pad Guide
   button" warning in the log is the recorder's unpause attempt with no uinput pad; harmless here.

## What the original does (these rides)

| ride | rows | vsync | what happens (row = tick after the state) |
|---|---|---|---|
| ride | 459 | 13109..14025 | gas from 15.5 ft/s; first lip ~77 ft/s, air rows ~85–130 (ground 0), lands clean; second air ~160–205; full left rows 210–239 (heading −2.2 → +0.45), full right 255–284, gas to 68 ft/s, brake (Square) 315–359 to 9.5 ft/s, gas to 58, coast at 52 |
| flip | 465 | 13109..14037 | same to row 45, then R1 + stick forward (trick nibble 4), stick back rows 66–71; the backflip arms and fires at row 79 (+0x1194 applied and +0x119c active = 1), pitch past 1.2 by row 90 and over, lands on its back at row 125 (wreck count 1, gas/trick inputs read 0 from row 126), put back ~row 194, then steer, brake, gas as in ride |

- **Tape to row:** tape tick *t* (fields 2t+1, 2t+2) is first read on row *t + 1*: gas reads 1.0 from
  row 1, the stick from row 46 for tick 45, and so on for every change in both tapes. Row 0 reads no
  input (gas 0), so tape field 0 (the blank one) is not seen as a press.
- **Rows** step by exactly 2 vsyncs (one 30 Hz tick), with no gaps in any of the four runs.
- **Row phase:** (pos(n+1) − pos(n)) × 30 lies between vel(n) and vel(n+1), nearer vel(n), as a
  substepped integrate gives. Which point in the tick a row is read at is not established (below).
- **Inputs as the bike holds them:** stick ±1.0 exactly at full deflection (`axis()` writes 0x00/0xff),
  gas +0x314 and brake +0x310 1.0 with Cross/Square (pressure 0xff), trick byte low nibble 4 for R1 alone.
  The trick word reads 0x636f6c00 with no button (1668246528): only its low byte is the buttons.

## How it was proved

A PINE recording at every tick, twice per tape: `research/c15/same.py ride-1.csv ride-2.csv` → 459 rows,
every column identical, raw blocks byte for byte; `flip-1` vs `flip-2` → 465 rows, identical. `ride` vs
`flip` are identical up to vsync 13201 (row 46, the first tick their tapes differ). So the tape plus state 4
gives the same ride every time: a fixture a replay can be held to bit for bit. The tape-to-row mapping was
read from the bike's own input words against `.ticks.csv` at every change.

## Not verified / not 1:1

- **Row phase within the tick.** The rows are read at the same point every run, but whether that is the end
  of the game tick, or part way through the frame, was not established. The replay test will have to find
  it (memory: capture-rows-read-at-three-phases is MX Unleashed's version of this).
- **The rider and the world are not captured.** Only the bike object, body, node, collision record, two
  wheels and engine. Anything the step reads elsewhere (the rider's stunt ids it reads through the bike,
  terrain, the AI riders if any) comes only from state04.ee.bin at the start.
- **The end of each tape is not read.** The movie ends and PCSX2 stops ticking: `ride` rows end at tape
  tick 457 of 460, `flip` at 463 of 466.
- **Row 0 is one tick after the state**, not the state itself (pos 2247.24 vs the image's 2246.76). The
  first tick's input (none read) is assumed to be the blank field 0.
- **The wheel block size** (0x920) is the gap between the two wheel objects, not a size read from code.
- **No clutch, d-pad (preload), 360 or airborne lean** is in either ride: Circle/L1/the d-pad were not
  pressed, because which button is the clutch on MX vs. ATV's PS2 pad was not looked up.
- **Only one backflip, and it wrecks.** It is M21's flip.pad timing, tried once; no landed backflip was tuned for;
  this one exercises the flip's firing and the over-rotation wreck/put-back (MD71), not a landing.
- Captures and tapes are only in s1's context/ (git-ignored).

## Next

A replay test for Handling::MxVsAtv once MD63e/MD63f/MD71 etc. land: seed from state04.ee.bin, feed
`ride.p2m2.ticks.csv` tick *t* on row *t + 1*, compare the bike/body/wheel words per row; `flip` after the
wreck and put-back are ported.
