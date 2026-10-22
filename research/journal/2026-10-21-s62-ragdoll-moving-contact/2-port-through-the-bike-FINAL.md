# 2 — the port through the bike (FINAL)

Task S62. 2026-10-22. Reopened after S61 landed (part 1 was blocked on its mode machine).

## What I did

All of it is in `crates/game-sim/tests/ragdoll_wrecks.rs`. No port code changed: S61's
`ragdoll_body::Actor::tick` and part 1's moving push-out already had what the s62 wrecks needed.

- **The capture folder is a parameter.** `Capture::load_in(dir, name)` and `state_in(dir, wreck)` sit
  beside S56's `load`/`state`, which now call them with `s56`.
- **`lay_in` also lays in the wreck bike's object** (`bike_a`/`bike_b` round the request's +0x1ac),
  when a capture has those columns. That means the s62 captures; the s56 ones do not have them.
- **A sphere hit is answered with what it is with.** The collide hook reads each hit's collidable
  (the record's +0x90 + 4i). From the original's RAM at that moment, it builds a `Moving`, one of:
  - a collision object on a dynamics body,
  - a collision object on a vehicle's chassis,
  - a mover (its two virtuals called for spin and velocity),
  - still.

  The test's `Replay` scene answers `Scene::other` and `Scene::velocity` from it, through the
  port's own `ragdoll_contact::moving`. So the velocity the bone is measured against is the port's
  sum, from the original's numbers.
- **`port_tick` no longer skips moving hits** (S61 returned `Err` and left 12 ticks out). It returns
  the hit count, and it can **carry the port's actor** from tick to tick (`carried`) instead of
  reloading it from the original's RAM each tick.
- **New test `the_port_replays_the_wrecks_through_the_bike`.** It covers the five s62 wrecks
  (front, side, back, tip, front3) from their first states:
  - The port runs on its own state the whole way.
  - Each tick, the original's own update runs in the Ee from its state. Up to the last tick the
    oracle holds the capture through (part 1's table), it is also checked against the capture.
  - The port is handed only what the original heard (the ground, the hits and what each is with,
    the sweeps, the skeleton's pose and rests).
  - Every number the update writes is compared (S61's `written`: every bone, pin, joint and
    collision, the actor's fields, the tail).
- `the_ports_tick_is_the_originals_to_rest` (S61's) now compares its moving-hit ticks too. It asserts
  that there are at least 12 of them.
- Filed **S105**: the ride's ragdoll still sees `ragdoll_contact::Open`, so in a ride a bone never
  hits the wreck bike.

## What the original does

- Every moving hit in all nine captures (s56 and s62) is with **one collidable**: the request's
  +0x1ac itself (B). Its collision object is B+0x8c0, and that object's dynamics body is B-0x780,
  the wreck bike's rigid body. So the bones push out against the wreck bike's own body. That is the
  `Against::Other` + `Side::Body` path. No mover or chassis is ever hit.
- That object is at B+0x8c0, which part 1 did not list (it named B-0x580 and B+0x670). It lies in
  `bike_b` (B+0x680..B+0xb80), so the capture carries it. Which of the three is which was not
  followed.
- Measured by printing every hit's collidable, owner, object, body, chassis and class over the
  test run: 132 hits, all the same. That probe was removed afterwards.

## How it was proved

Save-state capture + oracle replay, all of it in `./kit check` (`-p game-sim --test ragdoll_wrecks`,
all 4 tests pass).

| wreck | ticks compared | the capture's own (Ee = capture) | settles |
|---|---|---|---|
| front | 2603..3180 | to 2627 | 1 |
| side | 2608..3180 | to 3146 | 1 |
| back | 2478..3179 | to 3179 (all) | 1 |
| tip | 2473..3179 | to 3179 (all) | 1 |
| front3 | 2631..2687 | to 2687 (all) | 1 |

- 2616 ticks in all, every one bit-equal, port carried. 2029 of them are ticks where the Ee is also
  the capture. 11 ticks have a hit against the wreck bike's body.
- Each wreck runs through the entry (front3's state is already inside it), off-bike, touching and
  the settle, then hundreds of settled ticks (2514).
- **The bike-body answer matters.** Making `Scene::other` answer `None` (the hit as still scenery)
  made the new test part at front 2611 and S61's at front 2608, 86 numbers each. Then it was put
  back.
- **The counts are asserted** (ticks ≥ 2600, exact ≥ 2000, entries 4, settles 5, settled > 2500,
  hits ≥ 11). A first run with a guessed `ticks ≥ 3000` failed on the count alone, so the test
  does not skip.

## Not verified / not 1:1

- **No truly slow tip-over.** The task asks for one; `tip` is S56's tip-a (reason 3, entered at
  32 ft/s). Part 1 tried 19 pad scripts (standing, crawling, braking, leaning, flat and slope). The
  game holds a slow or stationary bike upright. A slow tip-over may need a human ride or a state
  on a steeper slope. Not attempted again this session.
- **Front is the capture's own only to 2627.** After that the Ee runs on from its own state, and the
  port is held against that, not against the game. Part 1's cause for the Ee parting from the
  capture at 2628 is still not found.
- **The mover and chassis paths of the moving push-out** were never hit in any capture. They are
  proved only on part 1's generated cases (`tests/ragdoll_contact.rs`). The test's `Moving::of` code
  for them (calling the mover's virtuals in a hook, reading a chassis' transform) has never run.
- **bike_box's other-actor frame (+0x14)** is 0 in every capture, so it is proved on generated cases
  only (part 1).
- **The other collision object's world centre of mass** (+0xa0, which the original's push-out writes)
  is written by the port into the test's copy and not compared. Nothing in the tick reads it back.
- **The broad phase and the bone spheres' scene test are not ported.** The port is handed the
  original's own hits. In the port's ride the ragdoll sees no wreck bike at all (S105).
- **Other actors** (AI riders and their bikes) are in none of these wrecks.
- **No flung ticks in the s62 wrecks.** The flung and landed arms are compared only in S61's s56 test.

## Next

Landed. S105 puts the wreck bike in the ride's ragdoll scene.
