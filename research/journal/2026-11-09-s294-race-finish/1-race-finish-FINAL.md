# 1 — race finish (FINAL)

Task S294. 2026-11-09.

## What I did

- Split S294: what S294 asked for is the whole race handler (5.4 KB plus about ten callees). This session
  ports the handler's **finish**. The two line lists (the checkpoint list and the second list) are filed as
  **S294a**, which waits on S293's path distance query. The per-card path work was already S293; I noted
  there that the time limit and the card's clocks are done.
- Fixed `race::race_kind` from C31: `+0x60` = 1 is a race that ends at its laps (`RACE`) and 2 is a practice
  (`PRACTICE`). `SINGLE_RACE = 2` is gone. `LapRules::line` now reads `kind != PRACTICE` (same number, right
  name), so its behaviour is unchanged and its oracle still passes.
- New in `crates/game-sim/src/race.rs`:
  - `LapRules::check_over`: the handler's opening over check.
  - `LapRules::clocks`: a racing card's clocks after its path work. Once the race is over, a card that
    has not finished gets its finish made up from its gap.
  - `LapRules::time_limit`
  - `LapRules::close`: the 30 s finish countdown, the end told once with the tie draw, then ranking and gaps.
  - `LapRules::rank`: the original's own odd sort, plus places and the place/lead notices.
  - `race::gaps` and `LapRules::ghost_gap`
  - `RaceNotice`, the new card and manager flag names, and `LapCard`'s `gap`, `off_track`, `time_left`
    and `attack_clock`. `LapRules` gains `finish_wait`, `finish_clock` and `time_limit`.
- Took nothing from opentennis: it has no race scoring.

## What the original does

Recipes: `./kit fn 0x00262aa0` (score_race_tick), `./kit fn 0x00261b80` (score_rank),
`./kit fn 0x002628f0` (score_gaps), `./kit fn 0x002629d0` (score_ghost_gaps). The stretch pcs are named in
research/oracle.txt's S294 block.

- **Over check** (handler start): a kind other than 2 or 3 sets manager flag 2 (over). The first card that
  is taking part (2), has neither 0x4001, has 0x8004, and lacks 0x400000 clears it again. So a race is over
  once no player's card is still racing. In C31 the idle player at the gate keeps the race open, which is why
  that capture never ends.
- **Clocks**: first `last = now` (xyz only). If the race is over (not a ghost race) and the card is not done:
  - race time = the list head's race time + the card's gap
  - lap time = (gap + lap start) / float(laps − card laps)
  - best lap = that, if quicker or there is none yet; then clamped to race time / laps if
    race time < laps × best
  - the card's laps = the race's laps, and the card is marked done (|= 1)

  Otherwise the race clock runs (held in a ghost race while the card is behind the line, 0x10000) and the
  lap clock runs (held while behind the line). Kind 4 with card flag 0x200 runs +0x168.
- **Time limit**: with manager +0xa0 > 0, card +0xc4 > 0 and the card racing, +0xc4 −= dt (held by 0x40000).
  At ≤ 0 the card gets 0x4000, +0xc4 = 0, and owner/chain vt+0x64 and listener vt+0x74 are told.
- **Close**: not a ghost race, manager flag 4 (a player's card done; set in the path block S293 ports), not
  over, and +0x9c > 0: +0x98 runs up to +0x9c, then the race is over and every card is done.
  - The first tick it is over (0x2000): walking the list, a next card whose race time is below this one's
    becomes this one's + rand × 2⁻³¹ (one `rand()` per such pair). Every card's owner/chain vt+0x1c is told,
    then listener vt+0x18.
  - Then a ghost race not finished measures ghost gaps. Any other race ranks if flag 0x4000 is set, and
    measures the gaps unless flag 0x10 (finished) is set.
- **Ranking**: a pass per place pulls any later card with more progress (+0x1c) into the place. After a
  pull it steps over the card that follows, and a done card in the place is left alone. Places are then
  counted from 1:
  - lost: (card, the card ahead, 1) to owner/chain vt+0x34 and listener vt+0x2c
  - gained: (card, the card behind, 0) to the same slots
  - new leader: owner/chain vt+0x38, and listener vt+0x30 gated by the listener's own +0x10/+0x14
- **Gaps**: per foot = 1 / head progress (0 if the head has none). A card not done gets
  race time − per foot × (head time × progress), floored at 0. The head's gap is −(second's), or 0 when
  alone. The product is the head's time × progress first; the other order is one ulp off, and the oracle
  caught it.
