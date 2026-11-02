# S189 — bike nearby search

2026-11-01. Task: `./kit plan S189`.

The ridden bike now keeps one broad-phase search (`Ride::nearby`) as the original keeps its list on the
rider (+0x7bc), instead of gathering afresh on every wheel probe: gathered once a tick after the
substeps (in `rider.rs`, after the cars are shown again), around the body rays, refilled only when their
reach leaves the bound, and with a ghost's gather dropping category 0x40 (cars, toys, bikes). The wheel
probe, the body rays and the put-back sweep collide only against that list. S129's braked replay no longer
leaves the cars out of the scene: it seeds the list from the image (ghost-gathered, no cars) and rides with
the train in the world, with the same lane results as before. Details in the numbered file.
