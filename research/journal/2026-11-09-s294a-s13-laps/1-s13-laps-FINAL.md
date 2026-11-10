# 1 — s13 laps (FINAL)

Task S294a. 2026-11-09.

## What I did

- Ported the race handler's two line lists into `crates/game-sim/src/race.rs`:
  `RaceRules::line_lists`, which takes the new `LineLists` (`ListGate`s: a `Gate` and its path `Spot`,
  plus the manager's reach word), and `race_flags::CHECKPOINTS` (0x40) / `ZONES` (0x200). A
  crossing is told as `RaceNotice::Checkpoint { line, through }` / `RaceNotice::OtherLine { line, through }`.
- The checkpoint flag clear for a waited-for card was already inline in `race_step` (S293). I moved it
  out into `RaceRules::clear_checkpoint` so the stretch oracle can call it. Its behaviour is unchanged.
- `RaceRules::card` now forwards to the new `card_on_lines` with empty lists, so its callers (the S293
  test) did not change. `card_on_lines` runs the lists where the handler runs them: after the spot
  search, the below-limit test and the checkpoint clear, and before the off-track test and the lap line.
- `research/oracle.txt`: `score_race_lines` / `score_race_lines_end` (the stretch), and the
  `score_race_tick` and S293 notes updated.
- Nothing taken from opentennis: it has no race-line logic.

## What the original does

`./kit fn 0x00262aa0`, inside the per-card loop; `./kit dis 0x00262d00 0x00263198`.

- Checkpoint clear: the card has flag 4 (waited for), its kept spot's path (`+0x94`) has flag 0x40, and
  the kept node (`+0x98`) is the path's next-to-last (`node+0x54` is set and its `+0x54` is null). Then
  the path loses 0x40.
- Manager flag 0x40, list `mgr+0x70` (linked through `+0x90`). The index i counts every entry, live or
  not. An entry is tested only if all of these hold:
  - it is live (`+0x30 & 1`);
  - the card's `+0xe4` is null, or its word i is 0;
  - the path distance (0x0026e0c0, keep_off 1) from the card's spot `+0x88` to the gate's spot `+0x94`
    is under `mgr+0x100`. When it is not, the distance the other way is asked and must be.

  The crossing test is start_gate_crossed(gate, card+0x60, card+0x70):
  - 1 (through): owner `vt+0x24`, then the chain (`+0x1c4`, next `+0x1c`) `vt+0x24` with (obj, mgr, gate),
    then the listener `mgr+0x30` `vt+0x1c` with (obj, mgr, card, gate), then the listener's own list
    (`+4`, next `+0x18`).
  - 2 (beside): the same calls on `+0x28` / `+0x20`.
- Manager flag 0x200, list `mgr+0x78`: every live gate is tested, with no reach and no passed words.
  Through goes to `+0x2c` / `+0x24`, beside to `+0x30` / `+0x28`.
- The handler never writes `+0xe4`. Who marks a checkpoint passed is not traced.

## How it was proved

`crates/game-sim/tests/race.rs`, `the_line_lists_match_the_race_handlers_card_stretch`. It is a stretch
oracle: it calls 0x00262d00 with $16 = card and $19 = mgr, and hooks 0x00263198 with ra = DONE. It runs
20,000 generated cases, each with:

- a random track (S293's `t_paths`);
- 0–4 checkpoints and 0–3 second-list gates, standing on the paths or anywhere, some dead;
- passed words, sometimes none (a null pointer);
- reaches that are 0, negative, small or large;
- a step at one of the gates.

It checks every listener call, in order, against `clear_checkpoint` + `line_lists`. Each check covers
the slot, the gate passed, the card passed to the listener, and the manager passed along. It also checks
every path's words and the three pools that the path distances use.

Coverage: 905 checkpoint throughs, 2178 checkpoint besides, 1525 second-list throughs, 3481
second-list besides. 6344 live checkpoints were skipped as passed, 1816 crossed gates were skipped as
too far, and 3029 cases cleared a checkpoint flag. A mutant that drops the reverse distance test fails.
The whole race test file passes (27 tests), the S293 per-card test included.

## Not verified / not 1:1

- **Where the lists sit inside the whole handler (`card_on_lines`)** was read off the disassembly, not
  oracled. The stretch is proved on its own. The S293 whole-handler test still masks manager flags
  0x40 / 0x200 and passes no lists. Reason: that test's generator is shared and long, and changing it
  would reseed it. The order (after the checkpoint clear, before the off-track test and the lap line) is
  plain in the code.
- **A checkpoint list longer than the card's passed words.** The original reads past the words into
  whatever memory follows. The port treats a word past the end as clear. The oracle keeps the words
  exactly the list's length, or none. In the game the words are `mgr+0x88` long, which is presumably the
  checkpoint count; that was not checked against a live image.
- **The listener's own list (`listener+4`, next `+0x18`)** is always empty in the oracle. The port
  has no listener chains, the same as every other `RaceNotice`.
- **No live capture.** sx01 was not checked for any gates on these lists, or for whether flags 0x40 and
  0x200 are set in a real race. Reason: the proof is the oracle; a capture would only tell which tracks
  use the lists.
- **The PC exe (`research/mxvatv-pc.md`)** was not read for this function: it has no mapping for the
  race handler.

## Next

Nothing for this task. A follow-up could find who writes the card's passed words (`+0xe4`), most likely
a listener on the checkpoint slots.
