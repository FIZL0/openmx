# 1 — online network row fits (FINAL)

Task V111. 2026-10-29.

## What I did

- The main-style list (`list()` in `crates/openmx/src/menu_screens.rs`) now takes the plate's width
  (`room`, 18 ems, the `strip(10.0, 18.0)` every list screen draws). A row whose lit pill (heavy type
  plus the pill's pad either side) would be wider than that has its type shrunk by the ratio, so
  "Network: Internet + LAN" fits; a row that already fits keeps `ITEM` size. The size is measured in
  heavy type whether the row is lit or not, so the row does not change size as the cursor passes.
  The row pitch and its middle are unchanged, so no other row moves; the shrunk row's pill is its
  own (smaller) height, centred where the full-size one was.
- `strip()` pushes a new `Part::Plate` so tests can see the plate's box.
- Chose shrinking over `fit()`'s ellipsis: "Network: Internet + …" would hide the one word the row
  is there to say.
- Nothing taken from opentennis: its menus have no such row.

## What the original does

Front-end is ours to design (AGENT.md); the original has no Online screen.

## How it was proved

`crates/openmx/tests/menu.rs` `every_online_row_sits_on_its_plate`: at 640x448, 1920x1080,
2520x1080 and 360x640, with LAN Only and Internet + LAN, the cursor on each of the five rows, every
row's text box and the pill are inside the plate (±0.5 px), and with the cursor on Network the four
rows above have the same boxes in both modes. With Online's room set to 1000 (no fit) the test fails;
with the fix it passes. `every_pill_fits_its_label_and_nothing_covers_the_logo` still passes.

## Not verified / not 1:1

- No screenshot looked at by eye: the menu `--shot` runs never exit here, and the geometry test
  covers what the task asked. How the smaller type reads beside its full-size neighbours is unjudged.
- The other list screens (Main, Pause, Split, Lobby) get the same fit; any of their rows that were
  already past the plate (e.g. a long lobby status row) now shrink too. Not enumerated which, if any.

## Next

-
