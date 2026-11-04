# 1 — side 3147 (FINAL)

Task S203. 2026-11-04.

## What I did

- Side's s62 capture is in `kept-context/s5/capture/s62`, and this worktree's `context/capture` had no
  s62. I symlinked it in (git-ignored) and did not copy it.
- Read rows 3140..3151: `dt`, `rng_state`, `w05ca1f8`, bone 0's `pos`/`last_pos`. Row 3146's bones equal
  3145's, and so do its generator and clock. Row 3147's clock is 2 ticks on and its `last_pos` x
  (4234.5444) is not 3146's `pos` x (4234.6533). So 3146 is S147's row read before the tick, with a
  tick that drew nothing.
- Surveyed every s62 capture for rows whose bones equal the row before. Front has 2889/2892/2894
  (generator moved, already `torn`), side has 3146 (generator held, the next row two ticks on), and
  back, tip, front2 and front3 have none. In s56's captures such rows are common after the wreck (the
  bones at rest, the clock stepping 0, 1 or 2 ticks). So `repeated` is left as it is, and the change
  reaches only the s62 loops, which call `torn`.
- Tests (crates/game-sim/tests/ragdoll_wrecks.rs):
  - `torn`: with the generator unchanged, a row also counts as torn when its clock is the row before's
    bit for bit and the next row's is more than 1.5 dt on.
  - `the_port_replays_the_wrecks_through_the_bike` and `every_sphere_test_of_the_wrecks_is_the_bike_meshs`
    skip a `repeated` row only if it is not `torn`.
  - `S62` side's bound is 3180 (was 3146). The replay asserts 4 gaps (was 3). The sphere test counts
    24828 tests (was 24822): the six of 3146's tick, which was skipped before. Every other count is
    unchanged.
  - `front_2889_is_a_row_read_before_the_tick` expects side's torn rows to be [3146].
- Took nothing from opentennis: it has no ragdoll capture replay.

## What the original does

Nothing new about the game. It is the same frame-counter tear as S147. This time the rest of the
game's tick drew no random number, so the generator could not show it.

## How it was proved

- `./kit check -- -p game-sim --test ragdoll_wrecks`: 16 passed. The replay prints "side:
  2607..3180, the capture's own to 3180".
- Mutation: with the clock clause off (3146 skipped as before) and the bound 3180, the replay fails at
  side 3147. Bone 0's `last_pos` x is 4234.6533 against the capture's 4234.5444, one step short.

## Not verified / not 1:1

- **Side's rows 3146 and 3147 are not checked against the capture.** As in S147, the hidden tick ran on
  a request the recorder never read. The port is compared with the original's update there, on 3146's
  (= 3145's) request.
- The s56-capture tests skipped in this worktree because it has no `context/capture/s56`. They call
  `repeated` only, and that is unchanged.
- Only the replay test was run under the mutation. The sphere test was not.

## Next

none
