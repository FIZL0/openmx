# 1 — net interpolate (FINAL)

Task MD65c2. 2026-11-03.

## What I did

- I read the quad's interpolation (`./kit dis -p atv 4e7bc0 --len 0x140`) and the biplane's that it ends
  in (`./kit dis -p atv 4a87c0 --len 0x480`). I checked that the slerp `./kit dis -p atv 134b90` has
  MX Unleashed's quat_slerp shape, then named it atv_quat_slerp.
- I peeked at the spawned image with a throwaway script (removed) to read the quad's pointers:
  - vt +0x3a8 is atv_quad_put_back.
  - The snapshot table *(+0xbcc) leads to quad +0xc40, +0x14b4, +0x14b8, +0xc80, +0x1528 and +0x940.
  - The lag times are 0.05 for position, 0.03 for velocity and 0.3 for the pose blends.
  - The world's area is 0..5760.
- Port: `quad::net_interpolate`, with the new types `quad::Snapshot`, `quad::Lag` and `quad::NetSmooth`,
  and the new call `Call::NetPutBack`.
  - It writes `Remote`'s position, orientation and velocity: the snapshot the net unpack reads.
  - It writes `Anim`'s two pose blends and its next stunt.
  - It writes `Quad::speed_len`.
- `base_animate` still hands `Call::NetInterpolate` to whoever runs the calls; this function is what that
  call means. That keeps the pattern `Call::NetUnpack` / `net_unpack` already uses.

## What the original does

- **The quad's half** reads the newer snapshot's +0x40 first, as the brief said.
  - Each pose blend is the older's at t ≤ 0, the newer's at t ≥ 1, and in between
    `adda(0, a) madd(t, b − a)`.
  - Each then runs through its lag: `min(dt, time)/time` of the way.
  - The newer's stunt id, wreck word and the +0x50 word are copied across.
- **The biplane's half:**
  - Position and velocity are lerped on VU0 with t unclamped. `w` comes from the newer snapshot.
  - The orientation is slerped the short way.
  - x and z are held 25 ft inside the world's area.
  - More than 8 ft from the last place: the position lags and velocity lags are *set* to the new values,
    skipping any FLT_MAX lane. The velocity lags are never run.
  - More than 16 ft: the quad is also put back there (+0xd40, vt +0x3a8).
  - Then the position goes through its lags into the snapshot, with `w` = 1.
  - The velocity's change is kept, the velocity goes in with `w` 0, and its length goes to +0x32c.
  - `dt` reaches both halves in f12, untouched by the net check.

## How it was proved

Function oracle on MD37a's atv-r-spawn.ee.bin. Only the put-back is hooked; the biplane's half and the
slerp run unhooked on the image's arc cosine table, which the test checks entry by entry. Every word
either side can write is compared by bits, and the original may write nothing else but its stack.

- `the_quads_net_interpolation_is_the_originals`: 20,000 generated cases on a laid-out set of pointers.
  These 13 paths are each seen at least 200 times:
  - t ≤ 0, t between, t ≥ 1;
  - x held inside, z held inside;
  - near, lags set, put back;
  - a FLT_MAX lane skipped;
  - parallel, short-way and plain slerps;
  - a combined path.
- `the_spawned_quads_net_interpolation_is_the_ports`: 2,000 cases on the spawned quad's own pointers,
  lags and area. Here bcc[0] aliases quad +0xc40.
- Hand mutations, one at a time:

  | Mutation | Result |
  |---|---|
  | slerp the long way | red |
  | no area clamp | red |
  | velocity `w` kept | red |
  | far threshold at 15 | red |
  | the lerp written `a + t·(b − a)` instead of through the accumulator | still green |

  The last one is expected: the PS2 FPU's madd rounds the product before adding, so the two give the
  same bits for normal values. The port keeps the original's shape anyway.
- `cargo test --release -p game-sim --test atv_quad animate` still passes.

## Not verified / not 1:1

- No live capture of a remote quad (no 2-player network session was run). The inputs are generated.
- The +0x50 / +0x940 word is copied but not traced.
- The order of writes when snapshot pointers alias each other is not checked. The image's pointers do
  not alias each other, and the port keeps each output in its own field.
- The put-back is a call. It runs between setting the lags and running them, and whoever answers
  `Call::NetPutBack` with `quad::put_back` must leave `NetSmooth` alone, because the original reads the
  lags again after the put-back.

## Next

Nothing for this task. Whoever wires the remote quad into a ride answers `Call::NetInterpolate` with this
function and `Call::NetPutBack` with `quad::put_back(.., false, false, ..)`.
