# 1 — body look clock in the sim (FINAL)

Task S328a. 2026-11-16.

## What I did

- Split the body selection's look-around clock (countdown, re-arm, fire, cancel) out of `Body::select`
  into `Body::look_timer` (crates/game-sim/src/rider_anim.rs), with a free `standing` test both share
  and `Params::rearm`. `select` now only reads a look below zero as "fired this tick".
- `Rider::body_look` builds the same ask and request `body_frame` builds (shared `body_ask`) and runs
  the clock on a generator it is handed; with none it uses the rider's own idle pool.
- `Sim::bodies_look` runs every rider's clock once per tick on the race generator (ai lane's, else
  race's), after the AI feeds and before the race frame, humans first, then AI riders by seat player.
  Wired into `step_all` and the single-rider `step`.
- draws.rs: `C33_LOOKS` / `c33_looks` give the tick 7 (ai2, ai0, ai1) and tick 8 (human) re-arms off
  the image's generator; the image check now asserts each body's +0xa30 equals them bit for bit.
- race4.rs: the replay checks the Sim re-arms the same bodies with the same bits at ticks < 15, and
  seeds each bike's body rates as the shell does (`game_render::vehicle::params`) — at zero, the grid's
  first threshold cancelled the human's clock. `C33_AGREES_THROUGH[4]` (the generator) 6 → 14.
- rider_anim.rs oracle and 2p tests call `look_timer` before the selection, as the Sim does.

## What the original does

`./kit fn 0x00248a70`, the fourth arm: when standing (no cut, wreck, gesture, backflip, air, reverse;
|band| < stopped; both axes centred; no throttle), a positive look (+0xa30) counts down by dt and on
going negative draws idle `(int)(rand·2⁻³¹·10) + 0x2f` and sets looking; a non-positive look re-arms to
remap(unit, 0, 1, look_min +0xa34, look_max +0xa38). Every arm then zeroes look when band[0] (+0xa14)
< band. Request pointer at body+0x488, cut at request+0x140. A probe (C33) showed the bodies run after
all AI feeds, in game player order: human (0), ai2 (1), ai0 (2), ai1 (3).

## How it was proved

- tests/ai/draws.rs `s328…`: the C33 image's +0xa30 at ticks 7/8 equal the re-arms drawn off the
  captured generator, bit for bit.
- tests/ai/race4.rs `s12e_free09`: the Sim re-arms ai2, ai0, ai1 at tick 7 and the human at tick 8
  with the same bits, and its generator now agrees with the capture through tick 14 (was 6).
- tests/rider_anim.rs selection oracle (30 000+ generated frames against the ELF's function) still
  passes with the clock split out.

## Not verified / not 1:1

- Only ticks 7 and 8 are proven; the generator parts at tick 15 for another reason, not looked at.
- "Humans first, then AI by seat player" is inferred from C33's game player order, one capture.
- Where the body walk sits against the race manager in the tick is unknown; it is placed before
  `race_frame`.
- The Sim has no clip table, so `body_look` counts pack extras and 360s as no gesture.
- AI throttle for `standing` comes from the controls the Sim rode that tick; not checked against the
  original's request fill.
- A rider stepped without a Sim gets no look-arounds unless its caller runs `body_look`; free rides
  without a generator draw on the rider's own pool with a zero clock.
- The gate idle draw and wreck-limb pool draws are still off the shared generator.
- A re-arm below zero (negative look bounds) would read as a fire; the oracle's fuzz now keeps the
  bounds positive. No disc has negative bounds. Kept so the snapshot wire does not grow.

## Next

- Find why the generator parts at tick 15 in race4 and move that draw into the Sim.
