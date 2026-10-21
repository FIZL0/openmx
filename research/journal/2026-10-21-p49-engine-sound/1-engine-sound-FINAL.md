# 1 — engine sound (FINAL)

Task P49. 2026-10-21. The user: the bike does not sound like the original, though P42 plays the
disc's own sounds.

## What I did

- **Found why it sounded wrong: every sound was the archive's previous file.** P42 read each record of
  `Aud\audio.res`'s directory as `[offset][size][name length][name]`. The record is really
  `[name length][name][offset][size]`, after a header of the count and one word nothing reads. So
  P42 gave each name the file before it in the archive. On the 125 this meant:
  - the idle (`idle03`) played `high11`, a high rev;
  - the decel (`decel01`, 2.2 s) played `clutch01`'s 0.25 s;
  - the clutch played whatever file sits before `clutch01`.

  The same was true of every wreck, landing, crowd and world sound and every song. Fixed in
  `Bank::parse` (`crates/openmx/src/audio.rs`).
- **Two P42 facts fall with it.** College Kids is a full song, not a 28 ms stub, and is listed now. And
  `Crowd\cheer.vig` is stereo like every other `.vig`, not a mono VAG. Every `.vig`'s per-channel size
  runs one 32-byte block pair past its file, which P42's VAG-or-stereo test had mistaken for a VAG,
  so `Clip::vig` now calls a file a VAG only when it is too short to hold a 0x800 header and one
  channel. The tests `the_discs_sounds_decode` and `the_music_sources_are_listed_and_switched` say so.
- **Replaced P42's engine with the original's note machine, ported line by line** (`Brain`,
  `audio.rs`). The original's engine sound is a chain of whole samples chosen by a state machine
  over four hands (grounded, gas, speed, clutch lever). There is no rpm pitch, which P42 had added
  as its own (it said so), and no per-gear sample. The states:

  | # | State | What it plays |
  |---|---|---|
  | 0 | Off | — |
  | 1 | Idle | Idle[0], looping |
  | 2 | LowAccel | LowAccel, at or under LowTorqueSpeed |
  | 3 | HighAccel | HighAccel, above it |
  | 4 | Power | follows the accelerations |
  | 5 | Sputter | alternates with Power |
  | 6 | Decel | off the gas, or 0.3 s airborne on the gas |
  | 7 | Clutch | clutch lever in with gas, moving |
  | 8 | Rev | the free rev: Panic[0], handed on at RevMidpoint |
  | 9 | Panic | the limiter: Panic[0] paused, the Panic[1] loop over it |
  | 10 | Unwind | coming back from the limiter |

  - Each list picks with no immediate repeat on the game's own `rand` draw, which the port takes
    from its own `Dice`; the sim's generator is not touched.
  - A started sample (not Idle[0]) is detuned by ±`PitchVariance` (0 on the 125 and the 250).
  - The next sample starts once the current one is within `OverlapTime` of its end.
  - `Engine` puts it on the mixer: each started sample is its own voice playing out under the next
    unless cut. Doppler is the original's: the closing speed along the line, level components only,
    in mph truncated, `× DopplerFactor / 1125`, and only above 0.01. Distance falloff is the file's
    `Min/MaxAttenuationDistance`.
- **Fixed the clutch hand.** The original's hand is the clutch lever (rider `+0x644`, the port's
  `Controls.clutch`). P42 fed it `held.clutch`, the clutch the engine is driven through, which is 1
  whenever the bike stands, so the note would have revved at every standstill (`main.rs`,
  `seat_sound`).
