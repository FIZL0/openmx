# 1 — s reason six (FINAL)

Task S37. 2026-10-20.

## What I did

Ported the wreck test's last clause, the loop over the collision record's contact list, and the one
contact field it reads. Then gave the port a contact for it to read.

- `crates/game-sim/src/air.rs`:
  - `Wreck::Scenery = 6`.
  - `Contact { mirrored, reach, other }` and `Other::{Scenery, Rider, Object}`: the three things the
    clause reads off a list entry.
  - `hit_wreck`: the clause itself.
  - `HIT_REACH` (6) and `HIT_REACH_FIFTY` (4).
  - `HEAD`: the bike's head ray, measured.
- `crates/game-sim/src/ride.rs`:
  - `hit_of` gets `6 => Hit::Front`.
  - `hits(ride, world)` sweeps `HEAD` through the ride's transform against the props and returns a
    Scenery contact when it hits.
- `crates/game-sim/src/rider.rs`: the wreck-test closure runs `wreck_test` and then `hit_wreck`, and
  keeps the hit's reason when both answer (`hit.or(landed)`). That is what the original's two entry
  calls leave in +0x748.
- `research/oracle.txt`: an S37 block with the list layout, the 50cc flag, the eleven body rays, and
  the collision record's collide, clear, add and resolve (the record vtable).

The live work is in `live/`:

- `scenery.toml`/`play.sh` recorded recording2's playback over the wreck. Reason 6 at frame_counter
  4124 is reproduced, but the list is empty at every tick boundary.
- So `save.sh`/`save_before.py` saved PCSX2 states at the 4121/4122/4123 boundaries during playback,
  and the EE RAM of each went to `context/states/s37-{7,8,9}.ee.bin` (slot-local, not committed).
- `ram.py` reads the bike, its record and its rays out of an image. It placed each image on its
  capture row by the vehicle's position.

## What the original does

`./kit fn 0x2522c0`, the loop at the end, after the landing clauses. It is not inside their
`+0x748 == 0` gate:

- If the record (bike+0x7d0) has +0x40 set, walk its list (+0x108, count +0x100, 0x80 bytes an entry).
  Skip entries whose +0 is nonzero (mirrored in by another record). Also skip entries whose +0x24 is
  not above 6, or 4 when bike+0x588 is set.
