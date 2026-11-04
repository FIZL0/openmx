# 1 — hits after a clause (FINAL)

Task B35. 2026-11-04.

## What I did

- `game_sim::handling::step::wreck_entries`: SLUS-21104's typed wreck test and hits together. It runs
  the clauses (`step::wreck_test`), then the hits (`step::hit_wreck`) whatever the clauses answered. The
  answer is both entries, clause first. The second one stays, and so does the last throw written (a
  joust hit writes its own throw; a scenery hit leaves the clause's).
- `rider.rs`'s wreck test: on `Handling::MxVsAtv` the hits are now asked after a clause's entry too,
  and the result is `hit.or(landed).or(spun)`. On MX Unleashed's handling and on Untamed's, the hit
  is still asked only when no clause entered, so the answer there is unchanged. With the hit `None`
  whenever `landed` is set, `hit.or(landed)` is the same as `landed.or(hit)`.
- `step::hit_wreck`'s docs: the scenery hit is no longer "answers nothing in the ride". It is the
  second entry when the loads clause wrecked first.
- `tests/mxvatv_step.rs`:
  - `mxvatv_wreck_test_is_the_originals` (MD63c's oracle): the wreck entry hook no longer swaps
    `+0xc8` to an empty record. That swap was MD63c's guess at "the record swap its collision call
    does", and it is wrong for SLUS-21104 (see below). The port side is `wreck_entries`, and the log
    of entries must match in order. A new tally checks that both double paths come up.
  - New `an_entry_leaves_the_record_touched`.
  - New `a_clause_then_a_hit_ends_on_the_hit`.
- research/oracle.txt `# B35`: comment rows only.
- Filed R34 (low): Untamed's entry, same question. My first `plan add` printed a "may be the same
  work" note and still filed it. My second try filed a duplicate, R35, which I deleted from
  plan/TODO.md by hand. That line was my own and uncommitted, and there is no `plan drop`.

- `./kit done --skip-audit`: the audit's 4 leaks (atv_wheel.rs:62/66, plan/DONE.md:519/533) are all
  at HEAD before this change, in files and lines B35 did not touch.

Nothing to take from opentennis: it has no wreck test.

## What the original does

SLUS-21104's wreck test (`./kit fn 0x004e24a0`) runs the clauses and then the hit loop and the
striker. A clause's entry does not end the frame. The hit loop gates on the collision record's
touched word, so the real question was whether the entry (`./kit fn 0x004e2260`) clears that word,
the way MX Unleashed's entry does through its record clear (S191a). It does not.

I ran the whole entry unhooked on the state04 image's own bike, with only the syscalls answered, and
with the record given a touch and a count of 1. Afterwards:
- the bike's `+0xc8` is the same record;
- the record's touched word and count are unchanged;
- the only word written on the record is `+0x200`, the part word set by the swap.

So the hits see the same contacts, and a frame enters twice. Counting the striker, it can enter three
times.

## How it was proved

- `an_entry_leaves_the_record_touched`: the real entry run on the state04 RAM image, as above. Every
  word of the record's first 0x300 bytes is compared before and after.
- `mxvatv_wreck_test_is_the_originals`: 10,000 generated states through `0x004e24a0`, with the
  corrected hook. The reasons entered, in order, and the throw at `+0x1260` all match
  `wreck_entries`. 382 states enter twice in both the original and the port: 197 are a clause then a
  joust, 185 a clause then scenery. Mutant: with `wreck_entries` skipping the hits after a clause
  (the old composition), the oracle fails at state 22 ("15 on the loads"): the original logs two
  reasons where the port gives one.
- `a_clause_then_a_hit_ends_on_the_hit` (needs no image): loads past the back limit together with a
  rider landing from above give (Back, Joust) and the rider's velocity as the throw. The same loads
  with scenery give (Back, Scenery) and leave the throw untouched. This is what
  `handling::wreck::wreck_test` does.
- Replay hashes: every replay is on `Handling::Original`, and that path's answer did not change. The
  whole gate is `./kit done`'s run.

## Not verified / not 1:1

- **Untamed (SLUS-21701)**: the change does not reach it. Untamed's oracle hook swaps the record too,
  but this machine has no Untamed RAM image (`context/untamed/ram/nat_or.bin`), so I could not check
  whether its entry keeps the record the way SLUS-21104's does. Filed as R34.
- **The striker's third entry**: SLUS-21104's striker (`+0x1440` against the bike's mask, reason 5)
  has no typed field in the port's ride or in `wreck_entries`, so it never enters there. This was
  already so before B35, and `handling::wreck` has it word for word.
- **The second entry's side effects**: the original enters twice, while the port's ride enters once
  with the final reason. The first entry's base re-sets the reason and the clock to 0 and rumbles the
  pad, so the original rumbles twice where the port rumbles once. The second entry's part swap
  repeats work on parts that are already swapped. On the port's side, the reason, clock, counting,
  wreck count and throw all end the same. None of this was compared on a live frame, because no
  capture has a clause and a hit in one frame.
- The backwards-landing bail (S161, not the original's) now sits under a hit on MX vs. ATV's
  handling: a hit in the same frame takes its place. That follows from the hits running regardless,
  but no original exists to compare it against.

## Next

Nothing for B35.
