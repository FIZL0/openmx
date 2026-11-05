# 1 — run calls in rust (FINAL)

Task F31b5h. 2026-11-05.

## What I did

- Ported the player's run's six calls, which the bike's stunt start and trick end hand out as
  `update::Out::RunOpen/RunScore/Mark/RunClose/RunTally/RunEnd`, into a new module
  `crates/game-sim/src/handling/run.rs` (`open`, `score`, `mark`, `end`, `close`, `tally`). It also
  has the listeners state 4's run tells: the bike's watcher (`watch_open/score/end/tally`, plus the
  stunt table's `repeat` scale) and the group (`group_score` with `kick`, and `group_tally`). Words go
  through the `run::Runs` trait. A listener told is `run::Out::Tell` and an event post is
  `run::Out::Post`.
- In tests/mxvatv_bike_ride.rs, the pure frame's `RunServed` (renamed from F31b5g's `RunDropped`)
  serves the six Outs with the ports (`serve_run`). `told` resolves a listener by its vtable slot,
  decodes the `j`/`addiu a0` thunk, and calls the matching port. `RunInRam` drops and counts the
  posts, the same way the preload cue is dropped.
- `flip_replays_bit_exact_with_nothing_handed_on` now asserts that the whole RAM equals the handing
  run's after every row, except `0x851fec..0x851ff0`, the word the dropped cue writes.
  `ride_replays_bit_exact_with_nothing_handed_on` asserts no difference at all.
- Six oracles, `the_run_{open,score,mark,end,close,tally}_is_the_originals`, built on
  `run_oracle`.

## What the original does

See research/oracle.txt `# F31b5h` for the addresses (`./kit dis -p atv <addr>`).

- **Nodes.** The run keeps a free list of ten-word stunt nodes.
- **Open** marks the player's run open (only when the run is scoring and not held off, and the
  player is in and not out). It frees the player's stunts back to the free list and tells the
  listeners.
- **Score** pops a node and fills it: score = base × multiplier, flags = kind | 0x11. It pushes the
  node onto the player's stunts, sets the running bit and tells.
- **Mark** sets or clears a node's bit 5.
- **End** stops a node. Without the flag it unscores it. With the flag it scales the score by x,
  then floors it (bit 2) or rounds it (bit 1) to the node's step. It tells even when no node is
  found, and keeps the running bit only while a node still runs.
- **Close** ends every running node, reading each next pointer after the call.
- **Tally** sums the scores, adds sum × x to the run's score (madd), floors the score to the run's
  step and tells. The watchers' list is told only when the player has a watcher. It then either
  frees the stunts or, with a history kept, puts them in front of the history and cuts the history
  to the run's length.
- **Watcher.** Each watcher call clears the bike's event record, fills it and posts event 5..8.
  The tally also raises the best combo (two stunts or more), keeps the time once the score reaches
  1e6, and lowers each stunt's repeat scale.
- **Group.** The score kicks each shown gauge. The tally posts event 10. Open and end do nothing.

## How it was proved

- **Function oracles.** Six, each over 10 000 random state-4 cases (`run_oracle`, seeds
  0x31b50001..6). Each case randomises:
  - the run's flags, step and history length, and its free list (the image's, a short scratch one,
    or none);
  - the player's flags, its stunt and history chains (0–5 nodes in scratch), its counts and scores;
  - the listeners: the real group and bike watcher, fake ones in scratch (a group with 0–2 members,
    a watcher, and 0–2 on the player's list), or none;
  - 0–3 planted gauges on the group's owner, plus a negative count;
  - the watcher's best combo, time, kept stunts and the table's repeat scales;
  - the call's arguments, ids sign-extended.
- **What each oracle checks.** The original runs with its fake-listener stubs and the post function
  hooked to log. Every case must match on:
  - the answer;
  - every word of the run, the player, the nodes, the group, the bike, its record, the table and the
    gauges;
  - no write outside those words;
  - the logged calls, in order.
- **Paths.** Each oracle asserts that its paths were taken at least 50 times each, e.g. the tally's
  freed / history dropped / kept / cut / best combo / best time / repeats scaled.
- **Mutation check.** Making the tally's watcher-list walk unnested (and the combo threshold ≥1)
  failed `the_run_tally_is_the_originals` at case 1.
- **Replays.** flip-1 and ride-2 replay bit-exact with nothing handed on, and the whole RAM matches
  as described above. The whole test file passes (61 tests).

## Not verified / not 1:1

- **The event post (0x272300) is not ported.** It is dropped in the pure frame, as the preload cue
  was. On flip-1 it leaves no RAM difference except the cue's word. Whatever listeners it dispatches
  to on other states (F31b5f's camera controller for ids 6/7) are unproven here.
- **Only state 4's listeners are ported.** `told` panics on any other vtable function. Group
  members and the player's +0x1d4 list are reached only as fakes in the oracle. State 4 has none.
- **NaN/inf compares.** The ports compare through host f32 (`F`'s PartialOrd). The generators draw
  no NaN or inf patterns, so those compares are untested.
- **Tally counts.** In the tally oracle, the counts (+0xcc, +0xd4) always match the chain lengths.
  With counts larger than the chains, the original walks off the history chain into garbage memory.
  The port's behaviour there is unproven.
- **Mark and end ids.** Their ids are drawn from 0–45 or −1. Other negative ids take the same branch
  as −1 (signed compare) but were not drawn.
- **PC version not read.** The PC exe was not compared.

## Next

- None for this task. The dropped posts' listeners are F48's (already in the plan); F45 can serve the run's Outs of update::trick with `serve_run`'s ports.