- The first entry that passes ends the walk, whatever happens next:
  - +0x74 (the other actor's record) is 0: enter reason 6.
  - The actor has a chassis (+0xf0) higher than the bike's by more than bike+0x784
    (JoustWreckHeight): copy its velocity (+0x260) to bike+0x560, then enter 7.
  - The actor has no chassis but a rigid body (+0xf4) whose mass (+0xc) is above the bike chassis'
    (+0x210): copy that body's velocity (+0x70) to bike+0x560, then enter 7.
  - Otherwise nothing.

What the field is: the writer `./kit fn 0x270240` puts the hitting ray's start and end, in the bike's
model space, at +0x10/+0x20. So +0x24 is how high the ray reaches, not where it hit. The bike has
eleven body rays (record+0x1f0's object), all from one hub at y 4.06. Only ray 8 (end y 6.674, the
head) reaches above 6. Rays 0 and 1 (the shoulders, 4.58) reach above 4. Reason 6 is therefore the
rider's head hitting scenery, or a 50cc rider's shoulders.

bike+0x588 is set in the bike constructor when the bike's name contains "Race50" or "Pro50" (strstr).

The entry stores its reason unconditionally, so when a landing clause and the hit both fire on one
call, the hit's reason is the one that stays. The window shows exactly that: over the 4123 image the
original's wreck test entered 0xb and then 6, and the capture shows 6.

## How it was proved

- **Function oracle, 10,000 generated substeps**: `the_hits_are_the_originals` in
  `crates/game-sim/tests/air.rs`.
  - Each substep is the existing wreck-test state plus a generated list of 0 to 11 entries: mirrored
    or own; reaches at the rays' ends, at the two limits and anywhere; scenery, riders and objects
    either side of their thresholds. It also sets the 50cc flag, the chassis mass and the chassis
    height.
  - The original's whole wreck test runs over that state. Its calls to the entry, in order, must equal
    the port's `wreck_test` followed by `hit_wreck`, and every word of the object graph must match
    afterwards (bike+0x560 included).
  - Path tally in the run checked: Scenery 1091, Joust 929, untouched 4978, an actor that let the
    rider off 2040, nothing reaching 962, a 50cc shoulder 1672, both clauses on one call 1979. The
    test asserts at least 50 on each.
- **The window**: `a_hard_hit_on_scenery_wrecks_at_reason_six` in `crates/game-sim/tests/ride.rs`.
  - On the frame_counter 4123 image, the bike's own `collision_record_collide` gives one contact: own,
    reach 6.6743364 (the head ray, equal to `air::HEAD`), no actor.
  - The port's `hit_wreck` over that list says Scenery.
  - The original's wreck test over the same image leaves 6 as the last reason entered, once.
  - On the 4122 image the list is empty and neither test enters anything.
- **The port's own sweep**, in the same test: a rider thrown under a sloping beam at head height
  wrecks at 6. Thrown at a wall, the rider does not wreck at 6, because the bike's nose meets the wall
  first.
- `./kit check` passes. `./kit audit` is ok.

## Not verified / not 1:1

- **The port sweeps one ray, not eleven, and once a frame, not per substep.** The original's record
  sweeps all eleven body rays every substep. The port sweeps the head ray once, through the transform
  the frame ended on. Reason: the rays come from the bike's model, which the port does not load.
- **The head ray's hit is not resolved.** The original's record also pushes the bike off what its body
  rays hit (`./kit fn 0x270dc0`). The port's `ride::hits` only reports the contact. Same reason; the
  frame's own body probes already do the port's pushing.
- **`HEAD` is one bike's ray, from one RAM image**: the freeride bike recording2 rides. Another bike's
  model may put the hub and the head elsewhere. Not measured.
- **The ray's frame is assumed.** I assumed the port's chassis transform (`Ride::transform`) is the
  frame the original's rays are in. It is consistent with the port's origin sitting at ground level,
  but it was not compared against the original.
- **The 50cc limit never applies in the port.** The rider is not told which bike file it rides, and the
  shoulder rays are not swept. The 4 is proved by the oracle but not wired.
- **The port only ever produces Scenery.** The Rider and Object branches (reason 7, with the impact
  velocity) are ported and oracle-proved, but the port's world has no actors to hit.
- **The port asks the hit only while the rider is not down.** The original's loop is not gated by
  +0x748, so it can enter 6 or 7 again on a rider already down. Reason: re-entering would restart the
  port's `Crash`, and what the entry does to a down rider was not measured.
- **One reason, not two entries.** When both clauses fire, the original calls the entry twice, with
  all of the entry's side effects both times. The port hands one reason (the one that stays) to its
  caller.
- **The mass handed over is `1 / inv_mass`**, not the stored +0x210, which the port's chassis does not
  hold. It is read only in the Object branch, which the port never reaches.
- **`hit_of(6) = Front` is inference**, like the rest of that table.
- **Not pinned to the game's own tick.**
  - Where the collide pass sits in the tick relative to the wreck test was not found.
  - The window proof runs collide and the wreck test offline over boundary images.
  - Whether the game's own tick 4124 entered 0xb before 6 (as the offline run over the 4123 image did)
    is not verified: the capture shows only the reason that stayed.
- **The window images are slot-local** (`context/states/s37-8.ee.bin`, `s37-9.ee.bin` in slot s4).
  Elsewhere that half of the test prints SKIPPED and passes. `live/save.sh` remakes them.

## Next

Nothing waiting. Follow-ups if wanted:

- Load the bike model's body rays and sweep all eleven per substep with the record's resolve, which
  would retire the first four items above.
- Carry the engine class to the rider for the 50cc limit.
