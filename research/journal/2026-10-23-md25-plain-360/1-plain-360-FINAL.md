# 1 — plain 360 (FINAL)

Task MD25. 2026-10-23.

## What I did

- Rode the brief's case in the sim (generated lip, MX vs. ATV's step, R1 held): one sideways push past 0.7
  **only arms** the 360 and nothing fires. The 360 fires only on the two-way gesture (past 0.7 one side,
  then past 0.7 the other), and then it brings MX vs. ATV's nose-up pitch with it. A player who pushes
  right and holds it gets no 360. A player who sweeps through gets a 360 with a backflip in it (the cork).
- Diagnosis, against the brief's three candidates:
  - **(c) the 360 never arming the way the player rides it: this is the cause.** The step needs a gesture
    no player guesses, and the flick it measures (ride::Flick) is a crossing, so a push from centre has
    flick 0 at best.
  - (a) pitch share: **not a fault.** air::three_sixty's yaw = dir·lerp(flick, 200..1100) and pitch =
    −lerp(flick, 600..1100) match tricks.md "The 360" in sign, scale and lane. The nose-up is MX vs. ATV's
    by design.
  - (b) the flip row winning: not the cause under R1. MD20's both-at-once rule lifts the lock-out, and a
    backflip armed first removes the 360's lip in MX's step anyway.
- Bike inertia (the brief's question): the disc 125's Mass block is weight 400, inertia (22.4, 18.4, 10.9),
  YawRateLimit 3.0, AirborneDamping 450. MX vs. ATV's mx_Race125P.phy has the same values. **MXU's inertia
  does not make the nose-up worse.** I found one more limit: YawRateLimit 3 rad/s caps chassis yaw, so a
  flat 360 at that rate needs at least 2.1 s of air. The generated lip gives about 2.07 s.
- Built the user's answer, an **Easy 360 of the port's own, on by default** (air.rs `SpinStyle::Easy`):
  - With R1 held, one sideways push past 0.7 (|y| ≤ 0.5) on a lip fires a flat 360 that way. There is
    no arm step and no nose-up torque.
  - In the air, a rate controller turns the bike at up to 5 rad/s and homes it in on one full turn
    (`progress` is the yaw rate summed its way). The spin ends at rest within 0.05 rad of a full turn.
  - While the spin runs, the bike's yaw limit is lifted to 5 rad/s for that substep only
    (`easy_spin_yaw_limit`, ride.rs around integrate).
  - Pitch and roll take plain flight's auto-level and not the flipping path's all-axis damping, so the
    bike lands level with no ground slide. The easy spin leaves the yaw lane alone.
  - The constants are `EASY_SPIN_*` in air.rs, each commented as the port's own.
  - A diagonal push (|y| > 0.5) still goes through the exact gesture, so MD20's 360 flips work under both
    styles.
- Added the switch: Options → Original → "360 Style", Easy (default) or MX vs. ATV. Original mode sets
  MX vs. ATV. It is stored in settings.txt as `three_sixty = mxvatv` (written only when not Easy). There is
  also `--three-sixty easy|mxvatv`. trick_packs::Pack carries the style into Moves. The Departure doc
  now names the 360 style as the one sim exception.
- Hashes: Spin's new fields enter the hash walk only when they are set, so MX vs. ATV rides hash exactly
  as before.

## What the original does

See ~/repos/openmx-mods/notes/tricks.md "The 360". Arming past 0.7 one side and firing past 0.7 the other
gives yaw and pitch (nose up) torques scaled by the flick. The spin decays only with R1 released and the
stick against the spin. The port's MX vs. ATV style is unchanged.

## How it was proved

- tests/ride.rs, generated lip, 60 Hz:
  - `md25_easy_three_sixty_turns_flat_and_lands_clean`, right and left: from take-off to rest the yaw
    turns 5.9..6.7 rad with |pitch| < 0.25. It rests before landing, inside SPIN_CLIP, lands with clip 0,
    no wreck, upright, heading within 0.2 of take-off, and scores THREE_SIXTY_RIGHT/LEFT.
  - `md25_easy_three_sixty_after_a_sweep_back`: the stick goes back first, then right or left (right a
    little up, y 0.4).
  - `md25_mx_vs_atv_needs_the_gesture`: one push arms only. The gesture fires with torque.x ≤ −600. Easy
    fires on one push.
  - `md25_mx_vs_atv_rides_as_before`: gesture and diagonal rides under MX vs. ATV's step. The folds are
    pinned (0x1d8d55415bc24f70, 0x13fe6e4514be885d). I computed them at HEAD in a scratch worktree, and
    they are identical on the branch.
  - `md20_three_sixty_flips_turn_both_ways_and_land_clean` now runs under both styles.
- MD16/MD20's own tests keep MX vs. ATV's step (md16_pack sets it), and so does tests/rotation_modifier.rs's
  pack (its R1 gestures arm first, which Easy would fire on).
- openmx tests/menu.rs `the_three_sixty_style_is_easy_unless_original`: default, Original, settings
  round-trip.
- Gate: `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- The Easy 360 is the port's own (user's request), so it can't be proved against the disc. Its 5 rad/s rate,
  the lifted yaw limit, the homing and the auto-level in the air are all port choices.
- I didn't ride it in the shell. `--trace` has no R1 or trick-pack input, and I didn't write a `--play`
  tape with R1. It is proved only in the sim on the generated lip, not on free06 or free09's real jumps
  and not on bikes other than the stock 125.
- The rotation-speed modifier scales Easy's torque clamp but not its 5 rad/s rate.
- MX's disc would refuse the 360 after a backflip was armed (a sweep back). The port's MD20 R1 rule allows
  it, and Easy relies on that.
- Easy ignores the stick and R1 in the air once fired. Nothing slows or stops it early.
- Online rides use Original's tricks, so they always take MX vs. ATV's 360. Easy is offline only.

## Next

- Nothing open. If players want it, a 360 rate knob could join the game modifiers.
