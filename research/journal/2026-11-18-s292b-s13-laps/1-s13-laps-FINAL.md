# 1 — s13 laps (FINAL)

Task S292b. 2026-11-18.

## What I did

- Found the work already done. The "vtable-only caller of the start-gate crossing test" that S292 left
  is the base AI's gate-mode driver (`ai_gate_drive` in research/oracle.txt). S314 ported it as
  `ai::drive::mode_four` in crates/game-sim/src/ai.rs, oracled. S314b wired it into `ai::Racer`, and S376
  feeds it the card's gate from the laps' chain of gates.
- The port covers what the task names: with the watch-gate bit it asks the crossing test of the card's
  next gate; on "beside" it latches the turn-back bit (`drive::TURNING_BACK`, the original's
  `|= 0xffffffff80000000`), and while that bit holds it measures the path distance both ways and either
  looks back along the path or clears the bit.
- No code change. I fixed the stale "not ported (S292b)" note in research/oracle.txt's S292 block to
  point at `ai_gate_drive` / `ai::drive::mode_four`.
- The task's files line names race.rs, but the driver belongs in ai.rs with the other drivers (as the
  task says, "with the AI driver it belongs to"), and it is there. Nothing to add to race.rs.
- Nothing taken from ../opentennis: no work was needed.

## What the original does

`./kit fn 0x00285090`: see the `ai_gate_drive` line in research/oracle.txt (VehicleAI vt+0x9c, mode 4 of
`ai_update_base`). No race/card → 0; card not taking part → let go, 1; no net and no gate → 0. On a net:
place, own-path reach, route follow, then the gate watch above. Off the net: aim at the gate along its
facing, gate speed, seek reach and hand-off. Then the shared tail.

## How it was proved

- Function oracle, 10,000 generated ticks: tests/ai/drive.rs
  `the_gate_driver_matches_over_ten_thousand_ticks` (S314), against the PS2 ELF.
- Re-ran it here and checked it really ran (a temporary panic printed the branch counts, then reverted):
  `[1589, 5748, 1435, 705, 173, 178, 163, 117, 406, 213, 310, 664, 650, 662]` — "beside" 173, "look back"
  178, "turn back dropped" 163, so every branch the task names is hit over 100 times.

## Not verified / not 1:1

- Mode 4 in a live race: no image or capture has a brain in mode 4 (the oracle line says "not seen in an
  image"; sx01's race brains are all mode 2). The driver is proved as a function only, not end to end in
  a ride. Reason: no race in the original found that runs its riders in mode 4.

## Next

Nothing.
