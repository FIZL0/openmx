# 1 — mxvatv biplane hull (FINAL)

Task MD110. 2026-11-11.

## What I did

- `flight::MxvatvRecord` and `Flight::record_mxvatv`. This is SLUS-21104's collision record on the plane.
  It holds the record (`handling::record::Record`), its resolution (`atv_contact::Held` with no body,
  since the flight's body is the one resolved) and the active part's rays.
  - With a record, `Flight::pass_mxvatv` runs F31b4's `clear` + `pass` before the world step.
    `RecordOnPlane` supplies the outside: the node is the plane matrix (`measure.matrix`), and the
    heightfield rays ask `handling::ground::under` on the world's SLUS-21104 terrain. The sweep and the
    world rays strike nothing.
  - The pass's contacts refill the held list. The force listener resolves that list with
    `atv_contact::resolve` (impulses on), after gravity, in place of MX Unleashed's `contact::resolve`.
  - The post's touch reads the record's `touching`, and no object is ever struck.
  - With no record (every plane built today, the shell's included), nothing changes. The field is off
    the hash walk and the netplay wire (`snapshot.rs` takes it as `None`), and boxed: Sim grows a word
    (hash.rs pin 12256).
- `World::mxvatv_terrain` (default `None`). `handling::ground::Under` answers it with its terrain.
  `Under::solid` stays false, and its doc says why: what the hull meets is the terrain, through the
  record's heightfield rays, not anything solid. SLUS-21104's props are not in the port.
- `tests/mxvatv_biplane_replay.rs`:
  - `read_record` seeds the record off `*(plane+0xc8)`: `record::read`, `record::held`, and the part's
    rays.
  - `record_diff` compares every word the pass and the resolution write (`record::words`, overlaid by
    `record::held_words`) after every frame, beside the body and the tires.
  - biplane3's ratchet is raised to every stretch at its end.
- Nothing taken from opentennis: it has no PS2 collision record.
- research/oracle.txt `# MD110`. Follow-up MD110a (low): build the record off the disc in
  `Flight::new_mxvatv`.

## What the original does

- **The plane's record:** the plane's record is F31b4's bike record class (`./kit fn -p atv 0x002051f0`
  for the pass).
  - Its heightfield word (+0x24) is always 1. On the bike that word is the tip-over flag, so the plane's
    36 rays (all starting at one point near the cockpit) query the terrain every frame. Any ray end that
    points down and lies under the ground files a contact (surface by largest corner weight, depth 0).
  - The record's node frame is the plane matrix plane+0x370 word for word (dumped at biplane3 frame
    2059).
- **Frame 2060 of biplane3:** this is the first frame of the stretch from 2000 that used to part. Three
  wingtip rays (3, 6, 16) dip under MC02's ground and file contacts. The body's force listener resolves
  them every substep after gravity (record vt+0x88, handed 1).
  - The first touch is hard (an impulse into the shock words, +0xc0), which is the push and shock MD101b
    saw.
  - Later contacts take the spring.

## How it was proved

Bit-exact replay of MD101's captures (`context/mxvatv-ps2/md80a/{biplane,biplane3}.ee.bin` with their
tapes). A port Flight is seeded every 100 frames, and after every frame the body, the three tires and
now the record's words (touch flags, sweep segment, count, every contact's ray, normal, point, arms and
relative velocity, the resolution's cofg-in-world, normal speed and relative velocity) are compared
word for word with SLUS-21104's.

- **biplane3:** all 24 stretches agree to their end (before: [.., 59, 0, ..]). This covers the hull's
  strike at 2059 and the stretch seeded in the contact at 2100.
- **biplane:** all 17 stretches still agree, the record compared on every frame.
- **Mutation:** with the resolution's impulses off, the stretch from 2000 parts again at frame 59, so
  the impulse path is exercised.

## Not verified / not 1:1

- **The sweep back's hit and the world rays' hits** (`RecordOnPlane::sweep` / `search` answer none).
  SLUS-21104's props are not in the port. Along both flights the game's sweep never pushes and the world
  rays file nothing; the record's flags, segment and list agree on every frame, which shows this. A
  flight that strikes an object is unproved. The crash test's `ignored` is false for such a plane
  (MD113 is open on that class mask).
- **The record off the disc.** The test seeds the rays and the tuning off RAM. `Flight::new_mxvatv`
  builds no record, so a shell plane still resolves with MX Unleashed's search (its `with_rays`
  path). Follow-up MD110a.
- **`atv_contact::resolve` against MX Unleashed's `contact::resolve` on the plane.** The plane's
  impulse mask is all ones, so these captures cannot tell the two builds apart.
- **The damping branch** (`+0x44` damp is 0 on the plane), and the **`node_move` / `place_part`** after
  a sweep push. Neither runs on these flights.
- **Several planes at once** (`frames_together`'s fleet) do not run the SLUS record's pass. Only
  `frame_struck` (`frame`, `frame_among`, `frame_met`, `frame_props`) runs it, and no fleet flies a
  SLUS-21104 record.

## Next

MD110a (the record off the disc), MD113 (the crash mask).
