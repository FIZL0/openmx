# 1 — freestyle event link failure (READY)

Task K83. 2026-11-20. Concept task: options page only, no product change.

## What I did

- Read context/notes/merge-gate.log (P383's merge gate). `freestyle_event` never ran: its test program
  failed to link (`linking with x86_64-linux-gnu-gcc failed: exit status: 1`, `could not compile openmx
  (test "freestyle_event")`), like eight others in the same build (K80–K85).
- Read crates/openmx/tests/freestyle_event.rs (one test). Pure: fixed sim on a flat floor, disc read-only,
  no ports, threads, temp or shared files. Its only clock is one `Instant::now()` base moved by hand
  (+1 s), so nothing is timed against the wall clock.
- The cause is the gate, already laid out on K82's page (https://claude.ai/artifact/HRQTe7bGEtuj49GARrjLdy).
- Published the options page: https://claude.ai/artifact/8FPuB4pa6SCGCcHgadKVaY
  - A (recommended): close K83 into K82's gate fix; no build task here.
  - B: also give the test a fixed start instant; tidy, fixes nothing that failed.
  - C: keep open and re-run under a loaded gate after K82 lands.

## What the original does

Not applicable (test-gate tooling).

## How it was proved

Gate log read by hand; test source read by hand. Not re-run under load.

## Not verified / not 1:1

- Did not run `freestyle_event` under load: the gate log shows it was never run, so a loaded run proves
  nothing about the failure; the source has nothing that races.
- The linker's own reason is not known: the gate's short diagnostics drop it (see K82).

## Next

The human picks an option on the page. If A, close K83 with K82's build task.
