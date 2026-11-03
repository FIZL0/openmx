# MD75 — the quad's physics frame

2026-11-02. Task: `./kit plan MD75`.

This is part 1 of MD75. SLUS-21104's rigid body (its integrate and its end of substep) is ported as
`atv_body::Body`, and the physics world's frame over the quad is `quad::frame`. In `tests/atv_quad.rs`,
two new oracles run against the original's own code off MD37a's `atv-r-spawn.ee.bin`:

- the body step, over 10,000 generated bodies;
- the world step on a world of the quad alone, over 3,000 states, with every Moto call hooked.

Still to do: the Moto that runs the bike steps in place of each `Call`, the steps not yet ported, and the
brief's whole-frame proof. They are filed as MD75 subtasks. Details are in the numbered file.

MD75a (`2-moto-steps.md`): `quad::Steps` runs the ported bike steps (input limiter, clutch, preload,
backflip, airborne control, and atv_wheel's suspension/tire forces/wheel integrate) on the quad. Ten
oracles prove it, each with that call unhooked so the original's own code runs, every region equal.
The rear-brake gap in `step::input_limiter` is filed as B17.

MD75b (`3-moto-own-calls.md`): the quad's own calls (engine update, rev limit, auto shift, head and tail,
gravity, air drag, peak rates and wheel clear) are ported in `atv_moto.rs` and answered in `quad::Steps`.
Each is unhooked in its own oracle, plus two combined oracles, and every region is equal. Spin air,
vehicle post and node position are filed as MD75b1.

MD75b1 (`4-moto-spin-post-node.md`): spin air (the 360), vehicle post (origin, world test, node orientation
and the post's callees) and node position (the world update over a four-node scene graph) are ported in
`atv_moto.rs` and answered in `quad::Steps`. Each is unhooked in its own oracle, plus one with both
PostProcess calls, and every region, the scene included, is equal.

MD75, part 5 (`5-wheel-spin.md`): the wheel integrate's surface drag and spin are ported in `atv_wheel.rs`
and proved alone in `tests/atv_wheel.rs`. `quad::Steps` runs them inside the wheel integrate.
`tests/atv_quad.rs` unhooks all three under the Force body: 10,000 states, every region equal. Still
open: the whole-frame proof, which is filed as follow-ups.

MD75d (`7-image-ride.md`): `tests/atv_quad/image.rs`'s `read_ride` reads the spawned quad out of
`atv-r-spawn.ee.bin` as a `Ride`, with the gaps it cannot carry listed (the scene past four nodes above all).
Poked back at the harness's layout, all 2,654 words the poke writes equal the image's.

MD75c1 (`8-spatial-move.md`): `atv_spatial_move` is ported in `atv_spatial.rs`. That covers the portal walk,
the list sub-database's virtuals, the link pool and the overlaps. `atv_spatial::Answers` answers
`quad::Call::SpatialMove`. `tests/atv_spatial.rs` checks it against the original's move in two ways: on
12,000 generated databases laid in the image's free RAM, and on the image's own database, run once as it
is and once with a portal laid in. Everything read back is equal. The quadtree sub-database is not ported.

MD89 (`8-spatial-move.md`, "MD89"): the quadtree sub-database's pick/in/move/out are ported as
`atv_spatial::Tree`. A third of the 12,000 generated cases now file the mover in trees (nodes made and
freed, chunks freed, heights widened, queries closed, the entry's halfword cleared). The image's own
4,264-node tree is moved in by the three vehicles' spheres. Everything read back is equal.

MD75c2 (`9-part-sync.md`): `atv_part_sync` is ported in `atv_sync.rs`, covering the node frame, the ray
probe's and the mesh's stands, and both boxes. `quad::Steps` answers `Call::PartSync` and sends the box on as
`Call::PartMove`, which `atv_spatial::Answers` moves. `Oasis` answers `Call::Gather` round the active part.
Two tests check it against the original. One uses 10,000 generated parts laid whole. The other uses the
image's own probe and mesh on Desert Oasis, where the nearby list matches entry for entry.
