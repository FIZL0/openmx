# 1 — toy numbers truncate (FINAL)

Task S382. 2026-11-18.

## What I did

- `race_start.rs` now reads the drop gate's `.toy` numbers the way the game does: strtod, then the
  truncating double-to-float (`game_sim::phy::number`). Two small helpers, `game_number` and
  `game_numbers`, sit beside `SinkWords`.
- Words moved over: `[LowerGate]` `TriggerPosition`, `[DropGate]` `MoveTime`, and the swing's
  `RotationAxisLocal` and `RotationAngleDegrees` (the last two are presentation, moved for one reader).
- A key that is there but not a number now reads 0 (strtod's answer), not the default. A missing key
  still takes the reader's default, as before.
- The piece counts are unchanged: whole-number parses, no float rounding involved.
- Nothing from opentennis: it has no `.toy` drop gates.

## What the original does

- From C40 (journal 2026-11-17-c40-drop-gate-live): the gate's reader takes each number with strtod
  and then the EE's double-to-float routine, which truncates. sx17's `704.9` is held live as 704.89996.

## How it was proved

- Save-state capture, already in place: game-sim `tests/drop_gate.rs`
  `sx17s_trigger_is_bound_as_the_image_holds_it` checks the C40 start image's trigger bits equal
  `phy::number` on sx17's text.
- New: race_start's `the_sink_words_and_their_defaults` reads sx17's `[LowerGate]` text through
  `SinkWords::read` and asserts those same bits (z = 704.89996, not 704.9).

## Not verified / not 1:1

- **Other tracks not checked.** MX.PSR is compressed, so a grep for the words found nothing. Only sx01's
  `.toy` is extracted here. Which other tracks sink a gate, and whether their truncated trigger lands on a
  different path spot than the old rounded one, was not checked. The fix applies to all of them anyway.
- **Malformed values.** A key whose value is not a number now reads 0, as strtod gives. No track was
  seen to do this, and the game's handling of it was not checked live.
- **MoveTime and the swing words** were not compared against live bits. Only sx17's trigger was. They go
  through the same reader in the game, per C40's reading of it.

## Next

- none
