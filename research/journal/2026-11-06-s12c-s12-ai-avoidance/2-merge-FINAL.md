# 2 — merging S12c with S12b/S12d (FINAL)

2026-11-09. The port in file 1 sat on wip/S12c (331bcd80); main had since taken S12b (`ai::controls`)
and S12d (`ai::plan_trick`) into the same two files, and the 11-06 fix session timed out on the
conflict.

## What I did

- Merged wip/S12c into task/S12c (which is main). The plan/ ticks from that branch were left out:
  `./kit done` ticks the task.
- `src/ai.rs`: both sides put their code after `spawn_roster`, and both defined `flag`, `PI`, `TWO_PI`,
  `INV_TWO_PI`, `CURVE_STEP`, `CURVE_INV_STEP` and `dot`. The constants have the same values, so main's
  copies are kept. The avoidance's flag bits (0x80, 0x40, 0x100, 0x20000, 0x100000) now sit in main's
  `flag` module beside S12b/S12d's bits, and none of them clash. The avoidance's `dot` (`Acc::mula…madd`)
  is replaced by main's `dot` (`fpu::mula/madda/madd`), which runs the same instructions, and its `wrap`
  by main's `wrap_turn`, which does the same arithmetic with commuted operands. The oracle below covers
  both swaps.
- `tests/ai.rs`: the avoidance oracle is added after main's tests. Two names clashed and were renamed:
  the feeler record's address `RECORD` is now `FEELER_RECORD`, and the test's `Pick::unit` (a float in
  0..1) is now `Pick::fraction`, because main's controls test has its own `Pick::unit`.
- research/oracle.txt: both sides had added an `ai_character_read` entry at the same address. They are now
  one entry.
- The follow-up ids from file 1 (S281–S283) had gone to S12b's splits on main. The AIChrctr.aip
  avoidance keys are now a note on S283 (the character reader), the path queries a note on S282 (the
  route follower), and the generic VehicleAI pass is filed as S301.

## How it was proved

`./kit check -- -p game-sim --test ai`: 7 tests pass. The avoidance oracle ran rather than skipping:
`ai avoidance: 7092 fans, 738 brakes, 1418 pushed back, 8291 with riders, 522 idle`, with 10,000 passes
bit-exact against the original in Ee.

## Not verified / not 1:1

Everything listed in file 1 still applies. The merge adds nothing new: the two swapped helpers are
covered by the same 10,000-pass oracle. Two gaps from file 1 restated:

- **The PC exe was not read** for the avoidance (file 1 went straight to the PS2 disassembly). The
  PS2 oracle is the proof; the exe could only have pointed at clauses this port lacks, and the one
  known gap there (the generic VehicleAI path) is filed as S301.
- **No live free09 capture** of the brake and swerve; that is S12e's.