- White and checkered flags: nothing in this handler draws a flag. They can only come from the listeners
  reading the lap line's notices (Lap, Finished) and the Over notice; see Not verified.

## How it was proved

All in `crates/game-sim/tests/race.rs` (item 14 of its header, and the C31 test after it):

- **Function oracle, stretches of score_race_tick**: each stretch is called at its first pc with $16/$19/
  $f24/$f25 set, its end pc hooked with ra = DONE, and every listener slot hooked.
  - over check, 10,000 lists
  - clocks, 20,000 cards (made-up finishes and running clocks over 2,000 each)
  - time limit, 20,000
  - close, 20,000 lists of 1–6 cards with ties in progress and race time, ghost cards, every manager flag
    and the generator anywhere; score_rank / score_gaps / score_ghost_gaps run for real inside it

  Compared bit for bit: every card word, the list's order, every listener call with its arguments in
  order, the manager's flags, the countdown and the generator.
- **Live capture**: `the_ranking_and_gaps_match_c31s_live_race` runs every tick of C31's list-ordered sx01
  single race (`context/capture/c31/sx01-ai4.csv`). The previous tick's list, with this tick's card words,
  goes through `rank` and `gaps`, and is checked against the captured order, places and gaps bit for bit:
  about 11,660 ticks, more than 2,000 of them with gaps (until the first finish). 39 of 12,000 tick pairs
  are skipped as torn rows, where the recorder read one card a tick early or late; cards still racing then
  disagree on the one race clock (e.g. frame 1175: the leader's race time and progress are frame 1176's).
- The lap line oracle (S13c) still passes with the renamed kinds and the new card words carried through.

## Not verified / not 1:1

- **The made-up finish, the 30 s countdown, the over notice and the tie draw are proved only by the
  oracle, not live.** C31's races never end: the idle player keeps the race open (manager flag 2 never
  set), and nobody has captured a race with the player finishing. A captured race with a player finishing
  would prove these; that is S13d's capture.
- **The time limit is not seen live.** Nothing in C31 has +0xa0 > 0, and which modes set a time limit was
  not traced.
- **The ghost gap is proved by the oracle only.** No ghost race has been captured.
- **The listeners' own gate on the lead notice** (listener +0x10/+0x14, which the original reads and
  clears) belongs to the listener, so the port only emits `RaceNotice::Lead`. The oracle ran with the gate
  always open (+0x10 = 0).
- **The white and checkered flags are not ported and not located.** No code in this handler or its callees
  shows a flag. They must come from the presentation listeners (mgr+0x30, the cards' owners) answering
  Lap / Finished / Over. Those listener vtables were not read. Presentation is matched by eye, so this is
  left to whoever builds the race HUD (S13d / a P task).
- **Card flag names are guesses from their uses**: PLAYER 0x4, PLAYER_TOO 0x8000, STANDS_ASIDE 0x400000,
  CLOCK_HELD 0x40000, ATTACK_CLOCK 0x200. What sets each was not traced; the arithmetic does not depend on
  the names.
- **Return values are not modelled.** The handler's answer (0 with no cards or no track) is unused by
  anything ported, and the port's `close` assumes a non-empty list.
- **None of this runs in the Sim yet.** The handler as a whole (path work S293, line lists S294a, then these
  pieces in order) is not assembled into a per-tick race step. The pieces are called in the handler's
  order only by the oracle stretches.
- The over check was oracled through the handler's entry with the call cut at the loop start. The clocks
  and time limit stretches ran without the path block before them, which S293 owns.

## Next

S293 (path work), then S294a (line lists). After both, assemble the handler's per-card loop
(path → lists → lap line → clocks → respawn spot → time limit) and `close` into one race step on the Sim.
