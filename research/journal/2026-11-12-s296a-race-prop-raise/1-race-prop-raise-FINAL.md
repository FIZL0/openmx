# 1 — race prop raise (FINAL)

Task S296a. 2026-11-12.

## What I did

- Ported the race activity load's prop raise as `race::TrackFile::raise_on_props(nodes, scene)`
  (crates/game-sim/src/race.rs). It takes the nodes `snap_to_ground` hands back (`word_04 & 0x80`).
  For each of a node's three points it builds a one-probe bundle: origin (x, y + 900, z), straight
  down, length 1000, with the sphere centred on the point and radius 1000. It asks the put-back's
  gather and per-object sweep (`back_search::query_each`, S83's port of put_back_solid; the same
  broad-phase function, kind 3) with a fresh search, and folds every met object's record point 0 y
  into the node's **first** point's y with `c.lt.s` (`F::from_bits` on both sides).
- `race::knockable_meshes(&PropSet)`: the knockable props' `f.col` meshes as `world::Prop`s, where
  their nodes stand at load (`PropSet::matrix`). The raise searches them as well as the scene's
  `.col` meshes, because on sx20 a hay bale is what lifts a node.
- Test `the_race_files_raised_onto_their_props_are_the_track_objects_in_the_images` (crates/game-sim/
  tests/race.rs, entry 36 in its list): five tracks, each against a RAM image of a race on that track.
- New RAM images, made in my emulator copy 4 with S133's `boot.py <track> <league> 2 <slot>` (slots
  31–35 of copy 4, league 0 for sx and 1 for nat03), then `./kit state ram`: `context/states/
  s296a-{sx06,sx08,sx11,sx20,nat03}.ee.bin`. The test skips without them.
- Updated oracle.txt's race_prop_raise line (ported; the two masked classes are the VEHICLE and RIDER
  category bits).
- Filed S318: `world::Ground` refuses the terrain's last sample row and column, where the terrain
  object's own query answers (see below).
- Nothing from opentennis (it has no race files or prop sweeps).

## What the original does

`./kit fn 0x2f8b40` (the ground snap) builds one sweep probe (count 1, length 1000, direction
(0, −1, 0), radius +0x160 = 1000) and a search (cell 10, reach 30, 128 kept). For each flagged node
and each point it sets the probe's centre +0x150 to the point and its origin to (x, y + 900, z). It
re-files the probe (radius 10), then calls `./kit fn 0x2f8820` with t2 = node + 0x14 every time.

`./kit fn 0x2f8820` clears the probe's record over its count, copies the probe's centre and radius
into its collidable sphere, and gathers (`0x218b80`, kind 3). For each candidate nearer than the
radius:
- type 2 (a mesh) with +0x31 & 1, and not in category VEHICLE|RIDER (`./kit data 0x43c108 16`):
  bundle_sweep_object. On a hit, `if (*y < record.point[0].y) *y = that`.
- type 3 (a placed group): it only sets the return value when a part is within 20. Nothing reads the
  return value.

This is put_back_solid's loop without the regather cache and the checkpoint test, which is why
`back_search::query_each` fits. A probe whose reach (1000) is past the search's bound always
regathers, so a fresh `Cache` per sweep matches.

## How it was proved

**Save-state capture on five tracks.** Each track's race file is read off the disc and stood on its
terrain. It is then raised onto the track's scene `.col` meshes plus its knockable props' meshes,
with nat03's ecosystem groups in the search too. The result is compared bit for bit with every point
of every node of the track object (`act+0xb8`) in the image:

| track | nodes | flagged | lifted |
|---|---|---|---|
| sx06 | 120 | 7 | 4 |
| sx08 | 104 | 7 | 4 |
| sx11 | 95 | 5 | 2 |
| sx20 | 93 | 18 | 7 |
| nat03 | 228 | 29 | 10 |

All 640 nodes match. The test also asserts that the raise only ever moves a flagged node's first
point, and only upward.

Two of the original's quirks showed up:
- **The knockable props are in the search.** With the scene meshes only, sx20 path 0's node 8 stayed
  at 89.999 while the image had 93.63. No scene mesh met any of its three sweeps.
  `bale_triple_01`, a knockable `.pco` prop, stands at (245, 92.1, 445) beside the node's **third**
  point. With the bales in the search the node matches.
- **The same address on all three sweeps.** The third point's sweep is what lifted the **first**
  point there. Only the original's same-address quirk explains that, and the port reproduces it.

## Not verified / not 1:1

- **The snap in the test uses the terrain object's query, not `world::Ground`.** sx11 path 1's node
  77 has its third point at z 765.0 in a 768-wide field. The original stands it at 89.97353, which is
  what `contact_search::ground` (the ported query) answers. `Ground::owns` refuses the last sample
  row and column, so `Ground` leaves that point at 0. That is a gap in S296's snap through `Ground`,
  not in the raise. The test asserts that `Ground` differs only in that strip (1 point over the five
  tracks). Filed as S318.
- **nat03's scene names 11 collision meshes neither of its archives has** (silo_spout_nat03.col,
  white_rail_fence_01..10.col). The port skips them. In the image the names appear only as scene
  text, never as loaded files, and nat03 matches with them left out. It is not proved that no raise
  would ever reach them.
- **The probe's collidable fields** (flags, its category mask against the props, `collides_with`) are
  `back_search`'s bike-bundle values, not read off the race load's probe. The images agree, but with
  only props and placed groups in the world, a filter difference would not show.
- **Animated toys' collision** (`toys::Toys::attach`, e.g. the sx finish gates and blimps) is not in
  the raise's props. No node on the five tracks needed it, and it is not known whether the toys are
  attached before the race load.
- **Placed groups crowding the 128-candidate list**: the gather's cap is ported, and nat03 ran with
  its groups. No case was seen where the cap cut off a prop.
- **No oracle** over generated inputs. The sweep and the gather are already oracle-proved by S83
  (back_search). This task's own logic (the probe, the fold, the first-point address) is proved by
  the five images: 66 flagged nodes, 27 lifts.
- **The AI net's file** (act+0xbc) is raised the same way in the original. The port has no reader
  for it yet (S297/S310), so it is not called on it.

## Next

None; FINAL.

## The gate's red atv_quad

`./kit done` was red in `atv_quad` `the_sweep_that_hits_the_palm_is_the_ports` (frame 124, 27 words
at 0x003dce50..). That test never reaches race.rs, and it failed the same way when run alone. The
MD119/MD118 merge on this branch's base left it red. Main fixed it in K51 (42d6c765e), which leaves
the original's triangle-test scratch out of the compare. K51's `sim_frame.rs` is taken unchanged:
its parent's copy of that file is the same as this base's. The plan and journal parts of K51 are
not taken.
