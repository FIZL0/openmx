# 2 — the ATV's wheel suspension and tire forces (PART)

Task MD37b, reopened after MD62–MD65 landed. 2026-11-02.

## What I did

- Read the reopened brief against the tree again. MD62–MD65 ported the quad's listeners, update,
  put-back and net unpack, but a captured ATV ride still cannot replay: there is no SLUS-21104 physics
  frame (world step, rigid-body integrate) in game-sim, no `quad::Moto` that runs the bike steps, and no
  Desert Oasis ground. And the two wheel steps the quad's Force body runs four times a substep,
  `atv_wheel_suspension` and `atv_wheel_tire_forces`, are not MX Unleashed's (`wheel::suspension`,
  `wheel::tire_forces`): other offsets, no transform object, a live bottom-out. So I ported those two
  (the largest self-contained piece left) and filed the rest.
- `crates/game-sim/src/atv_wheel.rs` (new, `pub mod atv_wheel` in lib.rs): `suspension`,
  `tire_forces`, `add_force_at` (the rigid body's vt +0x84), on the existing `wheel::Wheel` plus
  `atv_wheel::Body` (the rigid body as the wheels see it) and `More` (two wheel words `Wheel` has no
  field for). Callees go to closures as `wheel::Call`s.
- `crates/game-sim/tests/atv_wheel.rs`: two oracles off MD37a's `atv-r-spawn.ee.bin`.
- research/oracle.txt: a `# MD37b (wheels)` block (field map, constants) and rows atv_ground_query,
  atv_wheel_contact_velocity, atv_wheel_hinge_force, atv_body_add_force_at.
- Filed MD74 (contact velocity and hinge), MD75 (the quad's physics frame and a real `quad::Moto`),
  MD76 (Desert Oasis's ground query for the replay).

## What the original does

Read from the asm (`./kit dis 0x004a0cb0 --len 0x4a0 -p atv`, `./kit dis 0x004a1150 --len 0xe00 -p atv`);
Ghidra cuts the suspension short (it thinks the ground query does not return).

- **Suspension**: ray_hit = 0; no world, done. mount_world = turn(mount − cofg) by the body's quaternion
  + position. Up to 5 aims while the agreement < 0x3f7f069e (cos 5°, a literal — MX Unleashed reads it
  off a curve table): aim = the ground normal into the body, x zeroed, normalised (vrsqrt); probe =
  aim × −length; probe end = mount + probe, into the world; the ground query on a copy of it; the
  normal normalised, agreement = its dot with the old ground normal, which it replaces. Then: the
  answer below the probe's end → a miss (a swept contact's record byte +0x110 zeroed). No swept contact
  → ray hit, contact = the answer. Swept: the suspension line against the swept normal under
  0x3f34fdf4 (cos 45°) → the terrain wins; else the sticky `terrain_wins` flips only when the other
  surface is 0.1 higher; winning terrain replaces the contact and clears swept.
- **Tire forces**: the same mount/probe rebuild; along = (cp − pe)·n / (down·n) unless down·n ≤ 1e-6;
  the load point and compression = |load − pe|; the load scale is MX Unleashed's (0.5..0.64 →
  0.5..1.0). The contact velocity is asked once, and again after a bottom-out. Bottom-out (ray hit, or
  swept with along < 2): the body's position += n × 0.9 × overlap (MX Unleashed computes this push and
  drops it; SLUS-21104 applies it), the body's +0xd0 −= its up turned to the world × min(chassis
  velocity·n, 0), and −min(force·n, 0) along n through vt +0x80 (= add_force); +0xc0 takes n × 0 when
  that is longer (never). Damper picks out (+0x4c) unless closing ≤ 0; spring force = max(0, k·c −
  d·closing); steered and balancing → the hinge, else add_force_at. Unloaded → spring force 0, contact
  velocity (0,0,0,1). The extension limit and last compression as MX Unleashed's.
- **add_force_at**: arm = point − position; cross by opmula/opmsub; into the body (the same
  quaternion rotate as `quad::Body::to_local`), × the scale (1,1,1,1) lane by lane, vt +0x78; then
  vt +0x48.

## How it was proved

`./kit check -- -p game-sim --test atv_wheel --test atv_quad --test wheel`: 19 passed. The two new
tests ran (no skip line under `--nocapture`). Each runs 10,000 generated states through the original
with the body's real vtable (add-force, add-moment and add-force-at run unhooked), the ground query,
contact velocity and hinge hooked (arguments checked, calls logged, generated answers served), and
compares every word of the wheel (0x8b0) and body (0x200) and the call list; nothing else may be
written. Every path ≥ 100 states: suspension — no world, 1 / 2–4 / 5 aims, miss, miss under a swept
contact, ray hit, terrain won / kept, swept kept / kept with the flag dropped, an answer exactly level
with the probe's end; tire forces — no contact, no load, loaded, sprung, bottomed out with and without
the shove, hinge, body, zero and non-zero spring force, the contact exactly on the probe's end, the
compression exactly at the limit.

Mutations that turn it red: push 0.9 → 1.0, no correction, 4 aims, steered-only hinge, no record
clear, no load-scale floor, damper side flipped, cross turned to the world, margin 0.11, `<=` → `<` on
either margin, miss on `<=`, the 1e-6 guard at 0, along `>= 0`, compression-at-limit `>=`, compression
`>= 0`, no `min(.., 0)` on either closing or pressing, no x-zero on the aim, unconditional +0xc0, no
last-compression clamp, no extension limit. Survivors, equivalent on these inputs: along `< 2` vs `<= 2`
(no exact tie generated), `!(a <= b)` vs `a > b` (NaN only), the cos 45° literal moved 4 ulps.

## Not verified / not 1:1

- **No ride replay** still: MD74, MD75, MD76 stand between this and the brief's test. MD37b is blocked
  after them.
- "Every bike replay hash unchanged" holds trivially: nothing the bike runs calls atv_wheel.rs.
- The ground query's normal `w` is whatever the query leaves; the port takes the query's whole quadword.
- `More::record` is only the record's first byte; the rest of the record (what the query writes through
  t0) belongs to MD76.
