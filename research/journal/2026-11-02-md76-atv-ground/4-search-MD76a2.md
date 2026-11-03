# MD76a2 — the quad's wheel-ray search

2026-11-03. Task: `./kit plan MD76a2`.

`game_sim::atv_search` ports the quad's `ContactCall::Collide`: SLUS-21104's `atv_collide_against_search`,
run with the quad's trigger probe (quad `+0x1430`) over the nearby list the gather fills (quad `+0xc4`,
MD76a1). The probe's record, read out by `atv_search::contacts`, is the contact list `quad::wheel_contacts`
already takes. `tests/atv_search.rs` runs the original's own code off MD37a's `atv-r-spawn.ee.bin`, with
nothing hooked: the probe's "stand at this frame", then the gather, then the search. It compares the
record with the port's bit for bit.

## What the search is

It is MX Unleashed's `world_search::search_nearby` at the ATV's offsets (oracle.txt, MD76a2 block), with
two differences:

- A class-2 collidable whose first word is 1 is passed over before it is marked searched. Nothing in
  Desert Oasis's static world has it set, so `atv_search::search` takes a `retired` predicate, and
  `collide` passes "never".
- The probe's prepare runs only when the list carries the terrain grid (list `+0x40`) *and* the caller
  asks. The quad asks, but its list's `+0x40` is 0, so prepare is a no-op.

The pair is MX Unleashed's `ray_probe::pair` (record fields after `+0xac` moved up a word). Every pair
first zeroes a word per contact at `+0xac`. Nothing reads or writes that array afterwards, and the test
checks it stays zero.

The probe has four rays, one per wheel. The port builds their tree with `kiowa::ray_tree` from LF, RF,
LR, RR. The image's tree holds them as RF, LF, RR, LR, which `image_rays` reorders by sign, and the trees
come out equal word for word. The record holds three contacts a ray (12) and keeps one per ray.

Standing the probe at a frame copies the matrix, puts the centre at the root box's centre through it
(as `flight_search::reach`), and sets the reach to `sqrt(mula hx², madda hy², madd hz²)` over the root's
half extents. `atv_search::reach` does the same.

The frame is the quad's scene node: the body's rotation, with position = body position − row 1. The
body's CofG is (0, 1, 0).

## The plants

A plant group's parts are its species' collidables. They are shared by every plant of the species and
stood at the plant each time the search asks for one.

- **Spheres (kind 5, category 0x80).** Nine of MC02's eleven species have only a sphere. A sphere's
  ray test writes no contact; it can only set the hit bit. The port does not test spheres
  (`Shape::Never`), so its hit bit can be clear where the original's is set. The test allows this only
  when the original's record holds no contact and its list held a plant: 131 of 6,001 frames.
- **Cylinders (kind 0, category 0x40).** These are prickly pear B and weeds B. They are MX Unleashed's
  built cylinder (`eco::cylinder` + `eco::build`, via `eco::Trees`), stood at the plant's cell with
  identity rows. The test checks:
  - each species' part kinds and categories;
  - that no part has listeners, a callback, an owner or a vehicle;
  - both cylinders' trees, node for node, against the image's.

  MC02's two cylinders have **height 0**, both in the file and in the image's tree (flat rings). No
  ray can strike them, in the original or in the port. The 1,200 frames aimed at trunks run the
  plant path and come out equal, with no contact on either side.

## The proof

`the_trigger_probe_is_the_images`:

- the probe's tree word for word, and the record's capacity and one-per-ray;
- the stood centre and reach bit for bit;
- the species' parts and the two cylinder trees;
- all 120 scene meshes: frame (`+0x130` = the prop's `to_world`), kind 0, root 0, not retired, no
  listeners, no owner, no vehicle.

`the_quads_search_is_the_originals`: 6,001 frames.

- 40 frames at each of the 120 meshes. Each puts a random wheel's ray across a random triangle's
  centre (±0.6 ft), with a random yaw and a tilt of ±0.35 rad; one frame in eight is tipped
  anywhere (±3 rad).
- 4 tipped-anywhere frames at each of 300 trunks.
- The spawn's own frame.

On every frame the original's gather and search run first, then the port's, with the record carried
from frame to frame on both sides. The whole record must be equal: every contact's ray ends, normal,
point, struck collidable, placed, fraction and segment, plus the nearest, push, low, high, first and
flags. The hit bit must be equal except in the sphere case above.

Result: 1,421 frames struck, 1,617 contacts on all 120 meshes (1,263 frames with one contact, 129 with
two, 20 with three, 9 with four); 3,105 lists held plants.

`the_captured_swept_contacts_are_the_searchs`: MD37a's 8 rows (race-jump, race-wheelie-launch) where a
wheel's swept contact (`+0xd4`) is set.

- Capture rows are two frames apart, and the search runs at a frame the capture does not hold. So for
  each row the test tries 17 blended frames: eighths from the row two back to the row before, then from
  the row before to this row. That is 136 frames, and on every one the original's record equals the
  port's.
- The blended frame's contacts go through `quad::wheel_contacts`.
- In 3 rows a frame gives exactly the captured contacts: the same wheels touching, the same normal,
  and points within 0.25 ft (0.010, 0.021 and 0.018 ft):
  - race-jump 35853 (wheel 1);
  - race-wheelie-launch 35855 and 35859 (wheel 2).
- In the other 5 rows no blended frame does (see below).

How it was checked: `cargo test --release -p game-sim --test atv_search -- --nocapture` (3 passed),
`./kit check -- -p game-sim --test atv_search`, `./kit audit` (only the two existing plan/DONE.md leaks).

## Not verified / not 1:1

- **The sphere ray test is not ported** (`atv_ray_sphere`). It writes no contact, so contacts are
  unaffected (follow-up MD85), but the port's hit bit misses sphere-only strikes (131 of 6,001 frames). The distributor
  reads the record only when the hit bit is set, and an empty record gives no contacts either way.
  Anything else that reads the quad's hit bit would differ.
- **The retired word is never set** in Desert Oasis's static world. The skip is ported and passed
  "never"; it is untested.
- **Prepare is not ported**: the quad's list never carries the terrain grid.
- **Cylinder contacts are untested**: MC02's cylinders have no height. Another map with real ones would
  exercise the trunk path.
- **The cylinder's turn** (group `+0x16` bit 2, eco `+0x70`) is not ported. The table is NULL in the
  image.
- **Hierarchies, other vehicles and bodies** are not in Desert Oasis's static world. The record's part
  word, owner and vehicle are always 0 here, and the test copies the original's part word across.
- **5 of the 8 captured rows are not reproduced.** No blended frame gives their contacts:
  - race-jump 35855: the nearest frame has another normal;
  - race-wheelie-launch 35805: wheel 2 touches where the capture has none;
  - race-wheelie-launch 35851 and 35861: wheel 1 or 2 is 0.5–0.6 ft off;
  - race-wheelie-launch 35863: never touches.

  The search is bit-exact at every frame tried, so these show that the blended frames are not the one
  the game searched at, or that `+0xd4` is not just this frame's search. Its lifetime was not traced.
- **Nothing calls `collide` from `quad` or the shell yet.** The brief pointed at `quad.rs` and
  `tests/atv_quad.rs`. The port went into a new module and test instead (it stands alone, like MD76a1's
  `atv_world`). Wiring the search into the quad's Moto, with the gather and the node's frame, and
  unhooking it in the frame oracle is follow-up MD84.
- **No live PCSX2 run.** All of it runs off the one image (atv-r-spawn).
