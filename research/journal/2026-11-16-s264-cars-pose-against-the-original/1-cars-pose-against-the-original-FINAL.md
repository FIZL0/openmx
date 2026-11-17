# 1 — cars pose against the original (FINAL)

Task S264. 2026-11-16.

## What I did

- Read a driven buggy's node matrices out of a PCSX2 RAM image and compared every node with the port's
  pose. S149a's own car states are gone (not in `context/` or the old s1 context), so the image is C34's
  `context/states/c34-buggy-switched.ee.bin`: the buggy driven on free06, car object at 0x00605270, the
  same place S149a's switch parks it.
- Before: S245's relative aims were off by up to 0.43 in a turn row (rear Strut_02), 0.25 on the rear
  tires, 0.1 on the wheel hubs, 0.076 on the front tires, 0.0064 on the swing arm.
- Decoded the baja/buggy pose step (`./kit fn 0x0039edb0`) and wrote it as `buggy()` in
  `crates/game-render/src/car.rs`: every turn set afresh, the lower arm's and hub's angles from asin of
  the node positions' cross product, the swing arm's frame from its direction and up, all four tires
  stretched, the struts' skewed frames taken into their parents.
- After: every node of the original's tree within 0.0008 in a turn row and 0.0016 ft in place, the size
  of the float noise on the nodes nothing moves (0.0003–0.0007).
- The old relative rig (`arms()`) stays for models without the buggy's nodes (MD157's MX vs. ATV cars,
  the golf cart), so their look is unchanged. The truck is unchanged.
- Test: `crates/game-render/tests/car.rs` `against_the_original::the_buggys_pose_is_the_originals`.
  The disc test's "at rest it is the model" now allows 0.7 ft for the baja and buggy (see below).

## What the original does

- Each front hub's 3x3 is replaced by a turn about y by its steer, each tire's by a turn about x by its
  spin. The places stay. The model's own turns on those nodes are dropped: the front tires' ±10° z
  turn and the rear tires' x turn (which undid the hubs' and swing arm's bind turns) are lost.
- Right side then left: the upper arm is set about z (−z on the left) by 1.25 × compression ÷ |FR hub −
  FR upper arm|. The lower arm is set to identity, then about z by π/2 − acos(z of n(swingarm node −
  lower arm) × n(hub bottom node − lower arm)). The hub then turns in its own frame about z by the same
  of n(hub bottom − top swingarm node) × n(bottom swingarm node − top swingarm node). The cross order
  comes from the VU pair `vopmula a,b; vopmsub b,a` = a × b. acos is a 30000-entry lerped table.
- The swing arm's frame looks from (0, RL anchor y + the higher rear compression, RL anchor z) to its
  own bind place: rows [up' × dir, up', dir], with up' = y made square to dir.
- Rear tires set about x by spin; every tire's rows' y scaled by its stretch.
- Each strut pair: dir = n(Upper − Lower) in root space, measured once. Both nodes get rows [dir × z,
  dir, z] (neither unit nor square), taken into their parent's frame, places kept.
- The hub's bottom node and top swingarm node keep the world matrix the position reader wrote them
  with, before the hub's last turn. Nothing refreshes them, and no vertex sits on them. The test checks
  that stale world against local × (hub unturned) × upper arm, then the port's own against the hub.
- The baja shares the buggy's vtable (0x0045cf80), so it runs the same step.

## How it was proved

A save-state capture: one RAM image (C34's switched buggy, a ridden frame, every spring compressed
0.3–0.6 ft), every one of the 45 nodes under the root compared, 0.002 / 0.003 ft limits.

## Not verified / not 1:1

- **One still frame, steer 0.** The steer's order against the hub's post-turn is read off the code, not
  seen in a steered frame. The stale hub marks are checked only because steer is 0 (the test says so).
- **asin, not the acos table.** `std` asin stands for π/2 − the lerped 30000-entry table. They differ in
  the last bits, more near ±1. Inside the 0.002 limit here.
- **The baja is not compared.** No baja state exists (S149a's free07 one is gone). It runs the same step,
  but its model differs: its swing arm is unturned in the model, so the original's frame pitches it 7.4°
  and its rear tires sit 0.68 ft below the model's place at rest; its left hub's 11° model turn is also
  dropped. That follows from the code. It has not been seen on screen, so it may hide something
  (e.g. different anchors). Follow-up S342.
- **The truck is not compared.** Its step (`./kit fn 0x003e07b0`) is a different function, with a
  different node table (hubs set and lifted, axles rolled with the same acos table, drive lines and
  swingarms aimed with 0x001b1e20 toward stored places). First recording's parked truck (car
  0x00e1e210) is a usable image. Left as S245 wrote it; follow-up S343.
- **MD157's cars keep the port's own rig.** They lack the buggy's swingarm nodes; MX vs. ATV's own
  pose code was not looked at.

## Next

Nothing for S264. The follow-ups cover the truck and the baja.
