# 1 — biplane particles (FINAL)

Task P63. 2026-10-26.

## What I did

- **The shell's plane now feeds its collision object's contacts to the particles.**
  - `plane_fx_frame` (`crates/openmx/src/main.rs`) calls the new `PlaneFrame::touch`
    (`crates/openmx/src/particles.rs`) with the flight's `record` and `collider.relative`. S48's search
    writes both every frame.
  - `touch` passes every contact on the record's list:
    - as ground contacts while `on_ground` is set, with each contact's surface as its zone;
    - as object contacts while `on_object` is set.
  - With both flags set, only one point goes in as the object's. That raises the ObjectContact flag
    without a second emission off the same list. Both disc entries are off on ObjectContact anyway.
- **The contact's velocity, not the body's.** I added `PlaneFrame::contact_vel`.
  - A `CollisionModel` entry uses it for the start point (`contact − v·dt`) and for the emitter's
    velocity. When it is `None`, the entry uses `vel`, so the bike's feed is unchanged.
  - The original's entry reads the collision object's `+0x1e0`. The port's `Collider` documents that
    offset as `relative`, the relative velocity at the last contact it resolved. P58's notes called it
    "the body velocity", but it is not the same vector:
    - it includes the spin at the contact;
    - the ground does not move, so it is the hull point's own velocity.
- **Tests** (`crates/openmx/tests/biplane_particles.rs`):
  - `the_collision_objects_contacts_feed_the_wreck_dust` covers:
    - `touch` on a hand-built `Record`: ground, object, both, none, and a stale list being replaced;
    - the puffs taking `contact_vel` and not `vel`. The body's 1 ft/s is under the gate, and the
      contact's 30 ft/s emits.
  - `a_plane_flown_into_the_ground_puffs_where_it_hits` uses the disc:
    - the plane is built from `biplane.phy`, with `biplanem.col`'s rays;
    - it is dived nose-first into `Flat` ground at (0, −80, 80) ft/s, the way game-sim's
      `flight_collision.rs` flies it;
    - each tick, the frame is fed through `touch` into free06's `Vehicle Dust` and `Vehicle Chunks`.
    - Result: the hull first touches on tick 12 and crashes on that tick, and the first puff is on that
      tick. There are no particles before it, and every new particle starts within its extent plus a
      frame's path of one of the frame's contacts.
    - After 90 ticks: 105 dust and 189 chunks are in the air. I ran it once with a probe panic to see
      the numbers; a passing run prints nothing.
- **Follow-up filed: P156.** The bike's wreck feed (P74's `bike_fx_frame`) still uses the body's
  velocity, and on both flags it emits twice off one list. It should call `touch` the same way. I did not
  do it here: it is the bike's task, and its test file is different.

## What the original does

- The CollisionModel entry's frame is `./kit fn 0x003d75d0`. I re-read it for this task.
- **Flags.** The collision object is `*param_2[10]`:
  - `+0x48` (on the ground) sets GroundContact;
  - `+0x4c` (on an object) sets ObjectContact;
  - neither sets NoContact;
  - the wreck reason sets Wrecking or NotWrecking.
  - If the entry's Disable has any of these bits, the entry is off.
- **Velocity.** `+0x1e0 × dt` is computed once.
- **Contacts.** The entry walks the whole list: count at `+0x100`, entries of `0x80` bytes from
  `+0x108`. For each contact:
  - its surface (`+0x70`) indexes the entry's zone bytes. `0xff` skips the contact; a different group
    switches the sheet group;
  - the previous point is set to `point(+0x40) − that`, the current point to `point`, and the
    emitter is run.
- **Which list.** The one list is emitted whichever flag is set. The two flags only gate it.

## How it was proved

- **Synthetic and disc tests** (above): the wiring, the gate, and the timing on the hull's first
  contact. These run through `./kit check`.
- **Live in the shell, by eye beside S57's capture video.**
  - The original is s5 `context/s57/run1.mp4`. Video time ≈ row / 30 s: the csv's 4800 rows match the
    160.2 s video.
  - The wreck rows are 1084 (frame 12067) and 1942 (12925). Contact sheets at 36.05–36.65 s at 15 fps,
    and at 36.5–40.5 s at 4 fps, show:
    - the impact at ≈36.25 s throws a small **light-grey** dust burst where the wing meets the ground,
      gone in about 0.3 s;
    - when the tumbling wreck comes down again (≈38.25 s), another puff.
    - The "dark puff" in the brief is the plane's shadow on the ground beside it in the dive frames. The
      dust itself is light grey-tan.
  - The port: `openmx --vehicle biplane --play <dive tape> --headless`. The tape holds Cross for 360
    ticks, then the left stick fully forward. Shots were taken every 8 ticks over 420–484.
    - At tick 436 (the impact) there is a light grey-tan burst beside the hull, much like the
      original's.
    - Later frames show the wreck's dust behind the tumbling plane.
  - The per-second log:
    - `Vehicle Dust 0, Vehicle Chunks 0` for the whole climb and dive;
    - 22 dust and 101 chunks in the impact second;
    - dust again at each bounce of the 5 s tumble (23, 26, 54, 74), with the chunks dying out;
    - after the put-back, the last of it runs out.
  - The tape and the shot script were scratch files and were not kept. The tape is three lines of
    python: Cross is byte 3 `0xbf` with pressure byte 14 `0xff`, the left stick y is byte 7 = `0x00`
    from tick 360, and dt bits are `0x3d088889`.

## Not verified / not 1:1

- **The draw is compared by eye only, and on different crashes.** The original's crash is S57's
  recording over free09's terrain. The port's is a hand-made dive on free06, at a different speed and
  angle. The puff's colour and its place at the hull match. Its size and count are not compared, and
  the port's burst looks somewhat bigger. Rendering is matched by eye by policy.
- **No frame-by-frame particle count against the original.** A PINE capture of the particle manager's
  live count was not made. The emission rules are P58's, read from the decompile.
- **`relative` is whatever the port's resolution leaves.** It is the velocity at the last contact that
  `contact::resolve` handled in the frame's last substep. That is the original's `+0x1e0` only as far
  as S48's resolve matches the original's order of contacts. That match is S48's to prove, not this
  task's.
- **Surface −1.** A contact with surface −1 is clamped to zone 0. The original indexes its zone bytes
  at −1, which reads the byte before the table: the top byte of the preceding pointer, 0. That picks
  sheet group 0 rather than zone 0's entry. The plane's terrain and mesh contacts carry a surface, so
  this was not seen.
- **Both flags at once** puts one extra point in as the object's. With a modded list that leaves
  ObjectContact enabled, that point would emit once more than the original. Every list on the disc
  disables ObjectContact.
- **Object contacts on the plane** (props, meshes, bikes) were not flown live. Only the ground crash
  was. The disc entries are off on ObjectContact, so nothing is drawn there.

## Next

Nothing for P63. P156 feeds the bike's wreck dust the same way.
