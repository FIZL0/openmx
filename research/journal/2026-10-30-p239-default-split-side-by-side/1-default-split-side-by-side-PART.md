# 1 — default split side by side (PART)

Task P239. 2026-10-30.

## What I did

- `split::Split`'s `#[default]` is now `SideBySide` (the original's `v`), so every place that reads
  `Split::default()` — `Settings::default()` (the menu's local game and the P76 mid-ride join, which
  take `settings.split`), the shell's `Args` default for `--players 2` with no `--split`, and
  `menu::layouts` for counts other than two (unaffected: quarters/eighths) — lays a pair out side by side.
- The write-back pinned the old default: `Settings::text()` always wrote `split = h`, and the user's own
  `~/repos/openmx/settings.txt` has exactly that line. A pre-P239 `split` line cannot tell a choice from
  the old default, so the setting now lives under a new key, `split_screen`; an old `split = …` line
  loads and is dropped (like P177's `machines`). A choice made on the Split screen since is written as
  `split_screen = h|v` and wins on the next load. `--split h|v` on the command line still wins for a
  `--players` ride, as before.
- `--help` now says v is the default.
- Tests: `split.rs` `a_pair_with_no_split_chosen_is_side_by_side` (default → SideBySide, `--split h` →
  Stacked); `tests/menu.rs` `a_pair_with_no_split_chosen_rides_side_by_side` (fresh settings write
  `split_screen = v`, an old `split = h` file reads side by side, a `split_screen = h` file stays and
  round-trips Stacked, a local game of two with nothing chosen is SideBySide). The existing menu tests
  that pinned a join's Stacked layout now expect SideBySide; world_look's `two_views` (which reads the
  shot's top and bottom halves) now passes `--split h` explicitly.
- Nothing taken from opentennis: it has no split layouts of this kind.

## What the original does

The original lets the player choose stacked (layout 1) or side by side (layout 2); which one its own
front end starts on is not what this task is about — the default here is the user's call (2026-10-30).

## How it was proved

Unit and menu tests above; `./kit check --all` via `./kit done`.

## Not verified / not 1:1

- A user who deliberately picked "One Above The Other" before P239 loses that pick once (their old
  `split = h` line is indistinguishable from the old default's write-back); they pick it again and it is
  kept under `split_screen`. Chosen on purpose, since the user's own file holds the pinned default.
- Not checked by eye in a windowed two-player ride; the layout choice is covered by the tests only.
- The gate ran with `OPENMX_WORLDS` pointed at an empty folder: `disc_worlds`' world_mxu_free test
  compares against the user's installed `mods/` `mod.json`, which was rewritten on 10-30 by something
  newer than any branch's stitcher and reds on every slot. P239 never touches the stitcher.
- The original's own front-end default split was not looked up (not needed: the default is the user's).

## Next

none
