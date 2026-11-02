# 1 — sidepost head ray tick (FINAL)

Task S202. 2026-11-02.

## What I did

- Copied S139's three captures and their states (side1, sidepost, post: `context/capture/s138/`,
  git-ignored) from s2 into this slot, and ran
  `the_sweep_back_puts_a_fast_bike_back_out_of_a_post_as_the_original_does` (game-sim tests/replay.rs).
  The port wrecks sidepost at row 2481, reason 6, the same as the original.
- Built the commits in a scratch worktree to find which change fixed it:
  - 309e1db4 (S139's merge): port 2480, as S139's journal says.
  - 46860116 (the merge just before S152): port 2480.
  - 647583ff (S152's merge): port 2481.

  S152 alone moved it.
- Tightened the test: every capture's first wreck must now land on the original's tick, not within one
  tick. All three agree exactly (side1 11 at 2484, sidepost 6 at 2481, post 1 at 2571).

## What the original does

The brief asked which pose the body rays search from on a swept tick. The answer is where the sweep back
leaves the bike. The port already searched from that pose (rider.rs: `ride.sweep_back`, then
`rider_contact::ground` / `scenery_in` on `ride.transform()`), and the path and every put-back matched.
The early wreck came from somewhere else. Until S152, the head ray's wreck test also swept the head
through the props at the frame's end (`ride::hits_among`), and that found the post one tick before the
tick-start search did. S152 dropped that stand-in wherever the body-ray search ran.

## How it was proved

I replayed S139's save-state captures (sidepost: 16 ticks from its state at frame 2470). The port's
wreck freezes the chassis at (4671.606, 136.714, 3822.150), the original's frozen row to print
precision. Before S152, the port froze one row early at (4672.563, 136.641, 3821.570). The test passes
with exact ticks (`cargo test --release -p game-sim --test replay the_sweep_back_puts`, 1 passed, all 3
captures compared, 15 put-backs met).

## Not verified / not 1:1

- The captures and states exist only in s1's and s2's `context/`. In other slots the test prints that
  they are missing and passes.
- What S139's journal lists as not captured still stands: the refusal branch, moving struck objects,
  other prop kinds.

## Next

None.
