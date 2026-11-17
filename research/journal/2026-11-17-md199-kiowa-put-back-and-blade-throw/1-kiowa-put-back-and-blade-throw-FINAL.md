# 1 — kiowa put back and blade throw (FINAL)

Task MD199. 2026-11-17.

## What I did

- Two oracles in `crates/game-sim/tests/mxvatv_kiowa.rs`, on MD80a's `kiowa` image:
  `the_mxvatv_put_back_places_as_the_original` and `the_mxvatv_blade_throw_is_the_originals`.
- Both first failed on the body's centre only. SLUS-21104's body commit moves the centre on by the
  centre of gravity's offset, turned by the new orientation. The port's `place()` and the props' place
  put the centre at the place itself. The Kiowa's offset is (0, 5.5, 0); each blade's is (0, 0, 9).
- Ported that as `Wreckage::mxvatv` (a new flag, off by default) in `crates/game-sim/src/kiowa.rs`:
  - `put_back` with it on: spawn over the place first, owned or not; then `place_mxvatv` (place,
    then centre += turned cofg); then the unwreck; 0.5 if owned, else 0.
  - `throw_blades` with it on calls `Blades::throw_mxvatv` (the throw, then the same centre move; the
    node's origin is the centre less it again).
- Not switched on in the game yet: the original's put-back also snaps the spawn to the ground, which the
  port lacks. Filed as MD214, which switches the flag on with the snap.
- Took nothing from ../opentennis (no helicopter there).

## What the original does

- Put-back `./kit fn 0x004d36a0 -o atv`: placement `0x004a6f70(heli, 1, 1)`; if the owner `+0x684` has
  `+0x50` bit 0, `+0x10f8 = 0.5`; `+0x48c` vt `+0xb0` if set (0 in the image); vt `+0x378` (present).
- Placement `./kit dis 0x004a6f70 --len 0x140 -p atv`: `+0x50 = +0xd40` (spawn) always. Then, with a1
  and a ground at `+0x3b4` (set in the image): the ground query `0x004833c0(ground, +0x50, normal, 0, 0)`,
  `+0x50.xyz += normal * 0.3`, `+0x60 = +0xd50`, `+0x70 = normal`. Else `+0x60/+0x70 = +0xd50/+0xd60`.
  Then `0x004a4c60` (ground reset `0x00492ca0`, `+0x31c = 1`), `+0xde8 = +0xdec`, vt `+0x3a0`, the
  sound `0x004fa760` if `+0x10dc`, post (8, 0x15).
  - Probed with the query hooked (point y 100, normal (0.6, 0.8, 0)): place (2817.99, 100.24, ...), up
    the normal squared to the nose. So the up IS the normal, as the car's notes in research/oracle.txt
    already say (the older "drops the normal" line is not so for this function).
- Blade throw `./kit fn 0x004d3720 -o atv`: MX Unleashed's throw, unrolled for four blades (nodes
  `+0x11ec..`, wrappers `+0x132c..`): point `0x001952a0`, axes `0x00194960` (z row, y row, w 0), three
  `rand()`s times 2^-31 times 5, plus the rates, +7 on y, unpoint `0x001953c0`, cross, rotate
  `0x00195230`, plus the velocity; start vt `+0x130`; place vt `+0xf0` (= `0x00492e50`, the same orient
  as the helicopter's); velocity and the helicopter's rates written. Then `+0x1200..+0x120c`, `+0x1220`
  hidden and `+0x1224` shown.

## How it was proved

- Function oracle, 10,000 generated cases each, bit-exact:
  - Put-back: the calls out (camera, sound, event 8 21, present), the altitude fraction, the place's
    three quads, the body's last centre, centre, velocity, both orientations, rates. Spawns with any `w`,
    axis and signed-zero directions, varied cofg, owner bit both ways.
  - Blade throw: the four starts in order, the `rand()` state after, each blade body's last centre,
    centre, velocity, both orientations, rates; all six node bits.
- `./kit check -- -p game-sim --test mxvatv_kiowa --test kiowa --test kiowa_capture`: 38 pass.

## Not verified / not 1:1

- The ground snap in the put-back is not ported: the oracle zeroes `+0x3b4`, so the original skips its
  ground query. In the game the Kiowa has a ground, so the real put-back always snaps. Follow-up MD214.
- `Wreckage::mxvatv` is not switched on for the MX vs. ATV helicopter: without the snap, and with the
  port's spawn being the built centre rather than the model's origin, it would put the helicopter back
  wrong. MD214 switches it on.
- The ground reset's node, part and spatial calls ran in the interpreter but are not compared (no port
  state for them); nor are the body's accumulators (`+0x90..+0xd0`, `+0x1b0..+0x200`).
- The blade node's origin in `throw_mxvatv` (centre less turned cofg) is not compared: the oracle does not
  read the blade's scene node.
- The nodes' lazy world update (`0x00193b30`) is hooked in the throw oracle: the node rows are written as
  generated. The rebuild itself is MX Unleashed's `Throw::of`, not proved on SLUS here.
- The blades' start (vt `+0x130`) is only counted, not compared.
- Who writes the spawn `+0xd40`, and when, is not traced.
- The order: SLUS runs the unwreck's own writes, the put-back, then the base unwreck; the port places,
  then unwrecks. They touch different fields (checked by reading), so the order is not observable here,
  but it is not run as one call against the original.
- Inputs exclude NaN and denormals.

## Next

Nothing for this task. MD214 ports the snap and switches the flag on.
