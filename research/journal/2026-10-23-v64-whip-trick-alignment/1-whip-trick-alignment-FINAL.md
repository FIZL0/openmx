# 1 — whip trick alignment (FINAL)

Task V64. 2026-10-23.

## What I did

- Reproduced it in a game-render test that rides the shell's calls in the shell's order (`Rider::frame`,
  `Body::ride`, the chassis' want, `Chassis::frame`) off a flip ramp with the MX vs. ATV trick pack: whip
  hard right just after the lip, then a trick half a second into the flight, against the same trick started
  square. Pelvis, both hands and both grips are read in the chassis root's frame every frame.
- Before the fix: a gesture slot and `9oclockindianair` (the bike has its clip) came out the same whipped as
  square once the trick's fade was done. `cliffhanger` (the bike has no clip) did not: the body ran state 62
  while the bike stayed on the airborne row (35) at the frozen whip phase, pelvis x -2.0 ft against -0.4
  square, the rider 1.6–1.9 ft off where the square trick puts it for the whole trick.
- Cause: `Chassis::want_with_pack` sends a pack state the bike has no clip for back to the airborne row, and
  the airborne row is the whip's row, so the bike kept the crossup while the rider played the extra.
- Fix 1, `Chassis::want_beside` (vehicle.rs): beside a body, a pack state (`>= PACK_EXTRA`) the body chose is
  the chassis' state too, clip or not; anything else is still `Chassis::want`. With no clip the chassis
  draws its rest shape, as for a clipless gesture slot. `main.rs` uses it whenever the rig has a body, and
  keeps `want_with_pack` for a chassis with no body.
- Fix 2, `Chassis::frame`'s `fading`: a clipless pack state is out of the clip table, so the selection gives
  `Blend::Free` and no ramp, and the bike snapped square on the trick's first frame (the gap jumped 0.43 →
  0.81 ft). The machine did restart and runs its ramp, so a pack state on `Blend::Free` now fades the rest
  shape by the driver's own ramp, the same ramp the rider fades over.
- After: all three tricks are 0.000 ft from the square ride once the fade is over (from 0.43 ft the frame
  before the trick, which is the whip itself, closing every frame, by frame 14 of the trick). Hands and
  grips: 0.16–0.26 ft through the slot's fade, as square.
- The 360 (MD25's easy one, R1 + one push right on the lip): body and bike both run `threesixtyR` for 61
  frames, hands 0.30–0.42 ft off the grips while the clips play, 0.82 ft at worst on the fade back to the
  airborne row. Identical before the fix (checked with `want_with_pack` put back): both machines are on the
  same state, ramp (0.946 … falling by 0.054 a frame) and rate (3.25) through it, so it is two skeletons
  each blending their two poses, not the whip. Filed as V65.

## What the original does

Not read for this task. MX vs. ATV Unleashed's bike has a clip for every one of its own stunts, so the
original never meets a body stunt with no bike clip; the six missing clips are the pack's own gap (the pack
was cut from MX vs. ATV's files). What the port does with one — the rest shape, like a clipless gesture
slot — is the port's choice.

## How it was proved

`crates/game-render/tests/whip_tricks.rs`, `a_whipped_trick_keeps_the_rider_on_the_bike`: three tricks
whipped against square for every frame from the trick to the landing (the bike's state is the body's;
both rides in the same states; the gap may not grow by more than 0.01 ft a frame through the fade, and is
within 0.125 ft past it), and the 360's hands within 0.9 ft of the grips every spinning frame. Both halves
of the fix are needed: with `want_with_pack` back it reds on "cliffhanger at 215: the bike runs 35 under a
body in 62"; with the fade arm off it reds on "the whipped rider jumped from 0.430 to 0.809 ft". Port
against port only; nothing is compared to PCSX2.

## Not verified / not 1:1

- Nothing here is compared against the original: MX vs. ATV Unleashed has no whipped-trick case like this
  (its bike has every stunt's clip), and MX Unleashed has no pack. The test proves the port's bike and
  rider agree with each other, and that whipped equals square after the fade.
- A clipless pack state's bike fades over the body machine's `rate_default` (4.0), the rate the
  restarted state picks. That equals `rate_stunt` numerically; whether MX vs. ATV would fade its bike at
  a different rate for these tricks is not known.
- The 360's exit fade takes the hands up to 0.82 ft off the grips (pre-existing, unchanged to the frame by
  this task); the test only bounds it at 0.9. V65.
- A trick still running at the landing: with wrecks off the body keeps the slot's clip while the bike goes
  to its grounded row; the test stops at the landing and this is not measured. With wrecks on the rider
  wrecks there, as before.
- Not ridden in the shell by hand or with a tape: the test calls what `main.rs` calls in its order, but
  the shell's own frame (and the online path, which takes the body's `veh.want` from the session) were not
  run.
- The test needs `mods/tricks` (git-ignored, like `pack_clips.rs`) and passes silently without it; it was
  run here with the main checkout's copy.

## Next

none
