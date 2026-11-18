# 1 — zone progression (FINAL)

Task P349. 2026-11-18.

## What I did

- New `crates/openmx/src/track_progress.rs`: the save profile's map record (S315's `Progress`) kept per
  track and profile beside `settings.txt`, `progress/<track>/<profile>.progress` (a format line, then the
  record's bytes in hex). No file: a fresh profile's record (first hit and first run place unlocked,
  nothing shown — what recording2, first_recording and biplane hold, S315), written so it is kept. A bad
  file is said and read as fresh, left alone. Same layout and write-by-rename as P99's `saved_markers.rs`.
- `main.rs`: the free-ride seek load (S316) opens zones through `ai::seek::opener` instead of the
  place-0 closure. The humans are the sim's players (`0..players`). `Opening`: not held, no hit
  progression running, `race_live` = the first seat starts on the bike (off on any other machine).
- The record comes from the file only on a local ride. A played tape, `--online` and a lobby ride use the
  fresh record, so every machine and every replay builds the same seeker world.
- Profile: the first player's rider name, or `1P` (P99's `saved_markers::profile`), one record for all
  humans — the original reads one save profile.
- Took nothing from ../opentennis (no free-ride zones there).

## What the original does

S315's journal: each zone's open byte per player from the profile's map record at the map's load.
Nothing new read here.

## How it was proved

- Unit tests in `track_progress.rs` (`./kit check -- -p openmx --bin openmx track_progress`): the fresh
  record opens exactly what the old closure opened on a bike (hit/run place 0, race, stunt; not place 1,
  not no-place, not player 1); on a machine the race zone closes; the file round-trips; bad files refused;
  names stay inside the folder.
- A headless free09 ride (`--headless --frames 5`): `ai seek: 106 targets, 12 zones, 1 riders, 1 slots`,
  and `progress/free09/1P.progress` written with bytes +0x20 and +0x25 = 1 (removed after).
- `open_flags` / `opener` themselves are S315's, oracle-proved there.

## Not verified / not 1:1

- **Nothing in the app moves the record yet**: no hit or run completion unlocks a place or sets a shown
  bit, so every kept file stays the fresh record. The file is there for that work; not filed as its own
  task because hit/run completion in the free ride does not exist in the app yet.
- **The zone opens change nothing played today**: the seeker only reads a group's open flag for an owner
  that answers "HitFreeride", which the app never sets (`load_seek` sets `hit_freeride: false`). So the
  switch is not visible in any test or ride; the opens were checked by the unit test, not on screen.
- **`race_live` = "the first seat starts on the bike"**: S315 read the race activity's count as 1 on the
  bike, 0 in the biplane, from two images; which object it is was not traced. Read at the load only; a
  switch of machine mid-ride does not reopen or close the race zone (the original's is also a load-time
  write, as far as S315 traced).
- **One record for all humans**: a second local player opens zones by the first player's record. The
  original has one profile; split-screen in the original was not checked.
- **Online and tapes use the fresh record**, not the player's kept one, so the world is the same on every
  machine. If zone opens ever reach the sim, the pick would need to carry the record (like P105's markers).
- **`held` and `hit_running` are always false**: the app has neither (S315: what sets them is not traced).

## Next

Done; finish with `./kit done`.
