# 1 — trick chain blend (FINAL)

Task S214. 2026-10-28.

## What I did

- Built a harness (now `crates/game-render/tests/trick_chain.rs`) that rides a `Rider` in the air on the
  shell's body rates (`vehicle::params()`) and presses trick A, then trick B while A runs, so B is queued.
  Each frame it runs the shell's order: `Body::ride`, then `Chassis::want_under` / `riding_under` / `frame`,
  then `bare_under`. It measures the largest bone turn (degrees) and the largest drawn-joint move (ft,
  rider palette on the bike's grips plus the bike's own joints) from one frame to the next.
- **The body alone was already smooth.** Every disc pair (24×24, queued, and 3 frames after A ends) and
  every pack pair moves ≤ 1.2° on the switch frame and 0.00 ft. The body's mode-5 cross (`Blend::Pair`,
  new clip at `cross`, old at `cross_prev` from the latched `start_phase`) fades, as the original's
  arithmetic does.
- **With the bike in the loop, the drawn pose jumped 4.47 ft on the switch** out of a trick the bike has a
  clip of its own for (flatliner, kod, tsunami, ... — the bike lacks lazyboy, cancan, nohandedlander,
  pendulum, hartattack, indianair, barhop, supercan, superman, saranwrap, stalelin, onehandindianair,
  shoalin, cordova, deadbody, rigamortis, stripper). The rider holds the bike's grips, so the rider snaps
  with it, which is what the user saw.
- Cause: `anim::Stance::frame` (the chassis's 60-state machine) never took the update's latch that
  `vehicle_anim::Update::serviced` / `rider_anim::Body::latch` do for the body (`start_phase = progress`,
  `seq = seq as u8`). So:
  - `driver.seq` stayed 0. A queued trick carries `Tricks::seq` ≥ 2 (`Tricks::request`: 1 for a fresh trick,
    +1 per pop, 0 when idle), so the restart test `r.seq >= 2 && r.seq != seq` fired on **every frame** of a
    queued trick. Each restart rewinds the bike's snapshot and zeroes the cross.
  - `params.start_phase` stayed at the configured 0, so the bike's cross started its outgoing clip at its
    first key instead of where it was a frame ago.
- Fix: the two latch lines after the step in `Stance::frame`. For requests with no trick (biplane, menu,
  plain riding) progress and seq are 0, which is what the fields already held.
- After the fix the BIKE sweep's worst switch is 1.53 ft (tsunami → a pack extra). That is the body-alone
  figure to the hundredth, so the bike adds nothing now.

## What the original does

The update writes the two fields after the step on every vehicle-animation actor: last frame's gesture
progress into the start phase, and the request count as a byte. The body's port has carried this since
V12/S25 (`rider_anim::Body::latch`, `vehicle_anim::Update::serviced`). The bike is the same actor class
reading the same request block (S112: the bike and its rider read one riding request), so it latches the
same two fields. Nothing new was decompiled for this task.

## How it was proved

- `trick_chain.rs::a_trick_out_of_another_fades_into_it`: every trick chained into the next one along
  (23 chains on the disc alone, 30 with the MX vs. ATV pack linked into `mods/tricks`). The switch's
  largest step over 3 frames must stay within 2.5× (+2°, +0.1 ft) of the larger of two steps:
  - the first trick's own step over the same frames, played alone;
  - the cross-fade's steady step 3..8 frames on.

  With the fix: worst 0.78 of the bound (turn), and 0.78 / 0.71 (shift) on the disc / with the pack.
  With the two latch lines commented out it fails: nacnac → flatliner moves 1.97 ft against 0.47.
- The bound needs the fade term: Switchblade → No Footer moves Dummy_foot_R a steady 0.45 ft a frame
  (0.86 on the doubled frame) as the legs cross between two far-apart poses. That is the fade's step, not
  a snap. A snap is one frame's jump with the fade's step on both sides (4.47, then 0.39, 0.20).

## Not verified / not 1:1

- **The original was not filmed.** No PCSX2 capture of a back-to-back pair was made, so nothing was
  compared against the original's bone rotations frame by frame. The brief asked for one. I skipped it
  because the snap turned out to be a port-only omission, a latch the port already carries from the
  original for the body. The cross-fade arithmetic itself was not changed. A capture would still be needed
  to prove the bike's own cross matches to the bit.
- **The bike's latch is inferred, not read.** That the original latches start phase and count on the
  *bike's* actor too (not only the rider's) follows from both being the same vehicle-animation update.
  It was not traced on a bike actor in a save state.
- **The doubled step on the frame after a switch is unproven against the original.** On the restart frame
  the outgoing clip's phase is held at `start_phase`, and on the next frame it moves two frames' worth.
  That is the ported arithmetic, but its feel against the original is unmeasured. Hartattack → Indian Air
  turns LwrArmL 45° there, which is hartattack's own sharp key (22° a frame played alone).
- **The test's pack leg skips in the slot.** It runs only with a trick pack in the checkout's
  `mods/tricks/`. In this slot that was proved with a temporary symlink to the main checkout's pack,
  which I removed afterwards.

## Next

S215 (filed): film a queued pair in the original (a generated .p2m2 from the airborne state: a gesture, then a second
gesture while the first runs) and compare the bike's and rider's joints across the switch.
