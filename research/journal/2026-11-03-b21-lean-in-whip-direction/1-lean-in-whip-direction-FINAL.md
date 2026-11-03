# 1 — lean in whip direction (FINAL)

Task B21. 2026-11-03.

## What I did

- Measured `whip_tricks`' `a_full_whip_keeps_the_hands_on_the_bars` ride (the stick hard right off the
  flip ramp, the port's riding ragdoll stepped beside it) frame by frame, with debug prints that are not
  kept. I printed the drawn pelvis, the ragdoll's pelvis and every limb joint against the clip's, in the
  body's (the bike's model) space and in the chassis root node's frame. The root node's frame is the
  "bike frame" that C20 and the brief use: it carries the crossup roll, which the model space does not.
- Tried the obvious fix: lift the body along the root node's up (a new `Chassis::up`, passed through
  `Body::bike_up`) by the least amount that brings each grip within its arm's reach, and lean along the
  arm line for whatever is left. The lift came out **0 on every frame of the full whip**, because along the
  root's up each grip sits level with its shoulder. Shoulder-to-grip projected on the up is -0.11..+0.04 ft
  for arms 2.1..2.5 ft long, so lifting the body only moves the shoulders further from the grips. I
  reverted it.

## What the original does

C20's capture (`riding_spring`'s `a_full_whip_hangs_the_riding_body_as_the_originals`): the original's
pelvis sits about 0.34 ft off the clip's, at (-0.09, +0.35, +0.13) in the bike frame. Its hands still end
0.12 and 0.13 ft off the grips. The port's ragdoll does the same through the ramp ride's full whip (frame
291, root frame, ft):

| | clip | ragdoll | grip / peg |
|---|---|---|---|
| Pelvis | (-0.36, 4.08, -1.39) | (-0.45, 4.55, -1.43) | |
| UprArmR (shoulder) | (-0.98, 4.82, -0.39) | (-0.94, 5.16, -0.29) | |
| right hand | (0.66, 4.79, 0.98) | (0.86, 4.84, 1.06) | (0.97, 4.79, 1.12) |
| UprLegL | (-0.57, 3.85, -1.59) | (-0.73, 4.28, -1.60) | |
| right foot | (0.68, 1.63, -0.84) | (0.72, 1.89, -1.15) | (0.78, 1.71, -0.85) |

The whole body rises 0.35..0.47 ft, legs as much as shoulders, and the feet come off the pegs too. The
right shoulder ends *higher* than its grip. So the rise does not take up the arm's shortfall: the ragdoll
closes the hand's gap mostly by stretching the arm, from 2.14 ft posed to 2.27 ft shoulder to hand. The
offset is dynamic. On the same ride, on the ground under gas, the ragdoll's pelvis sits 0.7..0.86 ft
*back* of the clip's (frames 40..180). Through the whip that swings round to 0.48 ft up (frames 200..300).
`lean_in` models neither of these. It sees one frame's pose and grips and nothing of the ride.

## How it was proved

Printed from `cargo test --release -p game-render --test whip_tricks a_full_whip_keeps -- --nocapture`
with temporary prints, then reverted. The test passes as committed: the full whip 258..=302, ragdoll
pelvis 0.492 ft off the clips', ours 0.322, cosine -0.198, its hands 0.086 0.144 off the grips.

## Not verified / not 1:1

- The drawn stand-in (online seats, before the riding body is placed, `OPENMX_RIDING_RAGDOLL=0`) still
  leans across towards the grips at a full whip, not up. That is unchanged, for the reason above.
- Why the ragdoll lifts along the bike's up in the air (inertia of its bodies against the bike's turn,
  the pins, or the S253 target hanging under the bike's transform): not split apart. The port's ragdoll
  matches the original's there (C20, `WHIP_APART`), so the drawing is right wherever the ragdoll is drawn.

## Next

None for B21. The drawn rider goes the original's way once the ragdoll is drawn everywhere: P133 (the
riding ragdoll online) and V63 (delete the stand-in, which takes `lean_in` with it).
