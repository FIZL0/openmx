# 1 — whip trick alignment (FINAL)

Task V98. 2026-10-27.

## What I did

- **The cause.** MD41 replaced V64's `want_beside` in the shell with `Chassis::want_under`. Under a trick
  the bike has no clip for (`Chassis::lacks`), the bike stays on the airborne row (AIR) and the hands are
  held on the grips (`bare_under`, `Body::holds_on`). The AIR row plays the crossup sweep at the phase
  axis, and `Chassis::riding` takes that axis from the rider's packed pose. game-sim's motion.rs only
  writes the pose in its airborne arm. Pack extras such as No Footer take the `Blend::Free` arm, so the
  pose stays frozen where the whip left it. The result: the bike stays crossed up under a rider who is
  playing a square trick clip. That brought V64's report back for every clipless trick.
- **Measured** in `whip_tricks.rs`, which drives the shell's calls each frame. Each trick runs three rides
  off the same jump: whipped (stick right from frame 190, the trick tapped at 215 and held, then the whip
  again), square (the trick alone), and plain (the whip alone). The measure is V64's "apart" metric
  (pelvis over the seat, both hands to the grips), taken on the whipped ride and compared with the
  square ride. Before the fix, a No Footer started whipped put the pelvis ~0.68 ft off the seat and the
  whipped ride 1.33–1.40 ft from the square one. The hands the hold draws on the grips were 1.41 ft off.
- **The fix**, `Chassis::riding_under` in vehicle.rs. While the body is in a trick the bike lacks and the
  bike is on AIR, the row's phase and mix axes go to the middle, 0.5 (the square bike_air_Mid pose). They
  move in step with the body's own ramp: in as the body fades into the trick, out as it fades back.
  - The bike's lags are written each frame so its driver lands exactly on the request. Easing them by
    their own tau left a 0.14 ft tail and a non-monotonic gap that redded V64's step check.
  - Once the fade-out is over, the bike's axes are a bitwise copy of the body's, and the test asserts that.
  - main.rs (the only `chassis.frame` call site, so online seats are covered too) and MD41's harness in
    flip_tricks.rs now call it.
- **Tests** (game-render `whip_tricks.rs`):
  - `a_no_footer_started_whipped_keeps_the_rider_on_the_bike`
  - `every_trick_started_whipped_keeps_the_rider_on_the_bike`: all 32 tricks (28 slots and 4 extras per
    gesture group as wired), each started with the whip on.
  - What they check, from the tap to 3 frames before the landing:
    - the bike plays the body's state, or AIR under a held trick;
    - during the fade, apart stays within the whip's own gap + 0.025 ft;
    - after the fade, apart is within V64's APART (0.125 ft) of the square ride;
    - the drawn hands stay within 0.02 ft of the square ride where the square ride draws them on the grips;
    - after the trick, the axes equal the body's on every AIR frame that isn't held.
  - Results: 0.000 ft past the fade for every trick, the fade at most whip + 0.015 (Superman), and the
    No Footer settles 0.001 ft from the plain whip before the landing.
  - V64's own test now also accepts AIR under a held clipless trick, which is MD41's design.
- **Mutation:** with `riding_under` returning `Chassis::riding` unchanged, both new tests red (No Footer
  apart 0.68 ft, drawn 1.41 ft).
- **The other tricks from a whip:** the bike has clips for most slots (they keep its state equal to the
  body's, as V64 tested). The 17 slots and 6 extras it lacks are all covered by the all-tricks test and all
  pass.
- **Follow-up filed:** V99. A full whip alone (phase 0, hold 1) draws the hands ~0.32 ft off the grips. The
  arm IK reach clamps. This was already the case before V98, and it is separate from the trick.

## What the original does

motion.rs (the port of the original's bike driver) says that a gesture slot with no clip falls back to
`bike_air_Mid` at its midpoint, and writes pose 0.5 + 0.5·lean. So the original's bike goes square under a
clipless trick rather than staying crossed up. `riding_under` takes the AIR row to that same middle
(0.5, 0.5). The pack's extras are the port's own (MX vs. ATV's tricks on MXU's engine), so for them the
original has no exact rule, and this follows the slot rule.

## How it was proved

By the port's own measurements, not against the original. Two game-render tests ride 32 tricks × 3 rides
on the shell's calls (`crates/game-render/tests/whip_tricks.rs`). Also run: flip_tricks (4 tests) and
game-render vehicle (12 tests). The gate is `./kit check --all`.

## Not verified / not 1:1

- **No PCSX2 comparison.** MX Unleashed has no No Footer; it is a pack trick from MX vs. ATV. Filming MX vs.
  ATV doing a whip then a No Footer would need a new move in both.py and the extras' button mapping in that
  game, which isn't confirmed, and its rider RAM is unmapped, so even a film could only be judged by eye.
  The "square under a clipless trick" rule rests on motion.rs's port of the original's slot fallback.
- **The lockstep timing is the port's choice.** The axes go to the middle at the body's ramp rate, not by
  the original's lag. The original would ease its pose at PHASE_TAU from wherever it was, and that eased
  path is what redded V64's step check.
- **The feet were not measured separately.** The apart metric covers the pelvis and both hands. The feet
  are only seen through the pelvis and the bike's own pose.
- **The last 3 frames before the landing are left out** (LANDING), because the suspension moves the grips there.
- **The full whip's 0.32 ft hands gap** was already there before this task and is still there. It is V99.
- **The shell was not ridden by hand or by tape.** The test mirrors main.rs's call order
  (`want_under` → `riding_under` → `frame` → `bare_under`), but it is not the shell's binary.
- **Online seats were not run separately.** They go through the same call site.
- **The tests skip without mods/tricks/mxvatv_tricks.** A slot without the pack passes them unchecked.

## Next

Land it.
