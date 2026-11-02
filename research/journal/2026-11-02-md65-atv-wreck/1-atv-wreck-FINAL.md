# 1 — atv wreck (FINAL)

Task MD65. 2026-11-02.

## What I did

- **Split.** The brief covered four jobs: the wreck on WreckATVm.col, the put-back, the net unpack,
  and eight unpaired PC slots, each needing its own oracle. That is too much for one session, so I
  kept the put-back and the net unpack here and filed the other two with `plan add`:
  - MD65a (j: atv-wreck-body): the wreck body. `atv_quad_init` builds a part at quad +0x1434/+0x1438
    from WreckATVm.col, hung on the node at +0xe4, with part +0x88 = the quad.
  - MD65b (j: atv-slots): the unpaired slots.
- **Put-back.** I read the PC's `pc_quad_put_back` (vf55) first, then `./kit dis 4b8e30 -p atv`.
  - It matches the bike's override (0x004d8940) clause for clause, except that all four wheels are
    reset.
  - The flag byte +0x45 has a bit 0x2 that is cleared for the vehicle's put-back. If it was set, it
    is set again afterwards: the byte is read again and `& 0xfd | 2` is written.
  - Each hung node gets the first frame node's world frame (kind 0), the second's (kind 1), or neither.
  - The player's respawn copy is written only when there is a player and the first respawn word is
    non-zero.
  - The callees became `quad::Call` variants: `WheelReset`, `Detach`, `VehiclePutBack { place, keep,
    flags }`, `Reattach`, `ReadFrame`, `SetFrame { hung, frame }`, `NetPack` and `ActorMoved`.
- **Net unpack.** `pc_quad_net_unpack` was paired with vf104 (0x005d8bb0), but vf104 has no spin and
  no steering. It transforms the wheels' rest points by the body frame. `./kit xref 470420 -o pc`
  (`pc_wheel_suspension`) lists `MXVehicleAtv::vf70` (0x005dad30) and `Motocross::vf70`. Atv::vf70 has
  the PS2's clauses (snapshot, spin and wrap, steer filter, wheel rounds), so it is the twin. I fixed
  oracle.txt and research/mxvatv-pc.md. Then I ported the PS2 asm (0x004bdd20..):
  - The snapshot's velocity and orientation go into the body. Its position goes into the node position
    and the kept copy.
  - The centre of mass is the position plus body +0x1a0 turned by the new orientation, using the same
    VU0 rotate as `Body::to_world`.
  - Angular velocity is zeroed, and the speed is |velocity|.
  - The node is turned to the orientation.
  - Each wheel spins at the forward velocity in the body frame times its 1/r. The angle is integrated
    with adda/madd and wrapped: whole turns are truncated off with cvt.w.s, then one more 2π either way
    if the angle is still past ±π.
  - Steering: the node-local answer's `x` × 0.5 is clamped to the left front's ∓lock and lagged by the
    filter (fraction = min(dt, time)/time). The result goes to the left front, the quad and the right
    front.
  - Then wheel_clear, contacts, suspension and tire forces (the Force body without the integrate), the
    ground flags, and the vehicle's net post.
  - New `Quad::node_local` holds the node's answer, which the `Moto` writes (as `NodePosition` does).
- **Oracle.txt.** I added an `# MD65` block (field offsets in comments only) and names for the eleven
  callees.

## Checked

- `./kit check -- -p game-sim --test atv_quad`: 14 passed. I also ran it with cargo directly to make
  sure the two new tests ran and did not skip.
- `the_quads_put_back_is_the_originals`:
  - Inputs: 10,000 generated quads, each with a random flag byte, 0–11 hung nodes (some kinds outside
    0/1, including negative ones), and the respawn/player combinations.
  - Each callee is hooked and its arguments checked. The frame read writes a marker matrix, and the
    frame set must be handed one of the markers.
  - Every word of the quad, body, wheels and player matches, the call lists match, and nothing else is
    written. All 9 paths were seen ≥ 100 times.
- `the_quads_net_unpack_is_the_originals`:
  - Inputs: 10,000 states. Angles go to ±60 rad and land exactly on ±π. The node answer goes past both
    locks, dt falls on both sides of the filter time, and every touching mix is covered.
  - The node-local hook writes the generated answer.
  - Everything matches word for word. All 12 paths were seen ≥ 100 times.
- Mutations turned the tests red:
  - no hidden-bit restore
  - kinds 0..=2 hung
  - no respawn-word guard
  - `< -π` → `<= -π`
  - no `min(dt, time)`
  - front_down from `&&`
  - LR steered too
  - centre of mass not rotated

  Two survivors are expected, because neither changes the result for the generated inputs:
  - The clamp's max/min order: the two give different results only if the two ends of the lock cross.
  - `!(w <= π)` vs `w > π`: these differ only on NaN.
- `cargo check -p game-sim --tests`: no new warnings. `./kit audit`: only the existing
  plan/DONE.md:519/:533.

## Not verified / not 1:1

- The wreck body (MD65a) and the unpaired slots (MD65b) are not done here.
- The vehicle's own put-back (`atv_bike_put_back`), the wheel reset, the snapshot pack, and the
  node/actor callees are hooked, not ported. Their bodies are not checked.
- The hidden-bit restore reads the byte again after the vehicle's put-back. The hook does not change
  the byte, so a put-back that writes +0x45 is not exercised. The port restores from its own copy.
- Two words have no traced reader: what body +0xb0 holds (the node-local input), and what reads
  +0x141c (`PutBack::settle`).
- Steer filter time 0 and NaN inputs were not generated.
- Nothing calls `put_back` or `net_unpack` yet, and there is no capture or ride replay.
- `Motocross::vf70` probably pairs with the bike's net unpack 0x00259150 by the same xref. I have not
  checked it (noted in research/mxvatv-pc.md).
