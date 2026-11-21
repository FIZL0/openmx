# 2 — folded into K82 (FINAL)

Task K84. 2026-11-21.

## What I did

- The person picked option A from the options page (https://claude.ai/artifact/M93XCZbmgdcdjTWWBVjozP):
  fold K84 into K82's gate fix.
- Noted K82 (`./kit plan note K82`) that its gate fix covers `overlay` too.
- Changed no code: crates/openmx/tests/overlay.rs has nothing that races (see file 1).

## What the original does

Not applicable (test-gate tooling).

## How it was proved

Nothing new to prove: file 1 read the gate logs and the test source.

## Not verified / not 1:1

- `overlay` was not run under a loaded gate: the failure was a link failure, not a test failure, so a
  loaded run says nothing about it. K82's relink and stderr logging will show any repeat.
- The linker's own reason for the failure is still unknown: the gate's short diagnostics dropped it. K82
  adds the logging that would catch it.
