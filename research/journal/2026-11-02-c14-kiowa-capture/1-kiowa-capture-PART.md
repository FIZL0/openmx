# 1 — kiowa capture (PART)

Task C14. 2026-11-02.

## What I did

- **A Kiowa save state of my own.** S148's slot-5 states have been overwritten since, and S148 drove the
  pause menu with the uinput pad, which is root-only again (`/dev/uinput` is `crw------- root`). So:
  1. On copy 1 of s1, `./kit pcsx2 with -- python3 -u research/journal/2026-10-29-s148-kiowa/boot.py
     Free09 2 5 6` (S148's boot: event machine1 kind 2, skin 0). In game about 4 s after PINE.
  2. The cheat flag (`read32(0x004e2778) + 0x5c = 1`), then `save_state(7)`.
  3. A generated tape played from slot 7 (`research/c14/make_tape.py` writes it,
     `research/c14/play.sh` plays it with no recorder) does the pause menu:
     `60 6:start 40 6:down 30 6:down 30 6:down 30 6:up 30 6:cross 120 6:cross 400` (fields; a segment
     is FIELDS[:BUTTONS[:LX,LY]]). Afterwards `research/c14/actors.py` shows the helicopter
     (0x007ccae0, MX vtable 0x0045c980) as the first actor, now owned by the human's player record
     (ctl src 1; before the switch it had src 2), and the rider with no player.
  4. Movie stopped, resumed, `save_state(2)`: the Kiowa state, at frame 1203, running, flown.
- **The flight tape**, `context/recordings/c14/flight.p2m2` with slot 2 as its `_SaveState.p2s`:
  `60 180:cross 240 90:-:127,0 180 60:r1 60:-:0,127 120 600:square 120`, i.e. rest, climb, hover,
  stick forward, rest, right rudder, stick left, rest, descend, rest. 1,710 fields; it played two
  fields a tick, as S100 found for P21's generated tapes, so 854 ticks.
- **The capture spec** `watch/c14-kiowa.toml`: per tick the frame, dt, rng, the 16 controls and both pad
  halves, the helicopter's class/owner, the "pilot flag", climb, descend, the three sticks, the four
  commands, two raw blocks of the helicopter (+0x330..+0x3e0: every controller word, the hold timer,
  integral, speed command, wreck word; +0x460..+0x4d0: the collective filter and the measure's
  readings), the rotor's state (+0xd0..+0x120), the aero's outputs (+0x128..+0x144), and the body
  (position, velocity, quaternion, rates, and +0x0..+0x200 raw).
- **The capture script** `research/c14/capture.sh`: C6's `play_capture.sh` with the rider trigger
  swapped for `heli_ctl_src == 1 and heli_class == 0x0045c980`. It strips `required = true` from the
  spec copy, as C6's does: a required field is null during the System → Reset boot, and the recorder
  gives up on it before the start trigger ever arms (the first try failed that way).
- **Captured twice** on copy 1 at quarter speed (`NominalScalar = 0.25`, S219's setting; put back to
  1.0 afterwards): `context/capture/c14/flight.csv` (+ `.json`) and `flight-run2.csv`. Frames
  1204..2057 each, 854 rows, 0 gaps.
- **The start state as a RAM image**, for the replay to seed from: `context/states/c14-kiowa.ee.bin`
  and `.spr.bin`, unzipped from the tape's `.p2s` (frame 1203: the row before the first; the
  helicopter at 0x007ccae0, body 0x00606f30, at rest at (3025.32, 131.94, 4053.57)).
- All of `context/capture/c14`, `context/recordings/c14` and the two images are in s1's `context/`, and
  copied into s2–s5's (git-ignored, not committed). S148's `s148-kiowa.ee.bin` is now in s1 too.

## What the capture shows

