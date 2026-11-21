# 1 — drawn root on the tick (FINAL)

Task S474a. 2026-11-21.

## What I did

- The shell now lends the bike's chassis to the rider's step together with the body (B43's
  `LentBody`, crates/openmx main.rs). After the body is posed, the chassis is posed off this tick's
  motion frame (`ride_chassis_under`: want, request, `Chassis::ride`, bare) and its "Bike" node and
  grips go into `Rider::holds` before the riding step. So the riding step is held to this tick's
  root, not the last tick's.
- The shell's frame skips re-posing a chassis the step already posed (the seat's and the AI
  riders'), but still skins it. The threaded tick (P382) keeps a copy of the chassis for the
  pictures drawn meanwhile, as it does for the body, so `Chassis` and `anim::Stance` derive `Clone`.
- Picked the shell-side fix over sampling the root channel in the sim: the task offered either, and
  this one reuses the B43 lend that already moved the skeleton onto the tick. The sim, its hash and
  ragdoll_riding.rs are untouched.
- tests/replay.rs: `s474_ride_drawn` lends a chassis the same way (`S474Lent`, a `Rider::poser`).
  The pin is flipped and renamed `the_shells_drawn_root_is_the_originals_on_the_tick`.
- Nothing taken from ../opentennis: it has no riding body or drawn bike.

## What the original does

S474's finding: the riding request root (the bike model's "Bike" node) is the airborne pair posed at
the crossup of the tick the request is filled on.

## How it was proved

C37's windmill run (context/capture/s474/south3-cross.csv against b15/south3-bones), in
`the_shells_drawn_root_is_the_originals_on_the_tick`:

| tick | root off the chassis, ours / original's | ours from the original's |
|---|---|---|
| 1 | 29.67 / 29.67 deg | 0.000 deg |
| 2 (verdict) | 1.56 / 1.56 deg | 0.000 deg |

The bones' mean velocity gap with our roots and the original's hands and feet, worst lane (ft/s):

| tick | S474 (a tick late) | S474a |
|---|---|---|
| 1 | 0.30 | 0.33 |
| 2 (verdict) | 1.91 | 0.60 |
| 3 (entry) | 1.48 | 0.33 |

## Not verified / not 1:1

- The test stands in for the shell (it lends a chassis through its own `Rider::poser`, not
  main.rs's `LentBody`). No shell run was compared against a capture. Reason: the shell has no
  capture-driven windmill fixture, and the test reuses the same chassis calls.
- The verdict tick's bones are still 0.60 ft/s off (z lane), at S466's 0.6 bound with the
  original's roots. Reason: not this task's lag. The rest is the riding body's own, already there
  with the original's roots.
- Only the hands and feet are taken from the original in the bone gap check. The drawn grips from
  this tick's chassis were not compared to the original's. Reason: S474's pin compares roots only.
- A ride with no body (no rider model) lends nothing. Its chassis is still posed after the step, so
  any holds it hands over are a tick late. Reason: no riding skeleton without a body, so the riding
  body has nothing to pull to anyway.
- The chassis is now posed inside the rider's step, where the shell used to pose it after the whole
  Sim step. Anything the Sim changes on the rider after its step is no longer seen by the drawn
  bike on that tick, as B43 already made so for the body. Reason: not checked against a capture;
  the drawn bike is presentation apart from the holds.

## Next

Nothing.
