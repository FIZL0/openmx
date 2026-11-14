# 1 — zone open flags (FINAL)

Task S315. 2026-11-14.

## What I did

- `ai::seek::open_flags` (crates/game-sim/src/ai.rs): a free-ride start zone's flag byte for one player
  at the map's load, from the player's map record in the save profile (`ai::seek::Progress`, the record's
  bytes) and three session facts (`ai::seek::Opening`: held, a rider in a hit progression, the race
  live). `ai::seek::opener` turns it into the `open(player, zone)` answer `ai::seek::load` (S313) takes:
  open to the session's human players whose byte has bit 0. The flag bits are named `ZONE_OPEN` (1),
  `ZONE_SHOWN` (2), `ZONE_PLACED` (4).
- Corrected `ai_seek_zone_open` in research/oracle.txt (it had the bit values a power off: the opening
  sets 1 and 4, and 2 is the shown bit) and named the opening's pieces (`ai_zone_open_players`,
  `ai_zone_open_stretch`, `_end`, `_skip`, `ai_zone_hit_running`).
- Tests: crates/game-sim/tests/ai/zone_open.rs (new topic module).
- The app (crates/openmx/src/main.rs) still opens the first zone of each progression to every player:
  it keeps no save profile to read a map record from. Filed as a follow-up (below), not changed here.
- opentennis has no counterpart (no free-ride progression); nothing taken.

## What the original does

The opening is a vtable-only function (`./kit dis 0x00330bc0 0x00331150`; `./kit fn` refuses it). Per
player of the activity (its owner object; kind 2, an AI rider, is skipped), after some per-player set-up:

- the profile is the save profile's map record whose name is the event's track (`./kit fn 0x0036c2e0`,
  a case-blind name compare over the record list);
- if the rider manager (`*(0x004e2778)`) has its +0x58 word set, `./kit fn 0x003aa460` gives every zone
  bit 4 for the player (player < 2 only) and nothing opens; with no profile record, nothing either;
- otherwise, per manager zone record (+0x40, 12 bytes: place, number, type — equal to the zones' +0x70,
  +0x74, +0x78 in all three images) and zone i of the list:
  - Hit (1): the record byte at +0x20+place, bit 0, and `./kit fn 0x003aa780` answering 0 (no rider's
    owner answers "HitProgression") → open; shown = byte bit 1;
  - Run (2): byte at +0x25+place, bit 0 → open; shown = bit 1;
  - Race (3): the activity manager's live activity 8 (`*(*(*(0x004e1df8)+0x58)+0x20)`) has a non-zero
    +0x14 (its list count; 1 on foot, 0 in the biplane) → open; shown = the record's word +0x14;
  - Stunt (4): always open; shown = the record's word +0x18;
  - open: byte |= 1, |= 4, then bit 2 set when shown, cleared when not.
- The place indexes the record without a bound: place -1 (a zone its progression list does not name)
  reads the byte before the first.

## How it was proved

- **Oracle, 10,000 cases** (`s315_zone_open_flags_are_the_originals`): the original's code from the held
  test (0x00330ec0) to the end of its zone loop runs in the interpreter on recording2's RAM image, with
  random bytes in the save's free09 record (+0x14..+0x30, every byte places -1..4 reach), random zone records
  (place -1..4, type 0..5), the held word, the race activity's count, the owner's player (0 or 1), the
  hit-progression answer (that check hooked) and, 1 in 16, no profile. All 12 zones' two bytes, from 0,
  equal `open_flags` for the owner's player and 0 for the other.
- **Save states** (`s315_free_ride_images_hold_the_ports_zone_bytes`): recording2 and first_recording
  (free09) and biplane (free06, same 5 hit / 5 run / race / stunt layout) — each image's zone bytes
  equal `open_flags` of the save's record for its own track and its session, and
  `opener`'s answer equals bit 0. (The first of each progression opened, 5 = open|placed; biplane's race
  closed.)

## Not verified / not 1:1

- **The held word (rider manager +0x58)**: what sets it is not traced; 0 in all three images. Its
  branch is oracled, but no image shows it set, so when the game holds the zones is unknown.
- **"HitProgression"**: the check is hooked in the oracle, not run; which rider answers it and when is
  not traced (no image has it). It is an input (`Opening::hit_running`).
- **The race activity's count**: read as "the race is live"; which activity object 8 is (its list holds
  the rideable machines?) is not traced beyond the two images (1 on foot, 0 in the biplane). Input
  `Opening::race_live`.
- **Bytes past the record**: the original reads past the map record for a large place; the port reads
  0 there. Places in every disc progression list are 0..4, inside the record.
- **Players 2 and over**: the original writes the byte at `bytes + player` without a bound for a human
  of player number ≥ 2 (out of the zone's two bytes); the port's `open_flags` answers that player's byte
  but `load` only keeps two. Not reachable in a free ride (one human, player 0).
- **The save profile itself**: the port has no reader for the save's map records; `Progress` is the
  record's bytes as RAM holds them. The app does not use this yet (follow-up filed).
- **Zone records = the zones' place/number/type**: the writer of the manager's +0x40 records was not
  found; they equal the zones' fields in all three images, and the port takes the zone's.
- The three images are all the same save (only place 0 of each progression unlocked, nothing shown):
  the shown bits and later places are proved by the oracle only.

## Next

P348 (filed, low): the app keeps a per-track progression and opens zones through `ai::seek::opener`.
