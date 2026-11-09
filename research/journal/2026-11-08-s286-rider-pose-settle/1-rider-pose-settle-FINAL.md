# 1 — rider pose settle (FINAL)

Task S286. 2026-11-08.

## What I did

- `game_sim::handling::rider::pose(m, rider, dt, still, acos, curve)` and `rider::settle(m, rider)`,
  appended to `rider.rs` in a section of their own (S286). `Call::Pose`/`Call::Settle` now name them.
- The pose reuses `ragdoll_pose::pose`, the MX Unleashed port, for each body holder. The inputs are read
  off RAM: the node's world row 3 with `w` 1, rows 2 and 1 with `w` 0 (what `node::position`/`node::axes`
  hand out, after `node::update`), the holder's drawn frame `+0x60` and the body's `+0x1a0`. The outputs
  go back to RAM: the body's `+0x40`/`+0x50`/`+0x180` (word by word)/`+0x160`, then the drawn frame.
- The settle walks the world's body list. Each body's vt `+0x3c` is `wreck::body_commit`, run through a
  small adapter (`Bodies`) because `body_commit` takes `Wrecks` rather than `Ram`. The commit never calls
  `out`/`normal`, so the adapter answers them with `unreachable!`.
- Oracle: `tests/mxvatv_rider_modes.rs` `the_riders_pose_and_settle_are_the_originals`.
- opentennis has no ragdoll, so nothing was taken from it. The shape came from this repo's
  `ragdoll_pose.rs` (MX Unleashed) and `wreck::body_commit`.

## What the original does

Read with `./kit dis 0x00505720 0x00505b20 -p atv` and `./kit dis 0x00505710 --len 8 -p atv`. Neither is
a Ghidra function. The body slots were read with `./kit data 0x003cd200 0x70 -p atv` and `./kit dis`.

- Pose (a0 rider, a1 still, f12 dt): with `dt <= 0` (`c.ole.s`, so an unordered dt divides) it is still;
  otherwise `f20 = 1/dt`. For each holder i, while i < `+0x290` (re-read after each holder): holder =
  `*(+0x2a4) + 0xb0 i`, node `+0xa0`, body `+0xa4`. The position call puts the node's position at
  sp+416, the axes call puts z at sp+368 and y at sp+352. The quaternion comes from basis(origin, z, y,
  0, 1) through mat4_to_quat, then gets negated (VU `0 - q`) if `w < 0`. Still: velocity = rates =
  `(0,0,0,1)`. Moving: last = drawn row 3 `xyz` with `w` 1, and prev = mat4_to_quat(drawn), negated the
  same way. The velocity is (at − last)·(1/dt) in `xyz`. The relative quaternion goes through unturn and
  axis-angle (the acos table, then the sine-table entry, angle × 2; an acos of 0 gives axis `(1,0,0,1)`),
  so the rates are axis × angle·(1/dt). Then at += turned(q, body `+0x1a0`). The body's vt `+0x10`
  (`+0x40` = at), `+0x14` (`+0x50` = velocity) and `+0x58` (`+0x180` = q word by word, `+0x160` =
  rates) store them, and the drawn frame becomes basis(at, z, y, 0, 1). MX Unleashed's version has the
  same blocks in the same operand order. I checked each VU sequence against `relative`, `unturn` and
  `wreck::turned`.
- Settle: a0 = `*(rider+0x27c)`, then a jump to the world's settle. If count `+8` > 0, a loop over the
  bodies `*(*(world+0x10) + 4i)` re-reads the count after each body. Each body's vt (`+0x110`) `+0x3c`
  is the commit: `+0x60`, `+0x70` = `+0x40`; `+0x80` = `+0x50`; the accumulators get `(0,0,0,1)`;
  `+0x170`, `+0x190` = `+0x180`; `+0x1d0` = `+0x160`; `+0x70` += turned(`+0x190`, `+0x1a0`);
  `+0x60` = `+0x70`; `+0x1b0`..`+0x200` get `(0,0,0,1)`.

## How it was proved

This is a function oracle over 12,000 generated cases on state 4's rider image (`context/mxvatv-ps2/state04`,
18 holders, the world 0x010cc6f0 with 18 bodies). Each case runs the original's 0x00505720 and 0x00505710
whole in the interpreter (no hooks but the kernel's), then the port from the same words. Every word either
side writes, off the stack, comes out equal. The settle runs on top of the pose's writes, as in the
respawn. Each case draws:

- each holder's node: current (a random world frame) or stale (a random own frame, rebuilt from the image's
  parents);
- the frame's rows: rotations, sometimes skewed or with noise or signed zeros;
- the drawn frame: random, or exactly the node's axes' basis (no turn);
- the offset;
- each body's staged words;
- the counts: the real one, 0 or −1;
- dt: the step, 0, −0, negative, 1e-6 or random small;
- still: 1 in 5.

Path counts from one run:

| Path | Cases |
|---|---|
| moving | 4,346 |
| still | 1,705 |
| dt <= 0 | 2,591 |
| no-turn drawn | 4,231 |
| acos-zero axis | 919 |
| q flipped | 8,622 |
| pose, no words (count 0/−1) | 3,358 |
| settle, no words | 3,360 |

The drawn quaternion's flip is tallied too. The test requires each path by name, at least 20 times.

Mutant check: 1/dt changed to 1/(dt+1e-7) in the port, and the oracle fails.
The S278b mode-machine oracle still passes.

## Not verified / not 1:1

- **The body virtuals are not dispatched.** The port writes the three setters' and the commit's words
  directly, as the ragdoll body class (vtable 0x003cd200) does. The test asserts every one of state 4's
  bodies has exactly those slots. A body of another class would take the original elsewhere.
- **The tables are built, not read.** The port uses `AcosTable::build`/`CurveTable::build`, and the
  original reads the copies its startup built in the image. They agree on every case run, which is
  evidence but not a whole-table compare (that compare is the one the other mxvatv tests rely on).
- **Not wired into a running game.** Nothing in the app runs `rider::modes` yet, and the S278b oracle
  still hooks Pose/Settle as logged calls rather than serving them with the port. The pair is proven alone
  and has not been run inside the mode machine against a live capture.
- **Inputs not drawn:** a NaN/denormal dt; a stale node's parents (they stay the image's); a pose list or
  world list pointer of none.

## Next

None.
