# 1 — exe guided proofs

Task K5. 2026-10-26.

## What I did

- docs/PROVING.md: a new section, "Reading the PC exe first: aim the proof, don't widen it", between the
  replay harness and "Choosing, in practice". It says why the exe helps (RTTI classes, vtables, assert
  files, a decompilation Ghidra reads well), that it is never the oracle (x87, Xbox-derived, a later game
  with moved offsets and retuned constants), and the four steps: find the function in the exe (pc_ rows,
  class::vfN, pc.sources.tsv, shared strings); list every branch, clause and constant in the journal and
  find each on the PS2 side; aim the oracle's generated inputs (either side of each threshold, ±0, each
  index, a per-branch hit count) and a few short captures at the list; file a clause the port lacks, or
  write a PC-only one under Not verified. The list goes in the journal with proved-by / not-covered.
- AGENT.md "Proving it": a short paragraph pointing to it, with the "never the oracle" rule.
- Nothing taken from opentennis: it has no second build of its game to read.

## What the original does

n/a (docs). The facts about the exe come from R10's research/mxvatv-pc.md.

## How it was proved

Docs only (t: none). The commands it cites exist: `./kit fn … -o pc` (R10), `./kit plan add`,
`context/decomp/pc.sources.tsv` (written by `./kit ghidra name -o pc`). `./kit check --all` via `./kit done`: green on the first run (2340 tests; it stopped only on other
tasks' plan/DONE.md audit leaks, hence --skip-audit). The second run reddened one_fps's frame budget
(95 ms, then 81 ms alone) at load average ~40 on 32 cores; three lone reruns passed. Docs cannot move a
frame time, so it is load, and one_fps belongs in gate.toml's [flaky] list (not mine to add here).

## Not verified / not 1:1

- The method itself is untried: no task has yet followed the four steps end to end, so whether the
  per-branch hit counts are cheap enough to be the norm is a guess. The first task that uses it should
  say in its journal what in the section was wrong.
- I did not run `./kit fn MotocrossRider::vf44 -o pc` here (the example in the doc is the one from R10's
  `./kit fn -h`); the pc Ghidra dump is in whichever slot ran R10, not necessarily this one.

## Next

none.
