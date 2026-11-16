# 1 — mxvatv car step (FINAL)

Task MD131. 2026-11-16.

## What I did

- Ported the car tire step's contact velocity on whatever a tire stands on (`atv_car.rs`,
  `contact_velocity`). Before this, `Pieces` handed any tire step with non-zero stands-on words to
  `rest`. Now it answers every tire step.
- New `atv_car::Under` (still / object on a body / object on a chassis / mover) and `Holder` types.
  New calls `Call::StandsOn([u32; 2])`, answered into the new `Car::stood_on`, and
  `Call::ObjectCofg(object, v)`, which hands back the object's world centre of mass. The original
  writes that to the object's `+0x90`.
- `tire_step` takes an `Ask` (a closure that makes those calls). `Pieces` builds it over `rest`.
- Took the shape from the quad wheel's port of the same function family (`atv_wheel::contact_velocity`,
  F63): same three branches, same node helpers (`contact::Node::rotate` / `unplace`).
  Nothing came from opentennis, because it has no movers.
- The production ride (`OnWorld`) never asks: its sort search strikes nothing, so the stands-on words
  stay zero.

## What the original does

`./kit fn 0x004b5740 -o atv` / `./kit dis 0x004b5740 --len 1372 -p atv`. Written up in
research/oracle.txt `# MD131`. In short:

- The body's point velocity at the contact (`+0x130`) is the same as before.
- **Object word** (`+0x19c`) set:
  - On a rigid body (object `+0xe4`): the object's centre of mass is turned by the body's quaternion
    and written to object `+0x90`. The arm is the tire's **ground point** (`+0x100`) less (body
    position + that), unturned. The ground velocity is the body's velocity. The point velocity is
    turn(rates × arm) + velocity.
  - No body: the same on the chassis (`+0xe0`) through the object's transform (`+0xe8`), using the
    chassis's centre of mass, velocity and rates.
- **Else mover word** (`+0x1a0`) set: the mover's spin (vt `+0xc4`) is asked, then its velocity
  (vt `+0xc0`). The arm is unplace(ground point) less the origin `+0x180`. The point velocity is
  rotate(spin × arm) + velocity.
- **Neither:** both are `(0,0,0,1)`.
- Then the contact velocity is A − P, the body velocity is v − G, and the local velocity is the latter
  unturned.

## How it was proved

- Frame oracle, `tests/mxvatv_car_frame/movers.rs` `tire_step_on_movers_is_mx_unleasheds`.
  - 6,000 generated frames off `md39a/buggy.ee.bin`.
  - The contact sort, the probes and the tire step are all unhooked, so stands-on words come from the
    sort's real path, and rears are handed the fronts' words.
  - The sort's struck objects and kept movers now stay in its hits. Each one is laid out in free RAM
    with a generated body or chassis and transform, or a mover with its transform. The mover's two
    virtuals are hooked.
- What is compared:
  - every car, tire, body and piece word, bit for bit;
  - the call logs;
  - each object's `+0x90` against the port's `ObjectCofg`;
  - the order and number of the mover virtual calls against the port's.
- Seen: on a body 1780, on a chassis 1829, on a mover 2478, on an object over a mover 1741, and a
  rear handed a stood-on front's words 1959 times.
- Mutations went red: unplacing the contact instead of the ground point (148 words differ), and a
  wrong `w` in the written-back centre of mass (object 1's `+0x90`).
- `run` in pieces.rs became `run_with(.., movers)`. Every other run is unchanged: it draws on the
  same streams and still clears objects and movers.

## Not verified / not 1:1

- The objects, bodies, chassis, transforms and movers are generated, not real MX vs. ATV props. No
  captured ride has a car standing on one, and the buggy image's world has none under it.
- The transforms are always laid out up to date (`+0x100` = 1) with no parent. The node update that
  the two node helpers run first (`0x00193b30`) is never exercised on a stale node. The port assumes
  an up-to-date transform, as the quad wheel's port does.
- The rest answers which kind of ground the words name, and the port trusts that answer. It does not
  re-check the order (object first, then mover, then the body before the chassis). The test's answer
  follows that order, and both-words cases are covered.
- The mover's two virtuals are not ported, only answered. In the test they are the script's. What a
  real MX vs. ATV mover's `+0xc4`/`+0xc0` compute is not looked at.

## Next

Nothing for this task.
