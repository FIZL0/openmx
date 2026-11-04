MD75c3: a scene node's bounds gathered again

2026-11-04. Task: `./kit plan MD75c3`.

MD75c (`6-post-callees.md`) left the vehicle post's node vt `+0x94` (`atv_node_rebound`) hooked as
`quad::Call::NodeRebound`. This task ports it as `atv_moto::Scene::rebound`. `quad::Steps` answers the
call with it, and `Quad`'s node box (`node_box`) runs it inline when the bounds are stale, then clears bit 7.
Field notes are in `research/oracle.txt` under "MD75c3".

## What the original does

The rebound brings the node's world frame up to date. Its flag byte (`+0x100`) picks one of two shapes:
bit 4 means a sphere, otherwise a box. Bit 2 means the node has bounds of its own. Its list (`+0x10c` count,
`+0x144` pointers) names the nodes whose bounds it gathers. Each listed node goes through one helper,
`atv_node_sphere_add` or `atv_node_box_add`, with recursion off. A listed node without bit 2 is passed over.

- **Sphere.** It starts from the node's own centre (`+0xb0`) carried into the world (`atv_node_place`)
  with its radius (`+0x104`), or from radius 0. A listed node's centre and radius replace a radius not
  above 0. Otherwise the radius grows to that centre's farthest lane from the current one, plus its
  radius, if that is more. The centre goes back into the node's frame (`atv_node_unplace`) into `+0xd0`.
  The half extent's `xyz` and the reach are the radius.
- **Box.** The corners start at `vf0`, or at the node's own box (`+0xb0` ± `+0xc0`). A listed node's
  box goes through its world frame times the inverse of the node's. The inverse is built on the
  scratchpad at `0x70000000`. It is taken as `Scene::world_box` takes a box, so its corners carry the
  `w`s of that frame's rows 0 and 1. After the first box, the corners are `vmini`/`vmax`'d with the
  ones so far, on all four lanes. The centre and half extent are half the corners' sum and difference.
  The reach is the half extent's length.

Either way, the reach goes to `+0x108`, and a reach above 0 sets bit 6.

**A sphere with nothing to start from has no centre.** If neither the node nor any listed node has bounds
of its own, the centre is a stack word the rebound never writes. The port takes zero there, and the oracle
does not generate that case (`no_stack_centre`).

**`vmini`/`vmax` put `-0` below `+0`.** ps2-oracle's VU compares the bit patterns in sign-magnitude
order. `ps2_float::vu::mini`/`max` hand back the first operand on a ±0 tie. The oracle's state 109
showed the difference: a box corner `w` of `-0` met `vf0`'s `+0` and the port kept `+0`. The port uses a
local `atv_moto::pick` and leaves the shared crate alone. MD94 is filed to settle the shared crate
against the hardware.

## The image

The image's quad node (`0x0159c290`) has flags `0xc1`. It is a box with no bounds of its own, and its
bounds are not stale at the spawn. Its list holds 43 rider-skeleton nodes (flags `0xa5`, each with bit 2).
That is past `atv_moto::NODES`, so `read_ride` lists `+0x10c` and `+0x144` as gaps. Shorter lists are
read into `Node::list` and poked back. The whole-frame scene is MD75e's.

`context/mxvatv-ps2/md37a/atv-r-spawn.ee.bin` was missing from every slot. I rebuilt it from the
`eeMemory.bin` of copy 2's savestate slot 1 (`s2-slots/pcsx2/s2/.../SLUS-21104 (95BB1901).01.p2s`),
which is MD37a's slot for atv-r-spawn. The image read-back test and c001 pass on it.

## The proof

`tests/atv_quad.rs` `the_quads_node_rebound_is_the_originals` checks 10,000 generated states (seed
`0x0075_c301`, md75c's scene). Each node gets an own centre, half extent and radius (a quarter of the
radii are at or below 0), a list of 0 to 4 nodes (any node, itself included), and node 0's bits 2 and 4 at
random, stale. The hooked stand-in jumps to the original's rebound. Every region is equal, the scene's
words included, and each path (sphere/box × own/not × listed or not) runs at least 300 times.

The other tests that send the rebound inline (the vehicle post, b104, md75b1/c) now unhook it too
(`POST_STEPS`), and c001 no longer wants it as a call. All 61 atv_quad tests pass, as do atv_wheel,
atv_spatial, atv_search, broad_phase, flight_candidates, mxvatv_bike_ride, mxvatv_step and
untamed_reworked.

I also ran mutants, each against the rebound oracle:

| mutant | result |
|---|---|
| no `vabs` on the sphere's distance | red, state 47 |
| `Vf::mini`/`max` for `pick` | red, state 109 (`-0` vs `+0`) |
| no corner `mini`/`max` | red, state 1 |
| `r < 0` for `r <= 0` | red, state 0 |
| centre `w` the high corner's | red, state 1 |
| corners without the rows' `w`s | red, state 1 |
| `far > r` for `!(far <= r)` | green: the same with no NaN, which the EE FPU does not make |
