# 1 — nat05 prop world (FINAL)

Task F34. 2026-11-16.

## What I did

- `game_sim::atv_props` (new): `build` makes SLUS-21104's Free Ride world over a map from its files —
  `Env\<map>\<map>.scn` + `Col\*.col` and the `.toy` out of the map's `.res`, the plant groups of the
  `.esb` — with `atv_world` (MD76a1's builder), in the original's order. `Rays` is the bike's ray
  probes' narrow phase (meshes and plant parts by the quad's `Wheels`, zones by `atv_part`'s sphere and
  box tests, anything else nothing struck); `search` stands the reach and runs `atv_search::search`.
- `track::Scene::parse_with_taken_out`: the Free Ride files the shapes its activity takes out (MD13)
  all the same, with the "nearby" bit off. On Nat05 that is 24 AirRace shapes (not 26 as guessed).
- `tests/mxvatv_bike_props.rs`: three tests against state 4 (`context/mxvatv-ps2/state04.ee.bin`).

## What the original does

- The Free Ride's world is MX vs. ATV's quadtree: scene meshes, plant groups, the toy's trigger
  zones (7 spheres, 1 box on Nat05), then each start zone's mesh filed again (5 on Nat05). Taken-out
  shapes are filed with their nearby bit clear: the gather lists them, no search strikes them.
- The bike's three probes (bike `+0x1430`, `+0x1434`, `+0x1438`) are all ray probes of one vtable
  (`0x3b6c20`): the trigger probe (2 rays, 3 contacts a ray), the record's wreck part (11 rays) and
  riding part (14 rays, which is also the gather probe), 1 contact a ray. The pair by the other's kind:
  0 a mesh, 5 a sphere and 6 a box (hit bit, no contact), else nothing. Every pair resets the record's
  first (`+0x80`) to 1.
- A trigger zone's struck callback (`+0x120`, `0x1a8000`) writes the probe into the zone's
  `more_objects` (mode 1: excluded) and the zone into the probe's, so the next search passes over it.
  That is the ride's state, not the world's: the image's zones carry empty lists at load.
- Zones' fields: sphere radius `+0x180`, radius² `+0x184` (`mul.s`); box frame `+0x130`, centre
  `+0x170` (origin), half sides `+0x180`.

## How it was proved

Interpreter oracle on state 4 (gather `0x1d6a60`, search `0x1151c0`, stand `0x1bcfe0`):

- `nat05s_world_is_state_4s`: all 3213 tree nodes and 20021 entries equal; 391 collidables' fields
  (flags, nearby, reach, categories, object lists, centre); the moving list slot for slot; 8 zones'
  shapes bit for bit.
- `the_bikes_probes_are_the_images`: vtable, flags, record capacity, centre and reach; the trigger
  probe's tree equals `kiowa::ray_tree`.
- `the_bikes_probes_search_nat05_as_the_original`: per probe 1459 frames (6 per mesh aimed to put one
  ray across it, 1 in 8 tipped anywhere, plus the image's own frame). Gathered list and record word for
  word on every frame; contacts 927 / 4058 / 5355 on 203 / 219 / 219 meshes. 14 s.

## Not verified / not 1:1

- Plant spheres: Nat05's plants have no trunks (`Trees::props` is empty), only spheres. The port pairs
  them with nothing struck, so the probe's hit bit differs where a plant sphere alone was struck
  (9 / 11 / 7 frames). Records match. Needs the esb sphere built and stood per plant (filed).
- The ride's own collidables (vehicles' spheres, riders' and machines' parts in the moving list) are
  copied from the image and retired on both sides; the searches never pair them.
- The zone callback is not ported: before each search the test copies the image's lists into the
  port. The port's world alone would search a zone again after it was struck.
- The wreck and riding probes' ray trees are read from the image: `kiowa::ray_tree` panics on rays
  that all start at one point.
- No full ride into a prop: C15's rides file no contact (F31c), so frames are aimed instead.
- The record's sweep probe (record `+0x204`, kind 3) over the file-built world is not run (filed).
- Animated toys and physics collections in Nat05.toy are not filed; state 4's tree has none.

## Next

- F69 (plant spheres) and F70 (the sweep probe over this world).
