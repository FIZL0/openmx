# 1 — frontflip pose (FINAL)

Task P159. 2026-10-26.

## What I did

- `air::front_flipping(air, row)`: the bike is turning (`Air::flipping`) on the `UNLEASHED_ROTATIONS`
  row that is `FRONT_FLIP`, `row` being the ride's `trick_pack::State::flip_row`.
- `Rider::body_frame` asks the body for `backflip` only when it is not a front flip (it used to be
  `flipping || requested == FRONTFLIP`; `requested` never held FRONTFLIP, which is only ever a
  `started` tell, so that half did nothing). Through a front flip it asks `airborne` and calls the new
  `rider_anim::Body::hold_forward`, which sets the request's mix axis to `rider_anim::FULL_FORWARD`
  (0, the header's mapping of the stick's full forward lean −1) and snaps the body's eased mix axis to
  it, so the pose is `air_Fwd` alone from the flip's first tick to its last.
- The gesture and the 360 (`spin`) arms still run ahead of the airborne one in `Body::select`, so a pack
  clip for the trick (MD21) or a 360-front-flip's spin clip (MD20) still wins. No pack has a front-flip
  clip today (`Trick::FRONTFLIP` is not an extra id), so nothing there changes.
- Nothing taken from opentennis: it has no rider animation.

## What the original does

Nothing: the front flip is the port's own (MD16) and in neither game. The backflip path is untouched —
V35's turning flag still picks BACKFLIP on the disc's rides and on MX vs. ATV's backflip row.

## How it was proved

`tests/rider_anim.rs::a_front_flip_leans_forward_and_a_backflip_keeps_its_clip`: a rider in the air
with the flag up, leaning back (fore-aft 0.9), 40 body frames each way. Front-flip row: every tick asks
AIR, the mix axis is exactly 0 and the blend is the airborne pair at mix 0 (`air_Fwd` alone). Backflip
row and a ride with no pack: every tick asks BACKFLIP, its clip held at a tenth — unchanged.

## Not verified / not 1:1

- No original to match: the front flip and its pose are the port's own (user's ask, 2026-10-26).
- The snap of the mix axis on the flip's first tick is a cut, not an ease, when the rider was not already
  leaning forward; the brief asked for the full forward pose for the whole rotation, so the snap was
  chosen over easing (MIX_TAU 0.125 s) toward it. Not looked at in a running shell.
- The test sets the ride's air flags by hand rather than riding a real front flip off a lip (that needs
  the disc bike and the MD16 pack, as `tests/ride.rs::md16_front_flip_goes_round_and_lands`); the
  wiring from the ride's `flip_row` is the same field that test asserts is 1 on the fired front flip.

## Next

-
