# S279b — rider scene capture

2026-11-14. Task: `./kit plan S279b`.

A capture where MX vs. ATV's rider parts hit the scene, and the side-by-side bone-collision replay on it.
The tape `research/c15/scene.pad` (from state 4: gas, full left ticks 5..30, then straight on) runs the
bike into a nat05 mesh on row 188. Captured twice, identical, 300 rows. `the_bone_collisions_scene_is_the_ports`
replays it bit-exact on the original's scene and on the port's: 1,716 sphere pairs and 1,313 sweep pairs
with meshes, 209 sphere contacts and 15 swept push-outs, nothing handed to the original. The only parting
was the original's mesh sweep's scratchpad matrix stack (dead levels it pushes and pops). Plants, boxes,
movers and records with more than one contact are still not seen live.
