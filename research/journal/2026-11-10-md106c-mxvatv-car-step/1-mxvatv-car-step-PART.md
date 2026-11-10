# 1 — mxvatv car step (PART)

Task MD106c. 2026-11-10.

## What I did

- Checked the prerequisites before starting. MD106c is "part 4, after MD106a and MD106b". Both are still
  open in the plan, and neither has a branch or commit on main (`git log origin/main | grep MD106` shows only
  MD106 part 1). `crates/game-sim/src/atv_car.rs` still hands every callee (Controls, TravelCofg,
  SortContacts, Probe, VehicleForce, AntiSlide, Preload, Launch, Engine, TireStep, Aero, VehiclePost,
  Shift, WreckTimer, Landed, NodePosition) out as a `Call` with nothing in the port to answer it.
- To run a whole frame with only the ground query hooked, every one of those calls needs ported code to
  answer it. That is all of MD106a's and MD106b's work, so this task cannot be done first without doing
  theirs too. Blocked with `--after MD106a,MD106b`. No code was changed.

## What the original does

Not looked at this session. MD106's journal (research/journal/2026-11-10-md106-mxvatv-car-step) describes
the frame driver.

## How it was proved

Nothing proved. This session only checked the prerequisites.

## Not verified / not 1:1

- The whole brief is unproven: there is no whole-frame proof off buggy.ee.bin yet. It waits on MD106a
  (MXU pieces answering the calls) and MD106b (SLUS-21104's own calls), neither of which has landed.

## Next

Once MD106a and MD106b have merged: read MD39a's buggy image whole into Car/body/engine/tires. Then extend
tests/mxvatv_car_frame.rs with a case that unhooks every call except the ground query, answers that through
atv_ground on MC02, and compares every word. Account for the idle second buggy's writes and the other two
preforce listeners'.
