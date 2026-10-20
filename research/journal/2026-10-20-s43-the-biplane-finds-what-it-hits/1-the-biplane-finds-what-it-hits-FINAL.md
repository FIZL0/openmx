# 1 — the biplane finds what it hits (FINAL)

Task S43. 2026-10-20.

## What I did

- Ported the collision object's contact search as `crates/game-sim/src/contact_search.rs`, a new module.
  - `clear` empties the list, resets the four flags and resets the body's damping.
  - `add` appends a contact if there is room.
  - `sweep_back` is the pre-pass.
  - `collide` is the whole search: sweep back, then the heightfield rays, then the world rays.
  - `ground` and `surface` are the terrain point query and the ground result's surface pick.
- The terrain comes in through a `Heightfield` trait. `TerrainField` implements it over
  `track::Terrain::cell`. The two world searches come in through a `Search` trait (see S44).
- `contact::Contact` gained `from` (`+0x10`, the ray's start), which the search writes and
  `contact.rs`'s test now reads too. The small vector helpers in `contact.rs` are now `pub(crate)`.
  Nothing else in it changed.
- Named the routines in `research/oracle.txt` (S43 block): `terrain_point_query`, `terrain_cell_read`
  and `collision_record_sweep_back`, plus notes on the ground result and on `transform_set_position`.
- Filed **S44**: the world searches themselves, and wiring `collide` into the ride.

## What the original does

`./kit fn 2708a0` (record, dt), vtable `+0x7c`. Clear is vtable `+0x74` (`./kit dis` on the slot).

1. **Clear.**
   - Count `+0x100` = 0 and the flags `+0x40`, `+0x44`, `+0x48`, `+0x4c` = 0.
   - If `+0x50` and a body (`+0xf4`): the body's damping becomes `(+0x54, +0x54, +0x54, 1)` and its
     angular damping `(+0x5c, …, 1)`, through the body's vt `+0x1c` / `+0x54` (whole quads).
2. **Sweep back** (`./kit fn 270390`). This runs if `+0x3c` is set and there is a sweep probe `+0x1f4`.
   - `r` = the offset `+0xb0` turned by the body's quaternion.
   - The segment is `+0xd0` = `+0xe0` − velocity·lead(`+0xc0`)·dt to `+0xe0` = position + `r`.
     Both keep the position's `w`.
   - The segment is set on the probe, the probe is told it moved, and the probe is searched: the
     nearby search with `+0x10c` if there is one, the world search otherwise.
   - On a hit (`+0x116 & 0x20`) the struck object is checked. If it is another collision object
     (`+0x80`) or a kind-2 mover (`+0x7c`, `+0x1b0`), both velocities are compared:
     - own velocity: body `+0x70`, or chassis `+0x260`;
     - the other's: its body or chassis, the same way;
     - the mover's: its vt `+0xb8`.

     If both speeds are over 1 and the dot of their unit directions is over the facing `+0xc8`, the
     sweep does nothing (they are flying apart).
   - Otherwise the body goes to `point + push·push_out(+0xc4) − r`. The transform's position is set
     to that, the ray probe is told it moved, and `+0xe0` is set to it.
3. **Heightfield rays.** These run if there is a ray probe (`+0x1f0`), `+0x34` is set, `+0x30` is
   set, and there is a terrain (`+0xfc`).
   - "Up" is the body's quaternion un-turning `(0,1,0)`, or with no body the transform's y row.
   - Each ray whose direction points down against up has its end placed through the transform. The
     end must be strictly inside the terrain's x/z bounds. It is then queried
     (`./kit fn 1dce80`).
   - The cell is `coord·per_sample`, clamped at 0 and truncated, with the odd bit cleared when the
     terrain is coarse (step 2).
   - The cell reader `./kit fn 1ddaf0` sits behind a 10-entry cache, and heights are scaled by
     terrain `+0x80`.
   - The normal is the four corners' normals blended by bilinear weights and normalised.
   - The height comes from one of the cell's two triangles. Which diagonal is chosen depends on
     terrain `+0x760 == 1` or on the cell's parity, and then on the nearer corner.
   - A contact (kind 0, the ray's start and end, the ground point, the normal) is added when the
     ground is not below the end. Its surface is the corner of greatest weight (strict, from 0).
     `+0x40` and `+0x48` are set.
4. **World rays.** This runs if `+0x34` is set: the ray probe is searched (nearby or world, as
   above). For each of the result's `+0x8c` hits:
   - one contact of surface −1, with the struck object's other record and its mover if kind 2;
   - `+0x40` and `+0x4c` are set, and `+0x1fc` is set to the struck object;
   - if the object is another collision object: `+0x44` is set on both, the other's `+0x40` and
     `+0x4c` are set, and it gets a kind-1 contact with the normal negated and this record as its
     other.

**The brief was wrong in two places.** `+0x40` is a flag ("touched anything"), not a contact pointer.
`+0x1fc` is the last struck *object*, not a surface. The oracle compares both as such, and they match.

**Water.** None of the search, the point query, the cell reader or the ground result mentions water.
The ground result's depth slot (vtable `0x44daf0` `+0x14`) returns a constant 0, and nothing in the
search reads a depth. Water can reach the plane's contacts only through the world searches'
collidables, which is S44.

## How it was proved

Function oracle, `crates/game-sim/tests/contact_search.rs`, on C7's biplane save state:

- **`the_heightfield_query_matches_the_original`**: 10 000 points, comparing the port's `ground`
  with the original's point query. The terrain is the plane's real one, read through the original's
  own cell reader (`EeField`).
  - Coarse and fine, both split kinds, and points off the terrain.
  - Point, normal, surfaces and weights compared bit for bit.
  - 8 861 on a cell (4 431 split by parity, 4 430 one way), 1 139 off the terrain, 3 364 two
    samples wide.
- **`the_contact_search_matches_the_original`**: 10 000 cases of the real clear plus the real
  collide on the plane's own collision object, through its vtable slots (asserted to be the named
  functions).
  - Hooked in the original: only the world searches, the segment setter and the "moved" refresh
    (logged), plus a mover's velocity virtual.
  - Each case generates:
    - the transform and the body, near the real ground;
    - the record's flags, tuning, capacity and probes (present or not);
    - 0–12 rays;
    - 0–4 world hits on three struck objects;
    - two other collision objects on a body or a chassis, with partly full lists;
    - two movers (one of kind 2);
    - the sweep's hit;
    - dt.
  - Compared bit for bit: every record's flags and contacts (all fields), `+0x1fc`, `+0xd0`/`+0xe0`,
    the transform's position, the body's position and both dampings. The calls into the world
    (which probe, which search, the segment, the refreshes) are compared in order.
  - Reached, asserted: 6 740 ground contacts, 7 194 object contacts, 3 385 contacts given back to
    others, 3 043 with a mover, 3 104 cases with a list full, 1 460 sweeps that moved the body,
    1 204 that refused (flying apart), 1 604 cases with no body, 965 with no ray probe, 4 242 damped.
- Mutations, each failing then restored:
  - the ground test inverted fails at case 1 (flags);
  - the facing refusal removed fails at case 1 (calls);
  - the push-out dropped fails at case 12 (a ground normal, since the moved body moves the rays).

  Two equality-boundary mutants (`<=` → `<`) survive. Random floats never land exactly on the
  boundary, and the original's comparison was read from the disassembly (`c.ole.s`).
- `./kit check` (`--no-fail-fast`): everything passes except `trick::the_humans_tricks`, the known
  failure on C6's capture (`rows whose pad is not field 2k - 2`). It doesn't touch any code this task
  changed.

## Not verified / not 1:1

- **The world searches are not ported.** `Search` stands in for the nearby and world searches and
  the probes' narrow phases. The test feeds both sides the same hits. What the real searches find,
  and whether water is among it, is S44.
- **`TerrainField` (over `track::Terrain::cell`) is not proved against the cell reader.** The test
  runs the port's `ground` over the original's reader. Whether `Terrain::cell` returns the same
  pre-scale heights, normals and surfaces as `./kit fn 1ddaf0` is unproved. `Terrain::cell` also
  refuses cells that cross a block edge, which the original never checks.
- **The cell cache is not modelled.** It is global, 10 entries, keyed only by `ix<<16|iz`, not by
  terrain or step. A coarse and a fine lookup of the same index can share an entry in the original.
  The test resets the cache's ready flag whenever it toggles coarse, so that aliasing is never
  exercised. In one terrain at one step it has no effect.
- **The transform's dirtiness is not ported.** `transform_set_position` marks the node, its parents
  and its children dirty, and the world rows are rebuilt later. The port writes `node.origin` in
  place, and the test compares it against local `+0x70`. A parented transform is not modelled: the
  plane's has no parent, which the test asserts.
- **A record on a chassis with no body.** The sweep back runs only with a body (`+0x3c` needs one in
  the original's own data). The own-velocity chassis branch is read but not ported. Other records'
  chassis velocities are ported and proved.
- **The ground result is a global object.** Its state between calls (key `0x7fffffff`, stale
  normal/surface on a miss) is not modelled. Every reachable path writes what is read.
- **Unreachable paths:** a terrain with no tree, and negative cell indices (clamped at 0 before
  use). Not modelled.
- **Listeners and callbacks** on the probes and collidables are not reached here (hooked away with
  the searches).
- **Generated ranges are bounded** (positions near the ground, speeds ±40, unit-ish normals). No NaN
  or infinity was generated.
- **Not wired into the ride.** Nothing calls `collide` before `contact::resolve` yet (S44).

## Next

S44: port the world searches behind `Search`, then call `collide` and `resolve` from the ride.
