# 1 — whip lean against c20

Task T4. 2026-11-16.

## What I did

- whip_tricks `a_full_whip_keeps_the_hands_on_the_bars` has a direction check again, now against the
  original's lean (C20) instead of the port's `lean_in` stand-in.
  - `C20_LEAN` = (-0.093, 0.345, 0.133) ft: C20's mean pelvis-off-clip over its 36 full-whip ticks, bike frame.
  - The check averages the riding ragdoll's pelvis-off-clip over the same full-whip ticks of the ramp
    ride, as C20 does.
  - Cosine to `C20_LEAN` must be over 0.85 (measured 0.882).
  - The rise along the bike's up must be within 0.05 ft of 0.345 (measured 0.361).
- The full-whip ride (`whip_with_doll`) now steps at 1/30 s, the shell's and the original's tick
  (`TICK`, `WHIP_TICK` = 95, `TICKS` = 160). The other whip_tricks rides still step at 1/60.
- The ragdoll's hands-off-grip bound went from 0.15 to 0.18 ft (`DOLL_HELD`). At 1/30 the right hand
  is 0.151 off (was 0.144 at 1/60). The original's own hands are 0.158 and 0.174 off (C20), so the
  bound now sits at the original's worst.
- The full-whip length bound went from 20 frames to 10 ticks: the same time.
- The doc comment no longer says the drawn rider leans the ragdoll's way. It does not (B21).
- opentennis: no rider ragdoll or whip there; nothing taken.

## What the original does

C20 (research/journal/2026-11-03-c20-full-whip-capture): at a full whip the original's pelvis rises
along the bike's own up by about 0.35 ft, shifted back a little. It does not slide to the bars or the side.

## How it was proved

- `./kit check -- -p game-render --test whip_tricks`: 7 passed. The trick pack was copied into this
  worktree's git-ignored `mods/tricks` first. Without it every test in the file prints SKIPPED and passes.
- The numbers were read from a deliberate failing run, with an impossible bound (cosine > 1.1), then the
  bound was set. Full whip at ticks 128..=148 (21 ticks), landing at 152. Mean ragdoll lean
  (-0.089, 0.361, -0.050), length 0.375. Hands 0.126 and 0.151 ft off the grips. Clips worst 0.658 ft
  at tick 112. Drawn hands 0.00002 ft off.

## Not verified / not 1:1

- The ramp ride is not C20's ride. C20 is free09's jump 2, with the bike rolling through the flight; the
  ramp bike holds one attitude. So the z part differs (-0.050 against +0.133), and the check is a
  direction bound (cosine 0.85), not a match.
- Only a right whip is checked. C20 has no left whip, other bike or other rider.
- `lean_in`'s direction is not checked. It is wrong (cosine ~0.05 per C20); that is B21.
- The other whip_tricks rides (whipped tricks, 360s, the 360 throw) still step at 1/60. Their bounds are
  port-against-port, and moving them all to 1/30 would re-measure every bound. Not done here.
- The whole suite only proves anything where the trick pack is installed in the worktree's `mods/tricks`.
  A slot without it skips silently in `./kit check` output.

## Next

none
