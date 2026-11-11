# 1 — mxvatv car step

Task MD127 (MD106a split, part 3). 2026-11-11.

## What I did

- `atv_car::Pieces` (crates/game-sim/src/atv_car.rs) now also answers `Call::SortContacts` and
  `Call::Probe` with MX Unleashed's contact sort and tire probe, as SLUS-21104 builds them
  (`sort_contacts`, `set_contact`, `probe`).
- `Tire` gains the probe's words: mount, radius, a ground to ask (`on_ground`), probe and car normals,
  drop, lever, the axle's upper and lower points, the probe count, the contact point and normal, the
  terrain hold, the surface byte, the hit record, and the query's answer.
- `Car` gains `sort_rays` and the search's answer (`SortAnswer` / `SortHit`).
- What the two pieces call is handed out as new calls. The answers go into the car's and tires'
  fields, the way `ground_normal` and `node_pos` already do:
  - `SortPlace` and `SortSearch`: the ray probe's vt +0xcc and its search. `SortSearch` answers into
    `Car::sort`.
  - `Struck(i)`: a struck object's listener told of hit `i`.
  - `TireGround(k, at)`: the ground query. It answers into `Tire::query` and `Tire::surface`.
- `State::front_normal` is gone. The anti-slide reads the left front tire's `ground_normal`, which is
  the same word (tire +0x110).
- tests/mxvatv_car_frame.rs:
  - The tire's new words are read, poked and compared in every run.
  - `hook_sort` installs:
    - the place and search hooks. On the car's probe, the search lays the script's hits out in free RAM:
      list arrays, records, objects with one listener, and movers.
    - a stub listener that logs `Struck`.
  - The ground query hook answers the probe's queries (ra after the probe's call) from the script.
  - `generate_probe` and `generate_sort` draw the new inputs from a second Rng only when the sort or
    probe is unhooked, so the other tests' streams are unchanged.
  - New tests: `contact_sort_is_mx_unleasheds`, `sort_and_probes_are_mx_unleasheds` and
    `all_seven_pieces_are_mx_unleasheds`.
  - The update test sets `Tire::surface` from `Update::tire_surface`, because they are the same byte.
- research/oracle.txt `# MD127`: addresses, offsets, constants and image values.

## What the original does

The sort and the probe are MXU's (`car::Machine::sort_contacts`, `car::Wheel::set_contact`,
`car::Wheel::probe`), with these differences:

- **The probe's agreement cosine (5°) is a literal**, 0x3f7f069e. MXU reads it off its curve table.
- **The probe returns at once without a ground world** (tire +0x1a4 null). Its first act is clearing
  solid.
- **A sorted tire with the ground below its axle bottom gets its surface byte cleared** (tire +0xf0,
  the ground query's record argument).
- **The hand-off works out the car-frame normal and the drop**, which MXU leaves to the probe:
  `drop = unit((0, cn.y, cn.z, 1)) · -radius`.
- **The sort picks a tire with `c.ole.s` against 0**: rear when z <= 0, left when x <= 0. This is the
  same as MXU's `> 0` (the PS2 has no NaN). ±0 rays go rear or left, and the test draws them (the
  "on a zero" tag).
- **A struck object's listeners are told before the hand-off** (vt +0xc with 0, 0), and a mover is
  kept only when its kind word (+0x1a0) is 2.
- **The terrain hold**:
  - It is set when the axle faces the contact normal less than 0.707.
  - Otherwise, a tire not held takes the hold when `!(ground <= 0.1 + sorted.y)`.
  - A held tire lets go when `!(sorted.y <= 0.1 + ground)`.
  - While held, the tire leaves the sorted contact for the terrain. Otherwise it keeps the sorted
    contact and is not solid.

## How it was proved

Function oracle in the frame: SLUS-21104's world step over MD39a's buggy image, with
`SORT_CONTACTS` and/or `PROBE` unhooked, against `atv_car::frame` with `Pieces` answering them.

Runs:
- 3,000 generated frames with the sort alone;
- 3,000 frames with the sort and probes;
- 3,000 frames with all seven pieces.

The other nine tests still pass (`./kit check -- -p game-sim --test mxvatv_car_frame`: 12 passed).

In every frame:
- every word of the car, its tires (the new words too) and its body equals the port's;
- the call logs (place, search, struck, every ground query with the point asked) are equal;
- the original wrote nothing in those blocks that isn't compared.

Every branch is seen at least 20 times:
- sort: no rays, nothing found, each of the four tires, a ray on ±0, struck, mover kept, mover
  dropped;
- probe: no ground, asked once, again, and five times, below, below with the surface cleared, solid
  on terrain, sorted dropped by up, held by the axle, held taken, held kept, let go, sorted kept.

Mutation checks:
- Without the surface clear, sort_and_probes goes red at case 5 (1 word).
- With the sort's `z <= 0` made `z < 0`, contact_sort goes red at case 11 (29 words).

## Not verified / not 1:1

- **The ray probe's place (vt +0xcc) and search (0x001151c0) are hooked, not ported.** The port
  hands them out as `SortPlace`/`SortSearch`, and the search's hits come from the test's script. Who
  builds real hits is left to the ray search's own task.
- **The struck listeners are hooked**, and in the test each struck object has exactly one listener.
  The original walks however many the object lists. The port hands out one `Struck(i)` per hit with
  an object, so an object with zero or several listeners is not 1:1.
- **The original re-reads the hit list after each listener runs** (count and arrays). The port
  works on the search's snapshot. A listener that changes the list would differ. The test's
  listeners don't.
- **At most four hits** (`SORT_HITS`). The search's real limit wasn't read; the image's four rays
  suggest one hit each.
- **The ground query always answers a normal.** When the original's query writes none, the probe
  reads its stack buffer's stale value. The port has no such value.
- **Which ground world** the probe asks is not modelled. `Tire::on_ground` only says whether the
  pointer is non-null.
- **The idle buggy's place and search are skipped** by the hooks when the sort runs unhooked. Its
  probe's flags byte says nothing was struck, so its sort returns. Its tires are not compared.
- **`Update::tire_surface` and `Tire::surface` are the same byte** (tire +0xf0), held twice. Whoever
  wires the update and the probe into one ride must keep one.
- **Only the buggy** (MD39a's image) was run, not the other car classes.

## Next

MD106c (whole frame) once the remaining pieces land.
