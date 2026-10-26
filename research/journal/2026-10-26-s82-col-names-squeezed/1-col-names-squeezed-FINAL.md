# 1 — col names squeezed (FINAL)

Task S82. 2026-10-26.

## What I did

- `game_sim::track::Scene::parse` now passes an `[Object<n>]`'s `COL=` value through the `.phy`
  reader's own `squeeze` (made `pub(crate)` in `phy.rs`; nothing else there changed): spaces out except
  inside `[]` and quotes, quotes out. `SLT=` is left as it was, because the renderer's `slt_name` already
  squeezes model names (V41) and the free-ride filter squeezes on its side; the `track.rs` unit test
  expecting the model `"Open Gate"` still holds.
- New disc test `tests/track.rs::every_disc_scenes_shapes_are_in_its_archives`: over every track archive's
  every `.scn`, no collision name keeps white space, and every shape whose scene spelling had a space
  (its squeezed name is not in the scene text) is a member of the track's `.res` or `_2.res` and parses as a mesh. It finds
  66 of them on the disc, all on free09, free09_ss and nat20, and all now resolve (before, `Props::load`
  missed every one of them). The count assert (`>= 62`) proves that the test ran and did not skip.
- Nothing taken from opentennis: it has no scene reader.

## What the original does

The scene's collision placement reader (`./kit fn 0x001aa760`) takes the shape name with the ini reader's
text getter (`0x0014b5e0`) and then Position/Look/Up with `ini_quadword`, all from the same reader. The ini
line reader (`0x0014a040`, oracle.txt V41/S18 notes) drops ' ', '\t', '\r', '\n' outside `[...]` and
quotes before it splits a line at '='. So `COL=Large Egg.col` asks the archive for `LargeEgg.col`.

## How it was proved

- The disc test above: 66 spaced names over 30,063 placed shapes, every one of them now found in its
  track's archives.
- free09's lanes: the egg (the one spaced shape on free09/free09_ss, one per scene) is at
  (3244.9, 106.7, 3977.2). Over every capture under `context/capture` with a bike or plane position
  column, the closest any row comes to it is 797 ft (acceptance-freeride.csv), so no captured lane can
  touch it. `./kit check -- -p game-sim` passed (671 tests) with the fix, which includes the free09
  replay/capture lanes.

## Not verified / not 1:1

- The squeeze is applied to the `COL=` value only, not to the whole line as the original does. Other
  keys the sim reads (vectors, `Type`, `Hide`, `HasCol`) are trimmed, not squeezed. No disc scene was
  checked for a space inside one of those values; vectors split at spaces anyway, and `Type` is compared
  trimmed. Reason: kept to what the task names so that the merges stay small.
- nat20 has no capture, so its newly solid ice fences, arch and ruins are not ridden against the
  original. Only the file lookup is proved: each name is an archive member and its mesh parses. Reason: there is no nat20 state or recording.
- Prop owner numbers on free09 shift by one after the egg's index. None of the gate's tests saw a
  change, but no capture shows an owner number to compare against.

## Next

Nothing.
