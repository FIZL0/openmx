# 1 — 2p playback parts (FINAL)

Task C12. 2026-10-27.

**Result.**
- The user's new tape `context/recordings/split/truck.p2m2` plays back as their ride: P1 rides the
  moving truck (path car 3) from 132089 to 132894.
- Recaptured C10's way (`watch/2p.toml`, NominalScalar 0.05) as `context/capture/2p/truck.csv` and
  re-indexed: `context/capture/2p/truck.index.md` lists the truck ride. Film: `context/c12/truck.mp4`.
  All three are in s1 only.
- Why the old `2p.p2m2` parts from its ride (~40 s in) is still not found. What was ruled out is below.

## Rulings relayed

Three messages from another session (openmx-4b), relaying the user, 2026-10-27:

- "i definitely had the 60fps patch" — the tape was recorded with PCSX2's 60 FPS patch on. The RAM
  agrees (below).
- Stop the settings sweep. The user thinks some respawns put the riders back in a different place, so
  the playback parts after a respawn. They are recording a new 2p tape with a truck ride before any
  wreck (named `2p-truck.p2m2`, main checkout). Play that one C10's way once it is there, and block on
  it until then.
- The tape landed as `context/recordings/split/truck.p2m2` (+ state), not `2p-truck.p2m2`. Use that
  name, and go on with the Next steps on it.

## What I did

All runs on kit copy 1 (v2.9.127, `[Pad2] Type = DualShock2` set in the copy's ini first: it said None),
played by `research/c12/play_variant.sh` (C10's play_capture.sh with any tape, an optional film and C10's
36-column `research/c10/check.toml`). Every compare is `research/c10/compare.py 2p c12/<run>`. Captures are
in s1's `context/capture/2p/c12/`.

- **The input path is exact.** `research/c12/padbytes.py 2p`: on every row of C10's capture, both ports'
  polled libpad buffers (buttons, sticks and all 12 pressure bytes) equal one tape field byte for byte
  (field tick−2..tick, the read phase). No row matches none.
- **The tape's first field.** Field 0 is all zero bytes (to the game: every button held, both sticks
  hard up-left). `research/c12/variant.py` builds variants:
  - `zero0` (field 0 made neutral) rides the control's ride, so the game never reads field 0.
  - `drop1` (every field a tick earlier) parts at 128389.
  - `delay1` (a tick later) parts at 127915, because the shift moves the zero field to where the game
    reads it.
  So the .p2s sits on the tape's first frame, and the C10 alignment (offset 0/−1) is right.
- **Save → load is lossless.** The user's session recorded on from the live VM; a playback starts with a
  load. `research/c12/roundtrip.sh` saved and loaded straight back at 128458 mid-playback (the movie
  plays on). The next 1,500 ticks ride the uninterrupted ride: same wreck tick, centres within
  read-phase noise.
- **Filmed** the plain playback with PCSX2 Video Capture: `context/c12/baseline.mp4` (215.8 s; s1 only)
  beside `context/capture/2p/c12/baseline.csv`. That capture rides C10's ride again: same 17 wrecks,
  closest rider-car 183.8 ft, no wheel on a car. `research/c12/still.sh NAME SECONDS OUT` pulls a still.
