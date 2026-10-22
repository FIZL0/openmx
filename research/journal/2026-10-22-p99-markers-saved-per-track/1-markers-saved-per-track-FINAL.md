# 1 — markers saved per track (FINAL)

Task P99. 2026-10-22.

## What I did

- **The file.** `Markers::to_file`/`from_file`/`read_file` in `game-sim/src/markers.rs` produce and read this
  layout: magic `OMXmarks`, a u32 version (`FILE_VERSION = 1`), the bike's `.phy` word, then the five slots as
  `snapshot::encode_slots`. That function is new in snapshot.rs: `OMX1`, then each slot's full `Ride` through
  the snapshot codec's `Wire`. A wrong magic, version or bike, a cut file or trailing bytes gives an `Err`.
  The shell prints why and starts the seat empty. Nothing panics. One slot is 2430 bytes and five are 12018.
- **Where.** `crates/openmx/src/saved_markers.rs` decides where files go: `<settings dir>/markers/<track>/<profile>.markers`.
  The profile is the rider's name (cleaned as the garage cleans it), or `1P`/`2P`/.. when there is none. File
  names keep only letters, digits and `-`. A save writes a `.part` file and renames it over the real one.
  Clearing every slot removes the file.
- **When.** At ride start each local seat loads its file. After each frame `Kept::frame` compares a signature
  of the slots (`hash::of` each Ride, without the selection) with the last one written and saves if it changed.
  So placing or clearing a slot writes the file; stepping through slots does not.
- **Tapes.** `PadTape` has two new fields. `markers` holds the start file's bytes and is written as a
  `markers <hex>` line only when set. `cleared` lists the frames where the pause cleared them, written as
  `markers_cleared <k>` lines. `play_from` starts from the tape's markers and clears them at those frames.
  It never reads the files. `--play` neither reads nor writes any file. An old tape has no line and starts empty,
  as it did before.
- **Online.** `openmx_net::Pick` has a new field, `markers` (`#[serde(default)]`). It holds
  `"<track>:<hex of the file>"`. In the lobby, the front end fills it every frame from 1P's file for the room's
  track and bike, using `PickMarkers`, which reads the file again only when the key changes or after a ride.
  `Lobby::set_markers` sends a changed pick the same way the controller preset does. When a lobby ride
  starts, each room seat's sim gets `from_pick(pick.markers, pick.bike, room.track)`. That is the same bytes
  and the same build on every machine, so every machine starts with the same slots. A pick for another track
  or another bike, or one that doesn't decode, starts the seat empty on every machine.
- **Pause.** The pause has a new row, "Clear Markers", below Main Menu (placed after the old rows so
  their indices don't move). It produces `Act::ClearMarkers(pauser)`. Offline it clears the seat's slots,
  removes the file, and records `markers_cleared` on a recording. Online it removes the file only: the ride
  keeps its markers, because a local clear would desync the other machines. Under `--play` the row does nothing.
- `tests/menu.rs`: the two `Knob::ALL` → `Knob::MENU` lines from the modifiers pause test, which was red on
  this branch's base. This is the same change P94 made on main, so the merge sees the same lines on both sides.

## What the original does

Nothing. Session markers are the port's own feature, so there is no oracle.

## How it was proved

`crates/openmx/tests/markers_saved.rs`, 5 tests, run through `./kit check -- -p openmx --test markers_saved`:
- a marker placed and kept, the ride ended and a new sim started on the same track loads it, and teleporting to
  it gives a sim whose `hash()` and Ride bytes match the first ride's teleport bit for bit;
- another track, another profile and another bike all start empty; path sanitising holds; clearing removes the file;
- another format is dropped: wrong version, a codec change, every cut length and trailing bytes;
- a recorded tape (with a pause clear in it) replays with the markers folder deleted, `sim.hash()` matching on every
  frame; an old tape without the line still plays; a tape with a bad version errors instead of panicking;
- picks round-trip, and a pick for another track or bike, or junk, reads as empty.

## Not verified / not 1:1

- **No end-to-end shell run.** The shell wiring in main.rs (load at ride start, `Kept::frame` after each frame,
  the pause row, the lobby pick) compiles, and every piece it calls is tested, but no ride was flown in the
  shell to watch a file appear and come back. Reason: the unit tests cover the sim side bit for bit, and a shell
  run needs a pad tape that places a marker, which I left to save time.
- **Online not run.** No two-shell lobby ride was run with markers in the picks. The pick's bytes are checked by
  round-trip only. Size: 12018 bytes at five slots is about 24 KB of hex per seat, so 32 seats come to about
  786 KB, under the lobby's 1 MB message cap but not by a wide margin. A bigger `Ride` would need a look at this.
- **`--peer` online and drop-in joiners start without their saved markers.** They have no pick to carry them.
  The host's spawn sim is cloned before any file loads, so it is clean and no machine sees different slots. Filed as follow-up.
- **Only seat 0's markers ride on a tape.** A tape is the first player's. Other local seats in a split-screen
  recording start empty on replay, as they did before.
- **The file is keyed by bike, not modifiers.** A marker taken under other modifiers carries those Ride values
  back, as P98 does mid-ride. Another bike's file is dropped but left on disk until that profile places a slot on this bike.
- **The selection (`at`) is not saved.** A new ride starts at slot 1.
- **P94 is not merged into this branch.** Merging task/P94 hit a conflict in plan/DONE.md, and resolving
  it by hand is against the rules. P94 changes `markers::frame`'s callers and tests/markers.rs. This task does
  not touch those: its test is in a new file, `markers_saved.rs`, and its markers.rs code is an appended section.
  The runner's merge may still conflict in main.rs near the seat setup.
- The full `./kit check` before `./kit done` was red only on netplay's `four_peers_and_eight_riders_on_a_bad_network`
  ("only 250 ticks settled"), the known flake under load with other slots running.

## Next

Nothing for this task. Follow-up: P104 (markers for `--peer` and drop-in joiners).
