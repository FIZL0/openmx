# 1 — split screen captures (FINAL)

Task S93. 2026-10-21.

## What I did

- Copied the user's two split-screen recordings (`splitscreen.p2m2`, `splitscreen2_shortcolision.p2m2`, each with
  its `_SaveState.p2s`) into this slot's `context/recordings/split/` (a subfolder: replay.rs reads every .p2m2 straight in context/recordings and a two-pad tape doubles its counts). They are played only as `chmod u+w` copies in a
  mktemp dir, because PCSX2 records over a movie it plays.
- Wrote a two-rider recorder spec, `watch/split.toml`. Each player gets:
  - the ridden vehicle (actor type 2): player index, control source, wreck reason and state, ground and air
    flags, joust velocity;
  - its chassis: centre, force, moment, shock, twist, velocity and rates;
  - its collision record: count, the words a mirrored hit raises, and all 11 entries (kind, ray end, normal,
    point, other record);
  - the bike actor (type 4): player, velocity and the 4×4 transform.
  
  Both pads are read too: the raw buffers and stamps of ports 0 and 1, plus both control-state arrays. The
  header comment of the spec explains how the actor list is walked.
- Captured both playbacks frame by frame with `research/s93/play_capture.sh`. This is C6's AT-SPI recipe
  (Stop, Reset, recorder on a `start` trigger, Play the copy, resume) with this spec:
  - `context/capture/split/splitscreen.csv`: 2444 rows, frames 3113..5556. That is every tick of the 4887-field
    tape.
  - `context/capture/split/splitscreen2.csv`: 388 rows, frames 5765..6152 (775 fields).
  - Neither has a gap or a guard miss. Every row has P1 on player 0 and P2 on player 1, both local pads, and
    dt 0.03333 throughout.
- Saved a state at the tick before each contact with `research/s93/play_at.sh` + `at_ticks.py`. They play the
  copy again with no recorder, poll `@frame_counter` and send PINE save-to-slot. The emulator ran at
  NominalScalar 0.25 so each save lands inside the tick it was asked in. Each slot was then turned into a RAM
  image with `./kit state ram`, renamed in `context/states/`:

  | image | its `@frame_counter` | before |
  |---|---|---|
  | `s93-long-3402.ram` | 3402 | rider-rider, long ride, 3403 |
  | `s93-long-4661.ram` | 4661 | P1 into the biplane, 4662 |
  | `s93-long-5074.ram` | 5074 | P2 into the biplane, 5075 |
  | `s93-long-5505.ram` | 5505 | P2 into the biplane, 5506 |
  | `s93-short-5997.ram` | 5997 | rider-rider, short ride, 5998 |

  The contact at 3414 is a slow slide into an already-wrecked rider, and its RAM image is 3402's ride
  11 ticks on. I took no separate image for it (see Not verified).
  Also `splitscreen.ram` / `splitscreen2_shortcolision.ram` are the two start states.
- Took screenshots `context/shots/s93-long-3402.png` and `s93-short-5997.png`. Each was taken about 2 s at
  0.25 speed after loading the saved state, with idle pads, so they show the contact a few ticks on, not the
  user's inputs.
- Wrote `crates/game-sim/tests/split.rs`. Its reader loads each capture by column name. Two tests:
  - `both_rides_are_whole_with_both_riders_lanes`: row counts, contiguous frames, dt, both riders' guards on
    every row, a `p2_` twin for every `p1_` column, both pads' lanes. P2's pad is live in both rides; P1's only
    in the long one, because the short tape leaves port 0 idle.
  - `the_named_contacts_are_at_their_ticks`: each rider's contact onsets, which record each names, the
    striker's and the struck's entry kinds and speeds, and every wreck onset with its reason.
  
  To check the tests run and don't skip, I made the row count wrong on purpose once and the gate failed on it.
  The count was set back after.
- Helpers in `research/s93/`:
  - `contacts.py`: continuity, guards, and what changes in the hit lists;
  - `detail.py`: both riders per row;
  - `actors.py`, `peek.py`, `find.py`, `strings.py`, `records.py`: RAM-image walkers used to find the layout.

