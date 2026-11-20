# 1 — overlay link failure (READY)

Task K84. 2026-11-20. Concept task: options page only, no product change.

## What I did

- Read context/notes/merge-gate.log (P383's merge gate). `overlay` never ran: its test program failed to
  link (`linking with x86_64-linux-gnu-gcc failed: exit status: 1`, `could not compile openmx (test
  "overlay")`), with eight others in the same build (K80–K85). context/notes/check-merge2.log line 225 agrees.
- Read crates/openmx/tests/overlay.rs (25 tests). Pure: no `Instant`/`SystemTime`, no sleep, no ports, no
  threads, no temp or shared files. Drawing tests open the disc read-only and skip without it.
- Checked the mounted modules' unit tests (menu 2, vehicle 21, roster 14, split 5; files, chase, join,
  overlay none). menu reads `OPENMX_LATEST`/`OPENMX_UPDATED` in non-test code only; its two tests are pure
  settings parsing. Nothing races.
- The cause is the gate, already laid out on K82's page (https://claude.ai/artifact/HRQTe7bGEtuj49GARrjLdy).
- Published the options page: https://claude.ai/artifact/M93XCZbmgdcdjTWWBVjozP
  - A (recommended): fold K84 into K82's gate fix; no change to the test.
  - B: A, plus re-run `overlay` under a loaded gate after K82 lands.
  - C: keep K84 open as is.

## What the original does

Not applicable (test-gate tooling).

## How it was proved

Gate logs and test source read by hand. Not re-run under load.

## Not verified / not 1:1

- Did not run `overlay` under load: the gate log shows it was never run, so a loaded run proves nothing
  about that failure; the source has nothing that races.
- The linker's own reason is not known: the gate's short diagnostics drop it (see K82).

## Next

The human picks an option on the page. If A, close K84 with K82's build task.
