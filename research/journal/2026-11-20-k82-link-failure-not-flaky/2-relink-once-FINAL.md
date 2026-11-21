# 2 — relink once (FINAL)

Task K82. 2026-11-21. The person picked option A from part 1's page: "log linker stderr + relink once +
its own headline".

## What I did

- `ps2kit/cargo_gate.py` `build`: a compiler message whose text starts `linking with ` now writes its
  child notes to the log as `  = note: …` lines. Those notes hold the linker's stderr. The linker's
  command line, the first note, is cut to 300 characters.
- If the build failed and every compiler error was a link error, `build` runs the same build once more.
  Cargo keeps the compiled objects, so the second go only links.
  - It passes: the gate goes on, and the verdict says `linked on a second try, the link step failed
    once: <targets>`.
  - It fails on links again: the log gets `LINK_FAILED`, a fixed `(kit: …)` line.
  - Any compile error: no relink, as before.
- `ps2kit/checks.py` `summarise`: with that line in the log, the headline is `LINK FAILED (the code
  compiled, the linker failed, and again on a relink; no tests ran):`, then the error lines and the
  linker's notes (not its command line). Before, it read `BUILD FAILED`.
- Checked what rustc's JSON holds for a link error with a scratch crate calling a missing extern fn:
  the third child note is the linker's stderr (`ld.lld: error: undefined symbol …`).

## What the original does

Not applicable (kit tooling).

## How it was proved

- `tests/test_gate.py`: a fake linker script (`-C linker=` through RUSTFLAGS, its own target dir) that
  fails once while `linkflake` exists, and every time while `linkbroken` exists.
  - Once: the gate passes, the linker's stderr is in the log, the verdict names the relink.
  - Every time: the gate fails with `LINK_FAILED` in the log and both goes of the linker in it.
  - A compile error: no relink.
  - 80 passed, 0 failed.
- `tests/test_plan.py` `test_check_summary`: a log with `LINK_FAILED` gets the `LINK FAILED` headline,
  the linker's words, no command line, and no `BUILD FAILED`.
- My first try named the fake linker `ld.sh`. Rustc then passed it raw ld flags, and every link failed
  twice, which showed the "fails again" path working on a real cargo run.

## Not verified / not 1:1

- Why P383's links failed: still unknown. This change only makes the next one say why in the log.
- The relink was not tried against a real load-caused link failure. I could not make one happen on
  purpose; the test uses a fake linker instead.
- `tests/test_plan.py` has 2 failures that are not this change and were there before it: "every task
  line names the files it touches" (plan/TODO.md lines) and "docs and CI files do not need a re-test"
  (`LICENSE*` in the default list). Neither file is in the test gate.
- The installed agent-runner has no `BUILD FAILED` or `TESTS FAILED` in its source, so the new headline
  should change nothing there. Its repair sessions read the headline as text; not tried.

## Next

K80, K81, K83, K84 and K85 came from the same link failure. They can be closed with this change.
