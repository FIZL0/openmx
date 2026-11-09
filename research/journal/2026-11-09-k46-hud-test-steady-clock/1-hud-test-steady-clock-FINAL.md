# 1 — hud test steady clock (FINAL)

Task K46. 2026-11-09.

## What I did

- `crates/openmx/tests/hud.rs` `first_frame` raced two things, both removed (no timeout touched):
  1. **The wall clock.** Every test compares two or three separate shell runs' frames (with/without the
     HUD, plain vs `--overlay hud`). Headless rides tick by the wall's clock, so on a busy machine one run
     rides more ticks than the other before its second picture and the frames part outside the HUD too
     (the P210 red: "a plain launch is 1415 pixels off --overlay hud's frame"). Each run now passes
     `--steady-clock` (P52's `pace::Clock::steady`), so two pictures are two sixtieths of ride whatever the
     wall took.
  2. **Shared files.** Runs used the checkout root as cwd, so a `settings.txt`/`controls.txt` (and `mods/`)
     left beside the disc by a `--menu` run — or written by anything else running there — decided the ride
     (the P324 and 10-23 reds of `the_pause_menu_keeps_the_hud_under_it`). Each run now gets a store of its
     own under `CARGO_TARGET_TMPDIR/v28-<name>/`: a `kit.toml` naming a `disc.iso` link to the configured
     disc, as `tests/original_mode.rs` does. The shot is written there too.
- Which of the five tests reded in P315g's gate is not in any log I could find (the runner kept none);
  both races cover every test in the file.
- Nothing taken from opentennis: its tests do not ride a shell for frames.

## What the original does

Not applicable: a test-harness fix, no game behaviour.

## How it was proved

- Repro under load: the debug shell pinned to one core with two busy loops on the same core,
  `--renderer software --frames 2 --shot`, plain and `--overlay hud`:
  - wall clock: "ridden for 1.7s" vs "1.6s", the two PNGs differ (`cmp` at byte 1685);
  - `--steady-clock`: "ridden for 0.1s" both, PNGs byte-identical.
- `./kit check -- -p openmx --test hud`: 5 passed, and the per-run store dirs with their `shot.png` were
  written (the disc was found; not a silent skip).

## Not verified / not 1:1

- The P315g gate failure itself was not seen (no log kept), so which race it was is inferred: the wall
  clock is the one load alone can trigger, and it is reproduced above.
- The GPU renderer's pair was not repeated under the pinned-core load; the clock change is the same for it.

## Next

None.
