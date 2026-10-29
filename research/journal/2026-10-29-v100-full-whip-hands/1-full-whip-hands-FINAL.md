# 1 — full whip hands (FINAL)

Task V100. 2026-10-29.

## What I did

- **Measured the gap first.** I rode the plain whip from `whip_tricks.rs` (stick hard right from frame 190
  off the flip ramp) and printed every 4th frame.
  - The crossup eases to phase 0 by ~258 on the body and the bike alike (mix 0.5).
  - The clips alone leave the right hand 0.334 ft off its grip, and the left hand 0.000.
  - The clip-drawn rider (the limb stand-in, `hold_limbs`) drew the right hand 0.321 ft off. The posed
    right shoulder is 2.46 ft from the grip, past the arm's ~2.1 ft (upper + lower), so the solve can
    only straighten the arm towards it.
  - At square (phase 0.5) the shoulder is ~2.0 ft from the grip and the solve lands the hand.
- **Asked the original's model what it does.** The original draws the riding ragdoll: hold pins on the
  hands and feet, the bike box round the pelvis and a pose spring to the clip. This is the port's
  `game_sim::ragdoll_riding`, proved tick for tick against the original in S110/S111. I stepped it
  beside the same ride, the way `main.rs` steps it: the pose just drawn is its skeleton (`ragdoll_posed`),
  the grips just drawn are its holds, and it is read at the top of the next frame, placed by that
  frame's ride.
  - First reading: everything ~1.1 ft off, square included. That was my bug: I had placed the ragdoll by
    the ride *after* the step.
  - Second reading, correctly placed. At the full whip the ragdoll's pelvis is 0.93 ft off the clips'.
    In the chassis root's frame it goes from x −0.36 to +0.45, which is the bike box's side (half-width
    0.45). Its right shoulder is ~0.9 ft off. Its hands stay within 0.19 ft of the grips (0.07/0.04 on
    the last frame before the landing).
- **The answer.** The original's arms do not stretch: the ragdoll keeps its bone lengths. The grips do
  not move: they are the bike's own nodes. **The pose leans:** the hand pins pull the body towards the
  bars, as far as the bike box allows.
- **The fix**, `game_render::rider::lean_in`, called by `hold_limbs`. That is the path every drawn rider
  is held with: `Body::palette`, `skin_into_at` and chassis `held`.
  - It moves the whole posed body, as one, by the least translation that brings each held hand's
    target within its arm's reach (the posed lengths, less 1e-4 ft). The target is faded by the hand's
    strength, as `aim` fades it. It runs a few rounds so two short arms settle.
  - It does nothing when both hands are in reach, which covers all normal riding (the clips are ~0.15
    off), and nothing for a hand that has let go (strength 0: gestures, no-handers).
  - The feet are then solved onto the pegs from where the body now is. On the full whip all four limbs
    land: `held` [0, 0, 0, 0]. The pelvis ends at x −0.10, y 4.08, z −1.20 in the root's frame, inside
    the ragdoll's bike box.
  - `hold_on` (V10's bare limb solve, which body.rs's `the_stand_in_solve_puts_a_limb_in_reach_on_its_anchor`
    pins as "nothing above the limbs moves") keeps the bare solve, via the private `solve_limbs`.
- **Test:** `whip_tricks.rs` `a_full_whip_keeps_the_hands_on_the_bars` (`whip_with_doll` rides it with
  the ragdoll beside it). Its numbers:
  - The full whip runs frames 258..=302, and the landing is at 306.
  - The clips' worst is 0.334 ft at 302 (asserted > 0.3 and inside the full whip, so the test keeps
    exercising the far end).
  - The drawn hands are 0.0000 ft off on every held frame of the flight (the bound is ≤ SPIN_HELD, 0.01).
  - The body moves no further than the clips' own hand gap on any frame (0.321 at worst).
  - At the last full-whip frame, the drawn pelvis moves the way the ragdoll's does (cosine 0.989,
    asserted > 0.9) and by less (0.321 against 0.934).
  - **Mutation:** with the `lean_in` call commented out, the test reds: "the full whip drew a hand
    0.321 ft off its grip, at 302".
- `./kit check -p=game-render`: 252 passed.
- opentennis has no rider-on-a-vehicle limb hold, so nothing was taken from it.
- The trick pack (`mods/tricks`, git-ignored) was copied into this slot from the main checkout so the
  pack tests run rather than skip.

## What the original does

It holds the rider on the bike through the riding ragdoll and nothing else. It has no limb solve on the
drawn skeleton (V61). The hand pins pull the hand bodies to the grips. The arm bodies are jointed at
fixed lengths, so a pose whose hands cannot reach drags the torso and pelvis after them. The bike box
(centre 0, 4.3, −1.5, half 0.45, 0.6, 1.75 in the chassis root's frame) stops the pelvis. At the full
whip that stop leaves the right hand ~0.2 ft short through the flight.

## How it was proved

- **The lean's direction and size against the ported riding ragdoll**, stepped beside the same ride
  in-test. The ragdoll is proved bit for bit against the original's update (S110/S111, 1203 riding
  ticks), but here it is fed the port's own clip pose and chassis grips.
- **The hands on the bars** by the port's own measure, with a mutation check.
- **No PCSX2 film.**

## Not verified / not 1:1

- **No PCSX2 comparison of a full whip.** I neither filmed nor captured the original whipping all the
  way out. The "pose leans" answer rests on the ported riding ragdoll, whose update is proved against
  the original, but which is fed here by the port's clips, chassis and request rather than by the
  original's RAM.
  - V61's recording2 hold capture (s3 `context/capture/v61/holds.csv`, pin bodies and targets) does give
    the original's hand-to-grip distance on real ticks: median 0.17 ft, airborne median 0.15 ft. But it
    has no crossup column, so I could not pick out a full whip in it.
  - This is presentation (match by eye) under AGENT.md, so I took the cheaper proof.
- **The clip-drawn lean is not the ragdoll's amount.**
  - Ours moves the body the least that lands the hands (0.32 ft) and puts them exactly on the grips.
  - The ragdoll moves the pelvis 0.93 ft, to the bike box's side, and leaves the right hand ~0.2 ft off.
  - Same direction (cosine 0.99), deliberately smaller: the shell draws the clip pose by default
    because the user found the ragdoll too floppy (2026-10-23/24).
- **No bike-box clamp on the lean.** The pelvis is not kept inside the ragdoll's box. On this ride it
  stays inside, but a bigger shortfall on another bike or clip could push it out.
- **Only the hands drive the lean.** A foot the body leaves out of reach of its peg is straightened at
  it, as before. Not seen on this ride: the feet land, 0.000.
- **Only thq1 on the 250sx with the pack's clips, whipped right**, was measured. A left whip, the other
  riders and bikes, and other trick-pack clips were not ridden. The lean is general, but its size
  there is unmeasured.
- **The test skips without `mods/tricks/mxvatv_tricks`**, like every test in whip_tricks.rs.

## Next

Land it.