## What the original does

### Where both riders live

`@game_manager -> 0x34 -> 0x34` is the head of the actor list (nodes `{item +0, next +8}`). Both start states
have the same six entries:

- 0 and 1: the two biplanes (type 1, `Biplane\biplane.phy`; collision record at plane +0x170).
- 2 and 3: P1's and P2's ridden vehicles (type 2; the player record at +0x20 has the index at +0x30 and the
  control source at +0x34).
- 4 and 5: P1's and P2's bike actors (type 4).

The vehicle's collision record is the pointer at +0x7d0, not the vehicle itself (vtable 0x00454a00 at record
+8). The list is at +0x108 (count +0x100, capacity +0x104 = 11), with 0x80 bytes per entry:

- +0 kind: 0 is the rider's own ray, 1 is another actor's ray mirrored in;
- +0x20 ray end (local);
- +0x30 normal;
- +0x40 point;
- +0x74 the record on the other side (0 is scenery).

The records sit at the same addresses in both rides:

- P1 0x013311a0, P2 0x012a3420;
- the wreck bikes' records: P1 0x01333240, P2 0x012a4b30;
- biplane 0 0x0060b1f0.

Pads: port 1 is `@pad_buffers` +0x400 / +0x480, stamps +0x458 / +0x4d8, and its control state is
`@pad1_control_state` (0x679768).

**The count does not say whether a contact happened.** When a hit wrecks the rider, the count reads 0 in that
same row while the entries hold the new hit (4662, 5075, 5506). A wrecked rider's record is also not refreshed
(frozen) until the put-back. So the tests count a contact as an entry naming an actor whose bytes changed
since the row before.

### The models

Both players' physics file is `Bike\Race125P.phy`, read at vehicle +0x84 in both states. So in physics terms
the "different bike" is the same 125 Race bike. What differs is how it looks:

- **P1:** jersey #62 in grey and black with a name across the back I can't read at this resolution, white
  helmet. A red bike with a black number plate and red/white plastics.
- **P2:** jersey #002 in red, orange and white. A red bike with blue/white/red graphics.

The bike textures named in RAM are CR-125 plastics sets. I could not tell which pro rider P1 is: the jersey
name is unreadable and the rider texture strings in RAM are the whole catalogue (see Not verified).

### The contacts

All the kinds are T-bones or head-ons. None of them is a side swipe, and no rider lands on another.

| ride | tick | who | kind | what the original did |
|---|---|---|---|---|
| long | **3403** | P1 → P2 | **T-bone.** P1, riding west at 57.4 u/s, puts its front rays (kind 0, normal +x) into P2, who stands at 0.3 u/s facing north. P2's rear rays (end z −3.8) touch P1 in the same tick, and each record holds the other's mirror (kind 1). The top ray is 3.64 high, under the 6.67 head ray, so this is not a joust. | **Push apart, impulse shared:** P1 drops to 27.2 u/s, P2 is thrown to 28.7 u/s westward in the contact row. **P1 wrecks, reason 1** (Front, over the bars), one row later (3404). P2 stays up and slides to rest by about 3413. Riders put back at 3461. |
| long | **3414** | P2 → wrecked P1 | **Slow rub.** P2, at 1.8 u/s, touches P1's frozen record (P2's own rays kind 0, P1 gets a kind-1 mirror) and P1's wreck bike (0x01333240). The bike contact runs to 3417. | **Nothing:** no push worth naming and no wreck. |
| long | **4662** | P1 → biplane 0 | **Head-on into the parked biplane.** P1 is riding east at 85 u/s; its front rays hit a surface with normal (−0.99, 0.17, 0) facing back at it. | **P1 wrecks, reason 1,** in the contact row; its velocity falls from 85 to 3.7 forward + 12.6 up. Count reads 0 (above). |
| long | **5075** | P2 → biplane 0 | **Head-on.** P2 is riding east at 64 u/s; one own ray (kind 0) and two mirrored (kind 1) entries against the plane. | **P2 wrecks, reason 1,** in the contact row. |
| long | **5506** | P2 → biplane 0 | **Head-on.** P2 at about 73 u/s, heading 84°, front rays into the plane (normal −x). The plane's mirrored hit is in the next row (5507). | **P2 wrecks, reason 1,** in the contact row. |
| short | **5998** | P2 → P1 | **T-bone.** P2, at 59.9 u/s from the south heading −33°, puts its own rays (kind 0, normal (0.07, 0, −1)) into P1, who stands still facing east. P1 holds only the mirror (kind 1). From 5999 P1 also hits P2's wreck bike (0x012a4b30) as it tumbles past. | **Push apart:** P1 is thrown to 26.2 u/s north in the contact row and stays up. **P2 wrecks, reason 1,** in the same row. Riders put back about 6083. |

