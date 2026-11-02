Task S90. 2026-11-02.

## What I did

- The user lifted the 2026-10-24 ruling on 2026-11-02 ("undo that rule for s90 since we will make it
  more original, make sure we match or close to match the original springyness though"). The 10-23
  complaint had been "our guy is super floppy".
- Turned the riding ragdoll on by default. `riding_ragdoll_on` (crates/openmx/src/main.rs) is now
  `riding_ragdoll_asked(var)`: on unless `OPENMX_RIDING_RAGDOLL=0`. The guard
  `riding_ragdoll_is_off_by_default` is replaced by `riding_ragdoll_is_on_by_default`.
- Spring constants: already equal to the original's.
  `the_riding_bodys_numbers_are_the_originals` (tests/ragdoll_riding.rs) holds the SpringForce and
  DampingForce the port loads from mrider.phy bit-equal to RAM. It passes unchanged.
- Added the swing proof `the_riding_body_gives_as_the_originals` (tests/riding_spring.rs). Over S74's five
  captures and S201's free09-ride2/ride3, each part's give (its node's distance from the animated
  skeleton's node) is measured against the original's:
  - RMS per stretch;
  - peak per part;
  - settle after each put-back (the last tick above half the peak);
  - peak and settle after each jolt (a bump or landing that changes the bike's vertical speed by more
    than 4 ft/s in a tick).

  Result: 13 put-backs and 12 jolts. Body RMS is within about 3% of the original's. Settle ticks are equal
  or within 2. Jolt peaks are within 0.001 ft. Part peaks are within 0.06 ft. Tolerances: RMS 10% (or
  0.005 ft floor), peak 0.07 ft, settle 3 ticks.
- **The shell drew it floppier than the sim proof.** A probe of the drawn pelvis showed it in the bike's
  frame minus the clip's at -0.9 to -1.4 ft back, even at low speed. The original's is -0.3 to -0.66 ft
  back and about -0.1 ft down. The cause was the step's timing. `Rider::frame_bumped` stepped the body
  at its top (0a), on the bike before it moved, with a target that did not trail. The shell then drew it
  under the moved bike. S135's harness, which matches the original, has the request frame on this tick's
  bike and the target skeleton hung under last tick's.
  - Moved the riding tick to the end of the rider's frame (new step 9).
  - The request frame is now the moved bike. The target is hung under the transform from the top of the
    tick, through new `Riding::step_under`.
  - The posed and clip words are read at the top of the tick, as the harness does.
  - A put-back tick hangs the target under the new place.

  Re-probed on the same tape: the pelvis give is now -0.3 to -0.6 ft back and about -0.1 ft down,
  apart from landing spikes.
- A wreck's entry now hands over the bodies as last tick's step left them, because the riding step
  comes after the verdict. That is also the original's order: the body actor runs after the bike.
  `the_wreck_enters_from_the_riding_bodies` now compares against the pre-frame bodies. It arms wrecks
  from tick 3, so a body exists before the verdict.
- `whip_with_doll` (game-render tests/whip_tricks.rs) now places the doll by the ride after `r.frame`.
  Under the old pre-frame placement its hands sat 1.1 ft off the grips, and the test passed only through
  that error. Placed right, the hands are 0.04 and 0.01 ft off. The pelvis leans the stand-in's way,
  further (0.97 ft against 0.32 ft) and with some rise (cosine 0.81). The bound is now cosine > 0.75,
  and a new check holds the hands within 0.1 ft of the grips.
- Shell shots: a 545-tick tape that mirrors S74's 'ride' script on free09, with the ragdoll on and off,
  at ticks 96 (landing), 150, 212 (put-back fader), 225 and 410. The ragdoll rider's posture is close to
  the clip's, with a little more sag on the landing. The over-swing is gone.

## Not verified / not 1:1

- No PCSX2 shots beside the shell shots. `/dev/uinput` is root-only again, so `./kit pad` can't drive
  the original. A matched moment would also need a scheduled .p2m2 and the shell seeded at the S74 state's
  spot. The swing is proved numerically against the original's node rows instead. The look still wants
  an eye (the user's, or a PCSX2 session).
- A rider that wrecks on its very first tick enters from the skeleton, since no riding body is up yet.
- Online sessions have no holds, so they still draw the stand-in (V63).
- Tests: `cargo test --release -p game-sim -p game-render` passes in full. `cargo test --release
  --no-fail-fast -p openmx` passes except two cases. biplane_particles' ground puff also fails with
  `OPENMX_RIDING_RAGDOLL=0`, so it predates this change. netplay_browser's two desktop-vs-browser rides
  fail only under `--release`: the commit stamp differs from the wasm's "dev". In the debug profile all 21
  pass. I didn't run `./kit check`, since there is no gate for this run.
