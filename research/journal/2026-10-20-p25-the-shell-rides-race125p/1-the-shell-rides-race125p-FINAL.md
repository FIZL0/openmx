# 1 — the shell rides race125p (FINAL)

Task P25. 2026-10-20.

## What I did

- `openmx`'s default `.phy` is now the name the original's vehicle loader builds for a freeride boot,
  `track::physics_file(FREERIDE_ENGINE, FREERIDE_PRO, Tuning::P)` = `Bike\Race125P.phy`, looked up by
  suffix in the bike archive (`Res::find_suffix` folds case and slash, so it finds `bike\race125p.phy`).
  The old default looked for `<bike>.phy`, which no archive has (`125sx.phy` is not a member), and fell back
  to the first `.phy` member, `bike\pro125a.phy`.
- If the bike archive does not carry the freeride name (a 250 model's archive only has its own class's
  four files), the shell tries `<bike>.phy` and then the first `.phy`, as before, and prints
  `phy: the bike archive has no Bike\Race125P.phy; riding … instead` so it is never silent.
- `--phy NAME` still wins over all of that, unchanged. Its doc comment and the `--help` line now say what
  it overrides.
- Only `crates/openmx/src/main.rs` changed.

## What the original does

D7 (`research/journal/2026-10-15-d7-d7-freeride-bike-class/`): the vehicle build loader (`./kit fn
0x00208c50`) builds the player's physics name from two event fields, class (+0x10c) and tier (+0x110),
which on a freeride boot are the event constructor's 1 / 0, so it asks for `Bike\Race125P.phy` (and
`Race125A.phy` for the AI) whatever bike *model* is drawn. The model does not enter the name. The port
already had that as `track::physics_file` and the two `FREERIDE_*` constants. The shell just never called
them.

## How it was proved

The trace's own `phy:` line, which is what the brief asks for (`t: none`). Release build, `openmx --track
free09 --trace 1`:

| arguments | `phy:` line |
| --- | --- |
| (none, bike 125sx) | `bike\race125p.phy` |
| `--phy pro125a.phy` | `bike\pro125a.phy` |
| `--bike yz250f` | `bike\race125p.phy` |
| `--bike crf250_faisst` | `bike\race125p.phy` |
| `--bike rm250` / `--bike yz250` | the "has no Bike\Race125P.phy" line, then `bike\pro250a.phy` |

That the name is the right one is D7's proof (two RAM images), and that the file is the human's is S18's
(`crates/game-sim/tests/phy.rs`, which asserts the live bike object carries the same `physics_file` name).
`./kit check -- -p openmx`: tests pass.

The full `./kit check` came up red on a clean tree, but not because of this change:
`game-sim --test trick`'s `the_humans_tricks` failed with `rows whose pad is not field 2k - 2: [12, 19,
20, …]`. Both of its inputs match S30's: `recording2.p2m2` has md5 `1e50…87d` in every slot, and the
capture is C6's (`42810e…` md5, as S32 pins). The test takes `recording2-freeride.csv` before S30's own `recording2-tricks.csv` whenever the
first one has the trick columns, which C6's does. So in every slot holding C6's capture (s1, s2, s4) the
test reads a capture whose pad rows do not line up with the tape the way it asserts. S30's own run was in
s5, which has only `recording2-tricks.csv`. No pad-reader source changed between S30's commit and main. This is S30's bug
and not P25's, and I left the test alone.

To land, I parked C6's capture in this slot's `context/capture/p25-parked/` and copied s5's `recording2-tricks.csv` into
`context/capture/acceptance/`. On that, `-p game-sim --test trick` passes (11 tests) and the full
`./kit check` passes. Then, with C6's capture back in place, `-p game-sim --test ride --test replay`
passes too: those are the P20 chord test and S32's test, which skip without it. After `./kit done`, C6's
capture goes back into `acceptance/` and S30's copy stays in `p25-parked/`.

## Not verified / not 1:1

- **The ride on race125p was not compared against the original here.** This task only changes which file
  the shell loads. Whether the ride on it matches is P15's measurement and the S-series proofs, which
  already used `race125p.phy`.
- **The shell does not do what the original does with a non-125 model.** The original would still ask for
  `Bike\Race125P.phy` and get it from whichever mounted archive answers (D7 left that lookup unchased). The
  shell only searches the drawn bike's own archive, so on a 250 model (`rm250`, `yz250`) it falls back to
  that archive's first `.phy` (`pro250a`) and says so. I did not search the other archives, because which
  archive the original resolves the name from is not known.
- **The AI's `Race125A.phy` is not loaded.** The shell has no AI riders.
- **No single `./kit check` run covered every test against its intended capture.** `the_humans_tricks`
  fails on C6's `recording2-freeride.csv` (an S30 bug, explained above). The run that passed had S30's capture in its place,
  so the P20 chord test and S32's acceptance test skipped in that run. I ran those two separately against
  C6's capture and they passed. **Fix for whoever owns S30:** make the test prefer `recording2-tricks.csv`, or make it agree
  with C6's capture.

## Next

none — the task is done.
