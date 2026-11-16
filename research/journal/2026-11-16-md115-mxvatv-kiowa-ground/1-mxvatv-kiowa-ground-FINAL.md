# 1 — mxvatv kiowa ground (FINAL)

Task MD115. 2026-11-16.

## What I did

- `kiowa::OnMxvatv`: the Kiowa on SLUS-21104's ground, as a `Surroundings`.
  - `OnMxvatv::pass`, before the frame's world step: the record's clear and pass
    (`flight::MxvatvRecord::pass`), its heightfield rays on the terrain, its node the measure's matrix.
  - `resolve`: the record's resolve (vt +0x88, handed 1), `atv_contact::resolve` with impulses on the
    held list the pass refilled, mass the record's body's (+0x18).
  - `touching` / `entries`: the record's. `ignored`: always false (no props, so nothing struck).
  - `strike`: `rotor_in_ground` with `atv_ground::query` (0x004833c0) on the terrain; the rotor's
    contact search strikes nothing.
- `flight::MxvatvRecord::pass` (new, pub): MD110's `Flight::pass_mxvatv` body moved into a method on the
  record, so the plane and the Kiowa share it. `pass_mxvatv` now calls it; the plane's behaviour is the
  same (it wrote the matrix back from a copy; now it writes in place).
- `tests/mxvatv_kiowa_replay.rs`:
  - `Air` replaced by `OnMxvatv`, the record seeded off `*(heli+0xc8)` (`read_record`, as the biplane
    test's), the rotor spheres off `*(heli+0x11c8)`, MC02's terrain built from `mc02.res` and checked
    word for word against the helicopter's terrain object (`*(heli+0x3b4)`).
  - Each frame compares the record's words (`record_diff`: pass and resolution words) with the body.
  - Ratchet raised to every stretch at its end in both flights.
- Nothing taken from opentennis: it has no PS2 collision record (as MD110 found).
- Follow-up MD115a (low): the Kiowa's record off the disc, and the shell's MX vs. ATV Kiowa on it.

## What the original does

- The Kiowa's collision record (`*(heli+0xc8)`) has vtable 0x003b9290 (at +4): the same class as the
  biplane's record (MD110) and the bike's (F31b4). Its heightfield word (+0x24) is 1. The part
  (`+0x200`) has 22 rays in kiowa, 24 in kiowa3.
- Both images start on the ground with the record touching (+0x34 = 1).
- The frame runs the record's pass before the world step, as for the biplane. The body wrapper's force
  (0x004a4ef0 → 0x00493f60) applies gravity, then the record's resolve vt +0x88(record, 1).

## How it was proved

Bit-exact replay of MD104's captures (`context/mxvatv-ps2/md80a/{kiowa,kiowa3}.ee.bin` and tapes), in
`tests/mxvatv_kiowa_replay.rs` (`the_port_flies_kiowa`, `the_port_flies_kiowa3`). A port Kiowa is
seeded every 100 frames off the interpreter's RAM and flown beside it. After every frame, the body and
the record's words (touch flags, sweep segment, count, every contact's ray, normal, point, arms,
surface, the resolution's cofg-in-world, normal speed, relative velocity) must match word for word.

- kiowa: all 13 stretches agree to their end (before: `[0, 100 ×9, 92, 0, 0]`). This covers the take-off,
  the landing at 1092 and both stretches seeded on the ground after it.
- kiowa3: all 14 stretches agree to their end (before: `[0, 100 ×10, 91, 0, 0]`).
- No measure, command, loop, rotor or aero word drifts on any frame (`others_diff` prints nothing).
- Mutations (each run, then restored):
  - resolve's impulses off: kiowa's stretch from 1000 parts at frame 92 (the landing at 1092 is a hard
    contact). kiowa3 still passes: its landing and ground contacts take only the spring.
  - no resolve at all: every stretch seeded on the ground parts on its first frame, and both landings
    part (kiowa 92, kiowa3 91), as before MD115.

## Not verified / not 1:1

- **The rotor's strike never fires.** Neither flight puts a rotor sphere under the ground or wrecks. The
  replay only shows the terrain query answers "not in the ground" on every frame. The in-ground path is
  MD104's oracle (with the query hooked), not a capture. The port's `atv_ground::query` is MD76's proof.
- **The rotor's contact search and the record's world rays/sweep strike nothing.** SLUS-21104's props
  are not in the port's worlds. The record's list and flags agree on every frame, so along these flights
  the game's strike nothing either. A Kiowa that hits an object is unproved. `ignored` is always false.
- **The touchdown's entry word.** `entries` reads an entry's +0x24 as its ray end's `y` (MX Unleashed's
  layout, the same field `Grounded` reads). SLUS-21104's touchdown reader was not opened. It only gates
  the touchdown sound's `rand()`, which the replay does not compare (`touchdown_sound` false; the port's
  `Rng` is not SLUS-21104's).
- **The record is seeded off RAM, not built off the disc.** The rays, tuning and impulse mask come from
  the images. The shell's MX vs. ATV Kiowa still uses MX Unleashed's collision. Follow-up MD115a.
- **The damping branch and a sweep push** (`+0x44` damp, `node_move`). Not reached on these flights (same
  as MD110).
- **kiowa3 never takes an impulse.** Only kiowa's landing exercises the hard-contact impulse; kiowa3's
  contacts are all springs (shown by the first mutation).
- **The resolve's mass** is the record's body's +0x18 (`record::held`). On these flights it agrees; it
  is not checked against `1 / inv_mass` or the phy.

## Next

Done.
