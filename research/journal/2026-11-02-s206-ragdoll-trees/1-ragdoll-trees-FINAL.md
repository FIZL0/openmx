# 1 — the ragdoll's bones against the trees (FINAL)

## What the original does

The bones' search (body actor +0x628) has no_groups 0. The bone spheres' flags (+0x116: 0x12, 0x52 or
0x92) have bit 0x01 clear. So when collide_against_search walks the list, a placed group is not
skipped: each of its parts (group_part_count / group_part) goes through the sphere's pair unfiltered.
The only gate is the group's list distance against the sphere's radius. That distance is measured on
the ground plane from the plant's cell, less its reach, and clamped at 0 (the broad phase's
group_distance).

What else the search lists, on s89-along's free06 tree:

| entry | count | does a bone meet it? |
|---|---|---|
| class-3 groups | 5600 | no: all have 0 parts (bushes); they only take list room |
| class-1 entries, vtable 0 | 612 | no: the gather skips them |
| category 0x100 trigger boxes | 12 | no: listed, but sphere_collider_pair has no box (kind 6) test |
| category 0x18 meshes, +0x80 set | 6 | **yes**: the knockable props at rest. The filter passes, the pair tests them as meshes (F7) |
| 0x9a058 mesh | 1 | another rider's wreck bike, not this task |

On recording2's free09, 630 of the 726 groups have one part. That part is a class-2 mesh, category 4,
+0x31 0x3, +0x7c 0, +0x80 0: a still trunk, pushed out on the static path, exactly the port's trunk
Prop in eco::Trees. The collidable is one per species, which group_part moves under each plant.
free09 also has one hierarchy mesh (the utility shack). The port's Props don't load hierarchies.

## What I did

- `ragdoll_scene.rs`: Statics.props now also holds plants, as indices past the world's props.
  `Statics::group_keys` files the groups whose distance from the probe is under the largest bound the
  search can have (max(extra, reach, least)). The others can never be listed, so leaving them out keeps
  the walk order and saves filing 5600 bushes on every regather. `BikeSearch::gather_wooded` lists by
  `group_distance` for a plant. `StillProp::near_wooded` hands a listed plant's trunks to the sphere at
  the plant's place in the list, at the plant's distance. A contact on a trunk is the still scenery.
- `ragdoll_ride.rs`: step_posed_with reads `world.wooded()` for the gather and the scene.
- The snapshot and hash types are unchanged. The hash already writes every listed index.

## How it was proved

`tests/ragdoll_trees.rs`, on recording2.p2m2_SaveState.eeMemory.bin and the disc's free09:
- 6,000 gathers (3,000 fresh round a tree or a prop, 3,000 kept and measured again) give the same
  groups and meshes, in order, with the same distance bits, centre and bound as
  collision_search_gather. With the plants' keys left out, it fails on case 1.
- 20,000 spheres near trunk vertices, through collide_against_search with plant / plant + neighbour /
  plant twice / prop + plant lists, match the port's trunks bit for bit (count, with, held, point,
  normal). There were 8,365 hits, and 9,755 entries were gated out by distance.
- A ragdoll thrown at 25 ft/s at a lone tree stops about 2.1 ft from the trunk's axis. Without the
  plants it goes through.
- `./kit check -- -p game-sim`: 966 tests passed. The workspace builds.

## Not verified / not 1:1

- **List capacity.** The original's list also holds the bushes, trigger boxes, knockable props,
  bikes and the shack, which the port doesn't model. So near 128 entries the two lists can differ.
  The test skips full lists; none of its 6,000 was full (at most 12 entries).
- **Knockable props at rest** (category 0x18) are tested by the original's bones and not by the
  port's: F7.
- **free09's shack hierarchy** is in the original's tree and not in the port's Props, so a bone goes
  through it here.
- A plant's +0x12 flag bits were not read for any skip the original might make per plant. Every
  free09 plant with a part behaved the same in the sphere oracle.
- MX vs. ATV packs' plants with sphere solids give `None` parts, which no bone meets. Not checked
  against that game.
- There is no capture of the original's ragdoll against a tree. The ride test is only the port's.
- The `./kit audit` leak at plan/DONE.md:519 (P256's line) is not this task's.

## Next

F7: the knockable props at rest in the bones' scene.
