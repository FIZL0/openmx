# 1 — partial backflip score (FINAL)

Task R35. 2026-11-06.

The user (2026-11-05): "a backflip stopped before its actually a backflip gives no points, check what
the original does". **Answer: the original gives no points either.** A backflip that is stopped at
stage 1 or stage 2 of the watch, or that lands or wrecks before the watch comes round, puts nothing on
the score card: no backflip node, no other trick in its place, no combo points, nothing banked. The
HUD shows no trick text and the score stays 0. The port already does exactly this. No change to
score.rs; the task adds the proof.

## What I did

- **Tapes from recording2's human backflip**, not from free06. The brief asks for free06's jump.
  recording2 (free09) is the one recorded human backflip, and C8 had already learned how to bleed its
  torque. A hand-written free06 tape would have meant searching for a run-up and a flip gesture.
  `research/r35/make_tape.py` holds the left stick full forward, with Circle/Triangle/Square lifted,
  from field `--from` to 390. With the trick group at 0, a forward stick bleeds the flip torque at its
  rate_decrease (C8's try 1). So the earlier the push starts, the less of the flip is turned. Tapes:
  `--from` 272, 300, 330 and 360, in `context/recordings/r35/` (slot s1).
- **Captures** with `research/r35/r35.toml` (S79's `watch/s79-score.toml`: the card, the head node,
  the flip watch +0x1400, contacts, attitude, wreck), played by `research/r35/play.sh` (C8's).
  Kept as `context/capture/r35/{stage0,stage1,stage2,round}.csv`, from f272, f300-b, f330-b and f360.
  - The first f330 run (NominalScalar 0.3, load average 200+) lost 119 of 330 rows, including the
    landing.
  - The `-b` runs are re-recorded at 0.12 and are gap-free. f272 and f360 came out complete at 0.3.
  - The ini is back at 0.3.
- **A film** of f330, `context/shots/r35/f330.mp4`, with a strip `strip.jpg` (video frames 280..544).
- **Test** `a_backflip_stopped_short_scores_nothing_as_the_original` (crates/game-sim/tests/score.rs).
  Recipe in `research/r35/README.md`.
- Copied `context/decomp/boot.*` in from s2 (this slot had no boot function table).
- Filed **S281** (low): the watch's "out" gate, below.
- Nothing from opentennis applies (it has no tricks).

## What the original does

The scoring section of the player update (`./kit fn 0x27c890`, the rider's +0x1400 watch):

- **The trick-start hook with id 24** (the backflip, rider vt+0x10c) is called from only two places.
  Both test `watch == 3`:
  - the landing branch (both wheels down, or slow and not airborne);
  - "round in the air with nothing scoring" (card flag 0x40 clear).

  No other path pushes a node for a flip.
- **The watch.** It is stepped only while not wrecked (+0x748 == 0), airborne (+0x57c), and the card
  not out (card flag 0x1 clear) or the watch already past 0. Chassis up-y is +0x94 and forward-y is
  +0xa4.
  - Right way up (up > 0 and fwd > −0.79): 2 → 3, and 1 → 0.
  - up < 0 and fwd > 0 from 0 → 1.
  - up < 0 and fwd < 0 from 1 → 2.
  - Outside the air, or wrecked, any watch but 0 or 3 → 0.
  - Each reset only tells the card's listeners (vt+0x94, event 6, the watch's address). Nothing scores
    on it.
- Every other reader of +0x1400 in the ELF (grep of boot.c) only tests it:
  - the trick-start hook's combo open (`watch == 3`);
  - `./kit fn 0x27f430` (`!= 0`): it switches an object on the rider (+0xfe0/+0xfdc) on or off.
    Presentation; no card call.
  - Two plain writes of 0: one is the wreck (`jump_record_end`, S79). The other I did not open.

The captures (frame_counter ticks; the flip fires at 2507 in every run):

| capture | push from | watch | how it ends | card through it |
|---|---|---|---|---|
| stage0 | field 272 | never leaves 0 (the bike tips back, never past level) | lands clean 2598 | empty; the later cancan (2652) scores as usual |
| stage1 | field 300 | 1 at 2521; back to 0 at 2585, in the air, when the bike comes back the right way up | lands into a wreck (reason 4) 2596 | empty |
| stage2 | field 330 | 1 at 2521, 2 at 2537 (upside down); 0 at 2594 with the wreck | lands on its back, wreck reason 6, at 2594 (no wheel touching) | empty |
| round | field 360 (Taunt kept) | 3 at 2558 | lands 2594 | backflip node 24, 20 000 (× 2 nodes); banks 24 160 at 2605 |

The f330 film (`strip.jpg`): the HUD's score box reads `SCORE 0` through the turn, upside down, and
the crash. No trick name or points text appears at any point.

## How it was proved

- **Live PINE captures compared row by row with the port.** The test drives `Score` from each capture's
  wheels, air, hang time, previous-row speed, attitude, wreck and trick clocks, as `recording2s_points`
  does. It compares the card (flags, node count, total, combo points, head node id and points) and the
  flip watch at the three read phases.
  - stage0: 329 rows match and 1 is torn. Frame 2678 is a late read in the cancan's growth (it holds
    2679's points; the next row repeats it).
  - stage1, stage2 and round: every row matches.
  - The test asserts:
    - the highest watch stage per capture, 0/1/2/3, in both the game and the port;
    - that a backflip node goes on the card only in round, in both;
    - the banks: none for the partial flips, and (2605, 24160) for round.
- **By eye**: the f330 film's HUD, above.
- `./kit check -- -p game-sim --test score -- a_backflip_stopped_short`: passes (4 captures).

## Not verified / not 1:1

- **free09, not free06.** The tapes are recording2's ride (free09), not free06's jump as the brief
  says. The scorer reads only the chassis' attitude, the contacts and the wreck, never the track.
  I did not capture free06.
- **No partial flip landed clean on its wheels at stage 1 or 2.** A bike past vertical (stage 1) or
  upside down (stage 2) does not come down on its wheels. Both such landings in the captures are
  wrecks:
  - stage1 rotated back upright in the air first, reset the watch, then wrecked (reason 4);
  - stage2 landed on its back (reason 6).

  The landing branch reads only `watch == 3` (decompilation), so a clean landing at 1 or 2 would score
  nothing either. That is read, not captured.
- **A watch at 2 rotating back the way it came reaches 3 and scores 10 000** in the original's logic
  (from 2, the right way up gives 3, whichever way it turned there). The port does the same. Not
  captured; it would need a flip that goes past upside down and is then turned back.
- **The HUD was checked on one film (stage2) only**, by eye. The stage1 run was not filmed. The card's
  listeners are told of every watch change (event 6), and what they draw is not ported (P68's). The
  film shows no text for it.
- **The watch's "out" gate is not in the port.** Once the event's time runs out (card flag 0x1), the
  original starts no new watch. Freeride never sets it, so these captures cannot see it. Filed as
  S281.
- **Not captured at the capture's first tick**: every capture starts on recording2's state at 2374
  with an empty card. A partial flip inside an open combo (after a trick) was not captured. By the code
  it would score nothing, because nothing pushes a node.

## Next

Nothing; tell the user the answer above.
