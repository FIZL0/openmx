# 1 — rear dust follows the swingarm (FINAL)

Task S421. 2026-11-18.

## What I did

- `crates/game-sim/src/dust.rs`:
  - New `RearNode` (Bike's local, SwingArm's pivot, the rear mount, the emitter node's local).
    `swingarm(c)` is the original's aim. `point(root, c)` runs the node tree with
    `atv_moto::frame_mul` and adds the zero offset.
  - `Emitter::at` is now `place: Place`: `Chassis(at)` as before, or `Rear` (the rider's
    `RiderDust::rear`). The rear point is worked out once a frame.
  - `Feed::rear_compression`: the swingarm's compression, lagged a frame like the rest of the feed.
    `Sim::dust_frame` fills it from `ride.rear.w.compression`.
- `tests/ai/race4.rs`:
  - `c33_rear_node` reads the chain off the image.
  - The seed puts every emitter on `RearTire_Particle01` at `Place::Rear`. It asserts that the
    image's point equals `RearNode::point` of the seeded transform, bit for bit.
  - The C33_DUST check is unchanged and green.
- `tests/ai/draws.rs` (`s328_c33s_other_draws_are_looks_notes_and_dust`): the emit hook checks each
  rear emitter per tick (below).
- `research/oracle.txt`: the S421 block (node rebuild, the drive's aim, the lag).
- Follow-ups: S446 (front and exhaust chains), S447 (Bike's pose in the Sim, `later`).
- opentennis: nothing like it there. `frame_mul` and `math::basis` were already in this crate,
  both proved.

## What the original does

- emit_timing `./kit fn 0x0014daa0`: the previous point is set from `+0x40`. Then
  `./kit fn 0x001b2870` (node, 0, `+0x40`) runs `./kit fn 0x001b1750`. That rebuilds the dirty node
  worlds from the first clean ancestor down. Then `+0x40` = the node's world row 3 with `w` 1, then
  `vadd.xyz` the offset `+0x70`.
- The world rebuild:
  - Rows 0–2: `vmulax`/`vmadday`/`vmaddz` of the parent's rows by the local row.
  - Row 3: the same, but started with `vaddax` of the parent's row 3 and 0.
  - This is `atv_moto::frame_mul`.
- The root (`bike\anim\bike.psg`) is the chassis' `+0x254` node. Its world is clean when the
  emitters run, and nothing moves it later in the frame.
- The drive `./kit dis 0x00259ccc 0x00259d68`:
  - axle = (mount.x, mount.y `add.s` `+0x87c`, mount.z).
  - d = pivot `+0x9c0` − axle (`vsub.xyz`).
  - |d|² by `vmul.xyz`, `vadday.x`, `vmaddz.x` against 1. Then `vrsqrt` and `vmulq.xyz`.
  - `./kit fn 0x001b1d70` (SwingArm, d, (0,1,0,1), 1, 1, 1): `mat4_basis` at the node's own
    position, orthogonalised, forward kept.
- `+0x87c` (the vehicle's animation snapshot) holds, while the emitters run, the rear wheel's
  `+0xf0` from the end of the frame before.
- Bike's local (vehicle `+0x964`) comes from the chassis pose. It is the same for all four riders,
  translation constant. Its rotation changes in the last bits at ticks 7, 9, 14, 18 and 64.

## How it was proved

- **Ee, the original's whole frame, C33, 60 ticks** (`tests/ai/draws.rs`
  `s328_c33s_other_draws_are_looks_notes_and_dust`). For each of the 4 riders × 4 rear emitters, on
  every tick (3840 points):
  - The port's `swingarm(+0x87c)` equals SwingArm's local, all 16 lanes.
  - `point` of the root's world, Bike's local and `+0x87c` (all read as the original emits) equals
    the emitter's `+0x40` after the emit.
  - `+0x87c` is the rear wheel's `+0xf0` of the frame before.
  - The root's world is the same at the end of the frame.
  - The root is the chassis' `+0x254` node.
- **The Sim** (`tests/ai/race4.rs` `s12e_free09s_four_racers_ride_as_the_capture`): the seeded
  rear point equals the image's bit for bit. C33_DUST still matches on every checked tick. Both
  tests pass under `./kit check`.

## Not verified / not 1:1

- **Bike's local is held at the image's.** The original's comes from the chassis clip and moves by a
  few ulps (C33 ticks 7, 9, 14, 18, 64). From tick 7 the Sim's rear points can differ from the
  original's in the last bits. The chain is proved only when it is fed the original's Bike local. In
  the Sim no tick's rear point was compared bit for bit after the seed. Why: the bike's clip sampling
  is not in the Sim (S447).
- **Front sprays and exhaust are still fixed places in the chassis frame.** Their chains (Shocks
  under Handlebars; the exhaust pipe's own model) are not ported. C33's front sprays never draw. The
  exhaust matched its table. S446.
- **The emitter offset** (`+0x70`, and the offset turned by the node when `+0x240` bit 5 is set) is
  not modelled beyond adding 0. Every C33 emitter has a zero offset (the seed asserts it).
- **SwingArm's position is taken as its pivot `+0x9c0`.** The aim keeps whatever position the pose
  left. In C33 the two are equal (the seed asserts it). A clip that moves SwingArm is not handled.
- **Only C33 seeds a rear node.** No live shell race builds `Sim::dust` yet (S420).

## Next

None.
