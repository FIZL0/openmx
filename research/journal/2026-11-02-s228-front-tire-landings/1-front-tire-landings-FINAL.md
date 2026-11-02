# 1 — front-tire landings (FINAL)

Task S228 (S227's ask). 2026-11-02.

## What I did

I indexed the landings that come after 10 or more airborne ticks (research/s228/landings.py and
research/s228/index.md), then rode the port through each nose-first one from the image before it. The
test is `p1_rides_each_event_as_the_original` in crates/game-sim/tests/fly_and_collide.rs.

## What the original does

- 137250: touches down at pitch -23.9 and roll 18.6, at 50.6 ft/s, after a 1.618 s hang. It wrecks
  with reason 3 (Right) at 137251, on the landing crossup clause, not on a nose clause.
- 139063: touches down at pitch -62.6 (89% of the 70 deg ground pitch limit), with vertical g 45.4
  (69% of 66), at 70.5 ft/s. It rides out.
- 139142: pitch -35.0 at a standstill. This is the rider's own put-back (body mode 0), not a landing.

## How it was proved

I ran a bit-exact replay of the port's sim from the original's RAM image 10..13 frames before each
landing, on the tape's pads. Every tick holds the capture's chassis velocity and centre bit for bit
through each landing. The wreck at 137251 comes with reason 3 in the original's frame, and 139063 does
not wreck. Run with `./kit check -- -p game-sim --test fly_and_collide`; it passes.

## Not verified / not 1:1

- Only the nose-first landings on this one tape are covered. A nose-first landing that wrecks on a
  nose clause (ground pitch or vertical g) does not happen here, so that path is not held by this test.
- The put-back at 139142 is not held: the port places the reset differently (S228b).

## Next

Nothing.
