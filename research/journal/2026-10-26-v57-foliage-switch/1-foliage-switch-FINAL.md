# 1 — foliage switch (FINAL)

Task V57. 2026-10-26.

## What I did

- `menu::Departure::Foliage`, row "Leaves", choices Crisp (ours: V56's cut-out leaves,
  `eco::Foliage::Cutout`) and Original (the original's blended leaves, `eco::Foliage::Original`).
  `is_original` / `set_departure` cover it, so `Menu::set_original` sets it and the master switch
  reads Mixed when only it is turned back.
- `Settings::foliage`, written to settings.txt as `foliage = original` only when it is not ours
  (in `original_text`, beside `plants` and `picture`), so older files are written as they were; an
  unreadable word keeps ours.
- `eco::Foliage::{ALL, word, from_word}` (`cutout` / `original`), as `Lod` has.
- The shell reads it: `--foliage cutout|original`, otherwise the front end's setting, set on the
  ride's grove (and printed as `foliage: …` beside `plants: …`), on every world tile's grove as it
  is taken, and on a pause-menu change on all of them and the GPU renderer's copies
  (`Renderer::set_plant_foliage`; `add_props` now copies foliage as well as lod onto a new tile).
- **Which tab:** the task says "under P126's Original tab", but P128's rule (user, 2026-10-23) puts
  P126's graphics sub-switches on the Graphics tab, and Fog / Plant Detail / Picture Shape all
  live there. Leaves is a graphics switch, so it sits on Graphics under World, right after Plant
  Detail — still one of the Original master switch's sub-switches. Decided unattended; it is one
  line in `Departure::tab` to move it.
- Nothing taken from opentennis: it has no such switch; this follows P126's own pattern.

## What the original does

Nothing new here: V56's journal describes the original's blended, depth-writing leaf materials,
which `Foliage::Original` draws.

## How it was proved

- `crates/openmx/tests/menu.rs::original_on_sets_the_leaves_and_turning_them_back_leaves_the_others`:
  fresh is Crisp and writes no line; the Original switch by pad sets Original and writes
  `foliage = original`; Leaves turned back by pad on the Graphics tab is Crisp, every other
  departure stays the original's, the master reads Mixed and the line is gone; the word parses and a
  bad word keeps ours. P126's existing tests iterate `Departure::ALL`, so they cover it too
  (Original on sets each, each turned back leaves the rest, each round-trips through the files);
  P128's Graphics-tab test lists the new row.
- `crates/openmx/tests/original_mode.rs`: the shell started from a store with Original on says
  `foliage: original`, with it off `foliage: cutout` (needs the disc; it is present here, the off
  branch unwraps it).

## Not verified / not 1:1

- The pause-menu change mid-ride (`set_plant_foliage` and the world tiles' groves) is not exercised
  by a test or a shot: it compiles and mirrors the Plant Detail path, but nobody looked at a ride
  flipping Leaves live.
- No new before/after shot: V56 already compared the two foliage drawings on both renderers; this
  task only routes the setting.
- Placed on the Graphics tab, not the Original tab the task names (reason above).

## Next

Nothing.