The long ride also has one wreck with no actor in it: at 4081 P1 wrecks with reason 8 (nose dive). Every wreck
onset in the two rides is in the test: long P1 (3404, 1), (4081, 8), (4662, 1); long P2 (5075, 1), (5506, 1);
short P2 (5998, 1). In the short ride, P1 never wrecks.

`opentennis` has nothing for this task: it is not a split-screen bike game.

## How it was proved

- **Save-state capture over PINE:** two recorder runs of the user's tapes, 2444 + 388 rows of both riders, no
  gaps and no guard misses (`research/s93/contacts.py`).
- **Test:** `crates/game-sim/tests/split.rs`, two tests over the two captures, run through `./kit check`. A
  deliberately wrong count failed once, which proves they are not skipping.
- **RAM images:** each image's own `@frame_counter` was read and equals the tick asked for. In all five, both
  riders' chassis centres (walked from the image's actor list) equal that frame's capture row to the last
  digit. So the replays that saved them rode the same ride as the capture, and a row holds the state a save
  at its frame holds.
- **Screenshots:** the models come only from the two screenshots.

## Not verified / not 1:1

- **The pro rider and the bike models are not named.** I only see #62 grey/black (P1) and #002 red/orange (P2).
  The jersey name is unreadable at 642 px a pane, and the RAM strings I searched are the whole texture
  catalogue, not what each player loaded. Both players' physics file is the same `Race125P.phy`, so S86's
  "different bike" is at most a different look or collidable, not proved either way.
- **Which wreck clause fired is inferred.** I read the wreck reason (1 = Front, 8 = nose dive) from the record;
  I did not trace the decision through the code.
- **Contact kinds are named from the capture geometry** (headings, normals, which rays hit), not from any
  label in the game. The brief's "side swipe" and "landing on" don't occur in these rides: both rider-rider
  hits are T-bones.
- **The biplane is identified by record address only.** 0x0060b1f0 is biplane 0's record (plane +0x170 in
  the start state's actor list). The plane's own state is not in the capture, so whether it moved is unknown.
- **The rows are read at one of three phases.** In the short ride, the wreck reason shows in the contact row;
  in the long ride at 3403 it shows one row later (3404). This matches the known three phases of capture rows
  (memory: capture-rows-read-at-three-phases), not something proved about the game. The test pins the ticks
  as captured.
- **No RAM image just before 3414.** That slow rub follows the 3403 crash at under 2 u/s and is not a hit S88
  needs to start from. If it does, play the tape again with `at_ticks.py 3413:save:N`.
- **What the screenshots show:** they were taken a couple of seconds after loading each saved state with idle
  pads, so they show the riders, not the exact contact frame.
- **Emulator config outside the repo:** copy 4's `PCSX2.ini` had `[Pad2] Type = None`, which made the first
  short capture all idle. I set it to DualShock2. NominalScalar was 0.25 for the save runs and is set back to 1.0.
  Pad2 stays DualShock2 there; a follow-up is filed for `./kit pcsx2 copy`.

## Next

Done when `./kit done` lands. S86 and S88 should read their start points from the RAM images in the table
above, and the record and pad-1 layout from this journal.
