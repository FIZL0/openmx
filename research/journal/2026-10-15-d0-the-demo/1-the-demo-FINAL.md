# 1 — the demo (FINAL)

Task D0. 2026-10-15.

## What I did

- `./kit demo --check` → `12/12`. The synthetic program builds, dumps, decompiles, gets called by the
  oracle and compared against a Rust port, all without a disc.
- Read `docs/EXAMPLE.md` and `docs/PROVING.md` alongside. The part worth carrying forward is the closing
  section of the walkthrough: the demo program is *kinder* than a real one in three ways — its functions
  are small and leaf-ish, its floats are well-scaled, and it has symbols. A real game has none of that, so
  "the oracle agrees over 10,000 inputs" is a much stronger claim here than it will be later.
- `cargo test --workspace` passes on a fresh checkout with no game data: everything that needs the disc
  skips with the command that would produce it.

## What the original does

Nothing — no game code was touched in this task.

## How it was proved

`./kit demo --check`, 12 of 12 steps. That is the kit's own self-test, not a claim about MX Unleashed.

## Not verified / not 1:1

- Nothing about this game. This task's only output is that the tooling works and that I know what its
  output looks like.
- The demo's oracle runs against a program the kit generated, so a green `demo` says nothing about whether
  the R5900 interpreter handles the instructions *this* game actually uses. That gets tested for real in
  O1.

## Next

D1: the disc's identity into `kit.toml`, and `./kit doctor` as clean as it can get without a BIOS.