- **Recorded the original** (copy 1, `capture/run.sh`) on the human's `first_recording` state:
  - stand 4 s, gas 6 s (the bike leaves the ground once), off 4 s, two 0.6 s blips, gas 8 s, off 5 s
    (`capture/ride.pad`);
  - PCSX2's Video Capture records the SPU's output (the copy's speaker is muted); `press.py` and
    `save_as.py` drive its menu over AT-SPI;
  - `capture/hands.py` logged the note's four hands, its state, next state and queue head every frame
    over PINE.
- **Recorded the port over the same inputs.** `the_originals_ride_replays` (ignored; it needs
  `context/p49/`) feeds every logged frame's hands into `Engine` with `vehicle_125a.aud`, the ride's
  own bike, and renders 48 kHz stereo. It also writes each engine clip alone for `capture/find.py`.
- **Recordings and spectrograms are under `context/p49/`**, not committed: they are game data and
  binaries.
  - `original.mp4`, the capture.
  - `original.wav`, its audio (48 kHz stereo, 40 s).
  - `original.csv`, the hands log: 1199 rows at the game's 30 Hz.
  - `port.wav`, the port's render (48 kHz stereo, 40 s).
  - `original.spectrogram.png` and `port.spectrogram.png`: ffmpeg `showspectrumpic`, 0 to 8 kHz on a
    log scale.
  - `clips/*.wav`, the 19 engine samples.
  - The spectrograms in words:
    - The original's is broadband from end to end: the game's music and the world sounds are in the
      capture and sit 10 to 20 dB over the engine at idle.
    - The port's is the engine alone. It is quiet at idle, and loud bands from 100 to 400 Hz, with
      harmonics up to 5 kHz, sit at 6.9–8.4 s, 9.7–12.4 s, 16.9–17.5 s, 19.0–19.6 s, 21.1–25.6 s and
      26.9–28.2 s.
    - Those are exactly the original's on-gas states in `original.csv`. In the original's
      spectrogram the same spans show as brighter low bands under the music.
- Looked at opentennis's audio (N3, `research/journal/2026-10-07-n3-audio/`). Taken: the method, not
  code. There, every key-on was resolved through the save state's own loaded-bank tables to the exact
  sample the original plays. Here, that meant reading the state's sound objects and the SPU sample
  table to see which file each name had, which is what found the archive bug. Their IOP pitch table
  and ADSR do not apply: this game's VAGs play at their own rate.

## What the original does

- **The frame.** `./kit fn 0x30fcb0` (a0 the note, f12 dt), unless muted: run the decider
  `./kit fn 0x30ffc0`. If it started nothing, run the chain `./kit fn 0x311160`. Then run the
  emitter's own update, `./kit fn 0x1b6f30`. The starters are `0x310370` idle, `0x3104f0` accel,
  `0x310ad0` power, `0x310c50` sputter, `0x310980` decel and `0x310dd0` the rev, clutch and limiter.
  P42 had four of these read as the wrong states; `research/oracle.txt` has the corrected list.
- **The decider**, in order:
  - Airborne with gas, it revs once (the gas newly pressed), else it falls to Decel when the
    AirborneTimeToDecel timer runs out on a pulling note.
  - Airborne without gas, it unwinds the limiter or decelerates.
  - On the ground with the clutch lever in, gas revs, or engages the Clutch note above 1 ft/s.
  - Gas alone accelerates from Idle, Decel, Off, Rev, Panic or Unwind, and goes to Power from Clutch.
  - Off the gas, a pulling note decelerates.
- **The chain.** Once the current sample is within OverlapTime of its end, or not playing, it starts
  the next state's sample. A looping sample never ends. The free rev hands on at RevMidpoint instead.
- **The hands**, `./kit fn 0x280fb0`. The note's `+0x200` table points at the rider's per-frame
  snapshot:
  - grounded `+0x93c`
  - gas `+0x888`: a copy of `+0x658`, the rate-limited throttle
  - speed `+0x860`
  - clutch `+0x890`: a copy of `+0x644`, the lever
- **The sample lengths** the chain counts are the SPU table's (`./kit fn 0x19d860`: `spu+0x3e8 +
  8·slot`). The loaders write them to each sound's `+0x24`. In `first_recording` the 125's 19 sounds
  sit in slots 0x1f–0x31 in the archive's order. Each length is the file's own once the directory is
  read as `[len][name][off][size]`.
- **Gear-shift cue: none.** The note machine has no gear input, and the 125 and 250 lists have no
  per-gear sample. The "shift" heard in the original is the chain moving from LowAccel or HighAccel
  into Power and Sputter.
- **The landing cue** is not the note's. Landing on gas restarts an acceleration (Decel → HighAccel at
  9.74 s in the log); the thump is P42's landing sound, unchanged here.

## How it was proved

- **Function oracle, `the_engine_note_matches_the_original`** (`crates/openmx/tests/audio.rs`).
  - It runs the original's `0x30fcb0` on the `first_recording` RAM image against `Brain::frame`.
    There are 1000 runs of 24 frames: random notes, hands, dt (1/60, 1/30 or up to 0.05 s), sound
    stages (playing, position, heard, play gate) and bike parameters (lengths, looping, RevMidpoint,
    OverlapTime, PitchVariance, AirborneTimeToDecel).
  - The sound and emitter calls are hooked. Every frame the whole note must match: state, next,
    timers, flags, the no-repeat memory and the generator's state. So must every start (with its
    pitch), stop, pause and loop, in order.
  - Every state was reached (counts by state, 0–10: 680, 3789, 2503, 2879, 2749, 1262, 4424, 884,
    2963, 878, 989).
  - Totals: 3031 starts (1411 detuned), 3532 stops, 233 limiter holds.
  - A planted mutation (detuning the free rev instead of the idle) was caught, then reverted.
- **`the_ride_bikes_samples_are_the_originals`** (state + disc). Every one of the 125's 19 samples, as
  the port reads them, is as long as the original's loaded sound (its `+0x24`, within 2 ms) and loops
  as it does. Before the fix, all 19 lengths were wrong.
- **The PINE log against the port.** `the_originals_ride_replays` plays the 1199 logged frames of the
  original's hands into the port. The port's note is the original's on **1199 of 1199** frames, where
  it fails. Before the archive fix it was 942 of 1199: the port's 0.25 s "decel" handed back to the
  idle about 2 s early every time.
- **Which sample sounds in the original's recording** (`capture/find.py`, a normalised
  cross-correlation of the first 0.2 s of each port clip against `original.wav`).
  - For each of the 17 sample starts in the log, it ranks all 19 clips within ±0.15 s. The capture
    runs 0.11 s ahead of the log.
  - The clip the original started is the best match at 5 starts: HighAccel0 at 9.74 s, Power1 at
    11.64 s, LowAccel0 at 16.91 s, Power0 at 22.98 s (score 0.46) and HighAccel4 at 26.92 s. Those
    five are every loud on-gas start.
  - Under P42's pairing, the clip it would have played is never the best match, and ranks 8 and 14 at
    Power1 and Power0.
  - The off-gas starts (Decel, Idle) and the two LowAccel1 starts are under the music and rank 3rd to
    17th either way. See below.

## Not verified / not 1:1

- **By ear.** An agent cannot listen. The match is shown by the note timeline (1199/1199), by sample
  identity (the lengths, and the cross-correlation at the loud starts) and by the spectrograms'
  on-gas spans. Nobody has heard `port.wav` beside `original.wav`.
- **The spectrograms cannot be compared level for level.** The original's capture has the game's music
  and world sounds under the engine; the run did not turn the music off (menu navigation on a pad
  over the state was not attempted). Engine loudness against the original's mix, the engine gain
  (0.7) and its balance with the music are unverified.
- **The quiet starts are not identified acoustically.** Decel and Idle after the throttle, and
  LowAccel1, do not stand out of the music. Their identity rests on the length match, which is exact
  for all 19.
- **The 3-D mix is not the original's.**
  - The port uses its own mixer's distance gain and pan, with the file's min and max distance.
  - The original's integer volume falloff (`100 - (int)(...)`, `sound_3d_mix` in oracle.txt), its pan
    curve and its dead zone are read but not ported.
  - The original refreshes the 3-D position through the emitter update every frame; the port sets
    the voice's place every frame. Not compared.
- **Doppler.** Ported from the decompilation: the sign (only a closing bike is raised) and the
  units mix. Never seen in a recording, because the replay held the bike and listener still and the
  original's camera rides with the bike. Whether the Doppler pitch overwrites or multiplies the
  detune in the original is not settled; the port multiplies.
- **30 Hz versus 60 Hz.** The original ran the note at the game's 30 Hz in the capture. The oracle
  covers dt 1/60, 1/30 and others, but the shell calls `Engine::update` at its own frame rate, so
  OverlapTime and the air timer resolve at the shell's frame, not the original's.
- **The limiter's pause** (sound vt `+0x24` on Panic[0]) is ported as a pause of that voice. The SPU
  semantics of a paused voice's position are not checked against the original.
- **Other bikes.** Only the 125 (`first_recording`) was captured and length-checked against the
  original. The 250's and 450's lists go through the same parser and are trusted on that. Of the 50cc
  and the other bikes, only RevMidpoint was read.
- **No gear-shift cue exists to verify** (see above). Landing cues are P42's, not re-checked here.
- **The original's own `rand` draws for picks are not the port's.** By design (AGENT.md: audio never
  touches the sim's generator). So the port's picks differ from the original's sample for sample;
  the lists, the no-repeat rule and the jitter law are the same, and the oracle feeds both the same
  draws.
- **P42's other sounds** (wrecks, landings, crowd, world emitters, music) now read the right files.
  None of them was re-listened or re-checked here beyond the two P42 tests updated above.

## Next

Done, unless a human listening to `context/p49/port.wav` against `original.wav` hears a difference.
If so, the first candidate is the engine gain against the music.