| Ticks (frame_counter) | Pad | What the helicopter does |
| --- | --- | --- |
| 1204–1234 | nothing | on the ground, y 131.94 |
| 1235–1324 | Cross | climbs at up to ~49 ft/s to y 254.7 |
| 1325–1444 | nothing | holds: rises to 294.6 (1394), settles at ~293.5 |
| 1445–1489 | stick forward (+0x344 = 1) | to −14 ft/s in x |
| 1490–1579 | nothing | the speed command snaps back; slows to −7.8 |
| 1580–1609 | R1 (+0x348 = −1) | pedal command −1 |
| 1610–1639 | stick left (+0x340 = 1) | banks; turns towards −z |
| 1640–1699 | nothing | ~27 ft/s level |
| 1700–1999 | Square | descends at up to ~60 ft/s; touches down at 1816 (y ≈ 123.7), bounces until ~1894 |
| 2000–2057 | nothing | on the ground, sliding at ~1 ft/s on the slope |

- The pad words lag the tape by one tick (Cross at field 60 = tick 30 shows on frame 1235).
- No wreck: +0x3d4 is 0 and +0x364 (flown) 1 on every row. dt is 0.03333 on every row.
- The generator moves: 323 distinct `rng_state` values over the ride, so a replay has to draw what the
  original draws (particles or sound, not looked at).
- **S148's "pilot flag" `*(+0x170)+0x40` is a ground contact**, not a pilot flag: it reads 1 at rest,
  drops to 0 three ticks after the climb starts (1238), comes back at the touchdown (1816) and flickers
  through the bounces (1838–1894). +0x170 is most likely the rigid-body part's collision (the car layout
  in oracle.txt has its collision at the same +0x170); not read in code. With the flag set and climb 0, the controller zeroes the
  collective: on the ground the collective command reads 0.000 from 1844 on.

## How it was checked

- The two runs agree on every lane except the pad buffers and stamps on 847 of 854 rows. The other 7
  (1289, 1290, 1294, 1298, 1380, 1381, 1391) differ in `flight-run2.csv` only: there, five of them
  equal the next row's body word for word and two sit on the next tick's position. `flight.csv`'s
  rows lie on the line between their neighbours, and none of its rows repeats its neighbour's body.
  So `flight-run2.csv` was read late on those 7, and `flight.csv` has no torn row that the second run
  could show (`research/c14/compare.py`).
- `research/c14/look.py` lists gaps (0), repeated moving rows (none), the owner and class (1 and
  0x0045c980 on every row) and the table above.

## Not verified / not 1:1

- **No replay.** The brief's test (the capture replays bit-exact through the ported step) is not
  written: the port has no Kiowa tick yet. The measure listener is S220a (in progress in s2), and the
  input step and the `Kiowa` tick that joins input, preforce, force and measure into
  `biplane::World::step` are S220b. Blocked `--after S220a,S220b`.
- No screenshot of the switch or the flight: the copy has no pad for the Screenshot hotkey and the
  desktop can't be grabbed. That the human flies the helicopter is read from RAM (owner src 1, the
  rider without a player, the pad words following the tape).
- The state was made from a tape, not by hand: if the "Helicopter Flight Controls" card had still been
  up, the frame counter would have stopped; it ran.
- One flight, one place (free09's spawn); no wreck, no high-speed pass, no climb past MaxAltitude.
- The rows are read after the tick, at quarter speed; a second run is the only tear check.
- What the original draws from the generator during the flight is not known.

## Next (for the replay, once S220a and S220b are in)

- Seed the port from `context/states/c14-kiowa.ee.bin` (the helicopter at 0x007ccae0 and its body at
  0x00606f30, as in S148's image), feed the pad (`climb`, `descend`, `stick` x/y/z; first find whether a row's
  pad words are the ones its own tick read, as S58's PAD_LEADS had to for the bike), step, and compare the body and `heli_a`/`heli_b`/`rotor_state`/
  `aero_state` row by row. The world's ground and the collision are needed from 1816 on.