- **The 60 FPS patch.** PCSX2 ships one for this disc (patches.zip, `SLUS-20767_9866FE54.pnach`, "60
  FPS", "Might need EE Overclock"). It writes `li a0,1` (60 fps) into the frame limiter's argument at
  0x19210c, and writes `li a0,2` (30 fps) back while the halfword at 0x425da8 is 1. That flag is the
  IOP read-in-flight flag (`./kit xref 0x425da8`, `./kit fn 0x41a260`).
  - `research/c12/bios.py`-style reads of the states: both v2.9.52 states (2p and animandpump) hold the
    patched `li a0,1`; every v2.9.127 state holds the stock `li a0,2`. That is why the 2p state ticks
    at 60 Hz.
  - With the patch enabled (copy gamesettings `[Patches] Enable = 60 FPS`, "Enabled patch: 60 FPS" in
    the emulog), run `p1r0` rides the same ride as without it.
  - `research/c12/stream.toml` (2,700 ticks with the patch on, through P1's first two wrecks): the read
    flag is 0 on every row, the limiter word stays `li a0,1`, dt stays 1/60. The patch's 30 fps branch
    never fires, so it cannot part the ride.
- **EE cycle rate** (`research/c12/sweep.sh NAME PATCH RATE SKIP`, patch on):
  - +1 parts at 128679 (P2) and 128998 (P1); closest rider-car 37.5 ft, no wheel on a car.
  - +2 parts at ~128834; no car within 258 ft (4,444 of its first ticks).
  - So the cycle rate changes the ride: the game's dt is a 30-frame mean of a millisecond clock sampled
    at a point in the frame that depends on EE speed (S1's journal). But neither overclock is the user's
    ride (anchors below).
  - Stopped there on the relayed ruling. +3 and the rate without the patch were never run.
- **BIOS.** `research/c12/bios.py`: the 2p state's EE kernel code is the same as a kit state's made on
  SCPH-70012. Only data words differ: 60 + 275 + 1 words of 0x20000. So most likely the same BIOS.
- **Where the playback parts, from the human's own reactions** (`research/c12/idle.py`,
  `research/c12/anchors.py`). A human lets go of the pad after their own crash. The tape's let-go moments
  (45+ fields with nothing held) are the user's crashes, whatever a playback rides.
  - The baseline's P1 nose-dive at 129945 is followed 26 ticks later by a let-go (129971). No other
    baseline wreck of either rider gets one, and no later let-go follows a baseline wreck.
  - P2 goes idle at 130312 while the baseline has P2 still in a long drop (no crash). P1 goes idle at
    130385 with no baseline crash either.
  - Earlier on, P2's tape holds triangle + right for 1.5 s exactly while the baseline P2 is in that drop
    (a trick in the air), so the baseline still looks like the user's ride there.
  - So the playback is the user's ride through P1's first respawn (128995 → put back ~129061) and its
    second wreck (129945). It parts from it between ~130000 and ~130300, for both riders at about the
    same time. Both at once suggests something global, not chaos in one bike. The riders never come
    within 137 ft of each other.
  - The +1/+2 overclock runs miss the 129945 anchor, so they part from the user's ride earlier than the
    baseline does.
  - Note on the relayed lead: compare.py's "first > 5 ft at 129061" is between two playbacks on the same
    settings. It is the put-back teleport read a tick apart (C10 saw the same), not a parting from the
    user's ride.

- **The truck tape.** `truck.p2m2` is 1,802 fields (v2.9.52, author ryha), and its state ticks at 60 Hz.
  - Pad 2 is neutral on every field, so P2 stands at the start the whole time. Its body goes to mode 4,
    the idle look-around, at 132877..132996.
  - `research/c12/fetch_truck.py` copied the tape and state into s1. `research/c12/state_ram.py truck`
    extracts the state's EE RAM to `context/states/c12/truck-start.ram`.
  - `research/c10/records.py c12/truck-start` finds the same six car, two biplane and two rider records
    at the same addresses as `c10/2p-start`. So `watch/2p.toml`'s fixed addresses hold for this state
    unchanged.
  - The state's word at 0x19210c is `li a0,1` (`./kit pine read`): recorded with the 60 FPS patch
    again. Loaded live at frame_counter ~131435.
  - First run: `STATE=…/truck.p2m2_SaveState.p2s FIRST=131428 VIDEO=…/context/c12/truck.mp4
    research/c12/play_variant.sh` (check.toml spec, 1,900 rows). P1's wheel path-car word is 0x1b93ef0
    (car 3) on 662 rows, 132086..132893. Stills at 15 s and 22 s of the film show P1 on the flatbed's
    deck with the road beside it.
  - Recapture: `research/c10/play_capture.sh PID context/recordings/split/truck.p2m2 131430 1801
    context/capture/2p/truck.csv` at NominalScalar 0.05. That gave 1,801 rows, 131436..133236, no gap,
    no guard miss.
  - `research/c10/index.py truck`. It needed one guard for a rider with no touch rows (P2 never
    leaves the ground): its near-miss table now prints "(no row)" there.
    - "Truck rides": P1 on car 3, 132089..132894, car speed 45.3–45.6, front and rear wheels, stays on.
      The table has 101 lines for that one ride. At 0.05 the wheel's contact flag, path-car word and car
      velocity read 0 on about every other row (`research/c12/deck.py truck 132086 132893`: 236 of 808
      rows name the car, e.g. 132291/93/95/97). The 1.0 run of check.toml reads the same address
      non-zero on 662 rows in a row. So that is the row's read phase against the wheel step, not the
      bike leaving the deck: the film shows it on the deck throughout.
    - P1 reverses on the deck four times (132493..132670).
    - No wreck, no rider-vs-rider contact (closest 804 ft), and two scenery contacts at 132862/132869
      as P1 comes off.
  - `compare.py truck c12/truck`: the two playbacks agree within 2.24 ft (read phase) on both riders.

## What the original does

- The game never reads the tape's field 0. Each tick reads one field: field t at frame_counter
  127882 + t (offset 0/−1).
- The 2p state runs 60 Hz only because the recording session's 60 FPS patch had already written
  `li a0,1` into RAM, which the .p2s carries. The patch's 30 fps fallback is keyed on the IOP
  read-in-flight flag, and that flag stays clear through the first 45 s of this ride.
- The respawn (oracle.txt S5: `ride_reset_request`, `player_respawn`, `bike_unwreck`) takes nothing
  from the pad except the hand-reset chord. Its clock is dt-driven; `rand()` is drawn at the wreck
  entry only for reason 13. The put-back position code was not read in this task.

## How it was proved

Live playbacks on the original with PINE captures, every one compared row by row with C10's capture of
record or a control run:

| run | what changed | ticks |
|---|---|---|
| same-1500 (control), zero0, drop1, delay1 | the tape | 1,500 each |
| rt-load | save/load round trip | 1,500 |
| baseline (filmed) | nothing | 12,917 |
| p1r0 | the 60 FPS patch | 12,158 |
| p1r1 | patch + EE +1 | 12,916 |
| p1r2 | patch + EE +2 | 4,444 |
| stream | patch, read flag / limiter / dt | 2,700 |
| c12/truck (filmed) | the truck tape, check.toml, speed 1.0 | 1,900 |
| truck (of record) | the truck tape, watch/2p.toml, speed 0.05 | 1,801 |

Byte-exact pad comparison over all 12,917 rows of C10's capture.

## Not verified / not 1:1

- **The cause is not found.** The playback parts from the user's ride around 130000–130300, for both
  riders. Nothing tried here moves it: the tape's offset, the save/load, the 60 FPS patch, the BIOS.
- **The let-go anchors are an inference** that a human releases the pad after their own crash. Only one
  anchor (129971 after the 129945 wreck) ties the playback to the user's ride. The "parts between
  130000 and 130300" bound rests on two missed anchors (P2 130312, P1 130385). It was never seen on the
  user's screen.
- **The recording machine's settings are unknown.** It ran v2.9.52, not this box: pcsx2 here has been
  2.9.127 since 10-15. The user's ~/.config ini is all defaults, but that is this machine's.
  - Not tried: EE cycle rate +3, −1..−3, cycle skip, fast CDVD, MTVU/instant VU1 off, the
    NoInterlacing/widescreen patches. The sweep was stopped on the relayed ruling.
- **The respawn lead was not settled.** It was checked only so far: the respawn reads no pad, and the
  60 FPS read flag never fires. What the put-back position depends on was not read. The anchor
  evidence says P1's first respawn (~129061) did not part the ride.
- **The BIOS match is by the EE kernel's code only.** The IOP ROM is not in a .p2s, so it was not
  compared.
- **The +2 run lost frames** (4,444 of 5,672 in its window) before it was stopped. It is used only to
  say it parts early.
- **"The user's ride" for truck.p2m2 is judged by the film and the tape.** P1 reaches the truck and
  rides it with no wreck, which is what the user said they recorded. It was not checked frame by frame
  against the user's own screen.
  - The tape is short (30 s, no wreck before the truck), so the parting seen in 2p.p2m2 at ~40 s
    would not show here even if its cause is still there.
- **The truck tape has no rider-vs-rider contact.** P2 never moves, closest 804 ft, so S158 still has
  nothing to replay. S159 (truck rides) does.
- **The truck capture lives in s1 only** (context/ is per worktree). A task that reads it must copy it
  (or rerun the recapture below) into its own slot.
- **No test reads truck.csv.** `crates/game-sim/tests/two_p.rs` still reads 2p.csv. Wiring the
  truck capture into tests is S159's.

## Next

- S159: use `context/capture/2p/truck.csv`, P1 on car 3 at 132089..132894. To get it into another
  slot:
  - `python3 -B research/c12/fetch_truck.py` (from that slot's root).
  - Then, under `./kit pcsx2 with --` on a copy with `[Pad2] Type = DualShock2` and NominalScalar 0.05,
    run `research/c10/play_capture.sh PID context/recordings/split/truck.p2m2 131430 1801
    context/capture/2p/truck.csv`.
  - Then `research/c10/index.py truck`.
  - Read the deck words with the read phase in mind (above).
- S158 needs a new tape where the riders touch (asked in plan/HUMAN.md).
- The old 2p.p2m2's parting at ~130000–130300 is still unexplained. Untried leads: the recording
  machine's settings (asked in plan/HUMAN.md), EE cycle rate +3 / underclock / cycle skip, and what the
  game reads at ~130000–130300 that is not the pad (e.g. a timer read late in a frame). Short tapes like
  truck.p2m2 avoid it in practice.
