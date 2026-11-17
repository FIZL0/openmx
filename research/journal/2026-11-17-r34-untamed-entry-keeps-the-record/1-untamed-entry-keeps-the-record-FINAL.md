# 1 — untamed entry keeps the record (FINAL)

Task R34. 2026-11-17.

## What I did

- Answer: **yes**. Untamed's (SLUS-21701) wreck entry leaves the collision record touched, like
  SLUS-21104's (B35). So its hits also run after a clause's entry, and one frame can enter twice.
- `crates/game-sim/src/rider.rs`: the hits-after-a-clause branch now takes `Handling::Untamed` as well
  as `Handling::MxVsAtv`. MX Unleashed's handling is unchanged, so no replay hash moves.
- `crates/game-sim/tests/untamed_step.rs`:
  - New `an_entry_leaves_the_record_touched`: the real entry, unhooked, on the image's own bike.
  - `untamed_wreck_test_is_the_originals`: the entry hook no longer swaps `+0xf8` to an empty record.
    The port side is `step::wreck_entries`, and both entries must match in order. A new tally checks
    that both double paths come up.
  - `untamed_tricks_41_to_48_are_tricks`: the same hook and port change.
  - `WRECK_RECORD` (the empty record) is gone; nothing uses it now.
- research/oracle.txt `# R34`: comment rows only.
- Nothing taken from opentennis: it has no wreck test.
- The brief's other files (game-render's rider.rs and its test, handling/rider.rs) needed no change.

## What the original does

Untamed's entry (`// 004fa390` in Untamed's decompile) has the same shape as SLUS-21104's: the
reason's base, a rand throw on 13, the part toggle (vt +0x398), the record-part swap (vt +0x484), then
the wreck count. Like SLUS-21104's, it swaps the record's part, not the record, and clears nothing on
the record.

## How it was proved

- `an_entry_leaves_the_record_touched`: the entry run whole on the Untamed RAM image's first bike (one
  of six with Untamed's bike vtable), with only the syscall stubs answered. The record gets a touch and
  a count of 1 first. Afterwards the bike still holds the same record. Every word of the record's first
  0x300 bytes is unchanged except `+0x200`, which is the bike's new part. The entry ran to its last
  line (the wreck count went 0 → 1). No gp or other hooks were needed.
- `untamed_wreck_test_is_the_originals`: 10,000 generated states through Untamed's wreck test, with the
  corrected hook. Reasons in order and the throw at `+0x1250` all match `wreck_entries`. 414 states
  enter twice in both: 231 a clause then a joust, 183 a clause then scenery.
- Mutant: with the old swap put back in the hook, the oracle fails at state 8 ("front"): the original
  logs one reason where the port gives two. So the oracle tells the two apart.
- `./kit check -- -p game-sim --test untamed_step`: 3 passed, image present (not skipped).

## Not verified / not 1:1

- **The ride itself on Untamed's handling**: the rider.rs branch was not run on a live Untamed frame. No
  Untamed capture has a clause and a hit in one frame. The proof is the oracle on the wreck test and
  entry, plus the branch being the one B35 proved for MX vs. ATV.
- **The second entry's side effects**: as in B35. The original enters twice (two rumbles, the part swap
  repeated); the port's ride enters once with the last reason. Reason, clock and throw end the same.
- **The striker's third entry**: no typed field in the port, as in B35.
- **Only one bike and one reason run unhooked**: the image's first bike with reason 4. Reason 13 only
  adds the rand throw on the body (`+0xe8`'s `+0x1d0`), which does not touch the record.

## Next

Nothing for R34.
