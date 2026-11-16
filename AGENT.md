# Agent notes

Rules for whoever works here, human or agent. Read this, `PLAN.md`, the task's own file and the files it names;
other docs only when the task needs them (pointers below).

A re-implementation of a PlayStation 2 game (`kit.toml` says which): same behaviour, better rendering,
maintainable code. `./kit` is the entry point for everything.

## Non-negotiable

- **Game data is read from the user's own disc at runtime.** Never commit game data, extracted assets,
  decompiled code, dumps, captures or screenshots: they live in git-ignored `context/`. About to `git add`
  something under `context/`? Stop.
- **No retail addresses or decompiler names in shipped code** (no `FUN_…`, `DAT_…`, bare hex addresses).
  Describe what a function does, not where it lived. Addresses go in `research/oracle.txt` and journals only.
  `./kit audit` checks; `./kit done` gates on it.
- **Match the original exactly, where it matters.** Simulation (physics, AI, state machines, scoring, timing)
  is ported, not approximated, and proved. No placeholders, no "close enough", no TODO passed off as done.
- **Say what you did not verify.** Every journal entry ends with `## Not verified / not 1:1`: each thing you
  could not check or match, with its reason, or `none`. The most important habit here.

## How to write replies

- Put the answer first, in a sentence or two. Then details only if they're needed.
- Use short sentences and plain words, with no jargon the reader has to unpack.
- Use bullets over paragraphs, with one idea per bullet.
- Use names the user knows (the task id, the file), not internal function names.
- When you offer options, give a few words each and say which one you'd pick.
- Leave out recaps, caveats nobody asked for, and lists of things you're not doing.
- This is only about how replies read. Do the work as carefully as ever.

## What is exact and what is not

Decided per area; don't relitigate.

- **Simulation: bit-exact.** Floats through `ps2-float`'s `F`/`Vf`, never host `f32`; enable the float lint in
  those modules.
- **Rendering: ours, and fast.** The original is a reference for what the world looks like, not how it is drawn:
  match by eye, never 1:1, prefer the optimised way. The GPU renderer is the default and must run well on
  desktop, wasm and later phones; the software rasteriser is reference/fallback only. Logic runs at the
  original's rate bit for bit; the picture draws well above 60 fps up to the monitor's refresh, interpolating
  between ticks on the render side only (user, 2026-10-20). Don't replay the GS's rasterisation or open tasks to
  chase render differences: note them and move on (user, 2026-10-20).
- **Menus and front-end: ours to design.** The original is a reference, not a spec.
- **Presentation: feels right.** Cameras, cutaways, effects, audio cues — match by eye. Just check they do not
  touch the simulation's RNG or its timing.
- **Additions are layered on top** (mods, widescreen, upscaled textures): off, the game behaves exactly like the
  original and the replay tests pass bit-exact.

Before any task, look in `../opentennis` (same owner's further-along PS2 port, our code: copy freely) for the
problem solved already, and say in the journal what you took and changed. What it has: `docs/agent/OPENTENNIS.md`.

## Proving it

Cheapest method that actually proves it (`docs/PROVING.md`): pure function of args + RAM → **the oracle** over
≥10,000 generated inputs; timing/frame flow/state → **a live capture** (`./kit record`); tables, layouts → **a
save-state RAM image**; the whole thing → **the replay harness** (same input and seed, same per-tick hash).
First read the function in MX vs. ATV's PC exe (`./kit fn … -o pc`, `research/mxvatv-pc.md`), list its
branches, clauses and constants, aim oracle inputs and short captures at each, file any clause the port lacks.
The exe is a guide, never the oracle; bit-exact proofs are against the PS2 ELF and PCSX2. No proof, or "I read
the decompilation carefully", means not done.

## Context and tests

Everything read is paid for every turn (`docs/CONTEXT.md`). Never read a decompilation dump whole (`./kit fn`,
`./kit xref [--similar]`, `./kit dis`; a hook blocks it). Pipe output through `head`/`tail`/`grep`, read files by
line range. Tests only via `./kit check` (one line; says whether the build broke or a test failed), and while
working only the ones you touched (`./kit check -- -p game-sim --test ride`); three check slots serve every agent,
the full suite runs once, in `./kit done`. A run skips `gate.toml`'s `[[slow]]` binaries unless your branch
touches their paths, and reruns `[flaky]` load flakes alone once: touch what one proves without matching → `./kit
check --all` and add the path to `gate.toml`. Where code lives: `docs/agent/CODEMAP.md` (`./kit plan <crate>`),
never grep.

## The live game

Detail in `docs/PCSX2.md`. One PINE connection at a time (read static values from a save state during a
capture); don't touch the emulator while a capture runs; never send `select` on the virtual pad; run multi-step
jobs as one `./kit pcsx2 with -- CMD`; `CAPTURE FAILED` = incomplete file, fix and re-record; close your copy
with `./kit pcsx2 stop`, never kill it by name.

## Other docs

Read one only when the task touches it: `docs/PCSX2.md` (emulator), `PROVING.md`, `PS2.md` (hardware),
`GHIDRA.md`, `TROUBLESHOOTING.md` (a tool fails), `INTERNALS.md` (changing `ps2kit/`), `WORKFLOW.md` (long runs),
`docs/agent/TASKS.md` (plan commands, push rules, `./kit run`), `docs/agent/CODEMAP.md`.

## Merging

**Never merge, rebase, push the shared branch or resolve a conflict by hand.** `./kit done` pushes your own
branch; the runner merges, tests and pushes the shared one (`docs/agent/TASKS.md`). A worktree of another repo
(an openmx-mods branch for your task) or other ad-hoc work goes in `$(agent-runner home)/wt/<slug>`, never beside
the checkouts in `~/repos` (`../openmx-mods-<id>`); `git worktree remove` it once its branch is pushed.
