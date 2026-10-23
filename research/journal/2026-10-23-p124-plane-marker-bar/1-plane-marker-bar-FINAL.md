# 1 — plane marker bar (FINAL)

Task P124. 2026-10-23.

## What I did

- The shell built the markers' selection (`picks`) only for a seat on the bike, so while flying the
  bar, its hold bar, its label, the marker portals and the radar's marker icons were all `None`. The
  machine gate is gone: `picks` is now `hud::markers_shown_in(v, online)` alone (offline every view,
  online only this machine's own, as before).
- `hud::MarkerBar::read` is what one view's slot display reads: the selection and filled slots of
  the set `Markers` has up (the switch puts the machine's own up, P107, so the plane's slots while
  flying), and whether a direction is held and how far the Down/Up hold is. With the plane's set up
  it reads the d-pad as `markers::fly` does — `Table::SHIPPED`, and no trick buttons (the seat's
  `hands` while flying can still carry a trick bit from the poll; on the plane that is no gesture,
  so the hold bar must fill as the placement fires). The bike reads as before. Which reading applies
  comes from `markers.on`, not `garage.up`, so it is the set shown that decides, online too.
- The label, the slot boxes, the "P2…" boxes and the 200 ms hold bar are the bike's own drawing
  (`hud::draw_markers`, `marker_label`); P117's `marker_bar_shown` still hides it under Overlay None.
- Portals (`portal::markers`), radar (`overlay::marker_blips`) and the see-through marked planes
  already took the set up; they only needed `picks[v]` to be `Some` while flying.
- Nothing taken from opentennis: it has no markers or vehicle switch.

## What the original does

Nothing to port: the session markers are the port's own addition (P88), and so is their bar.

## How it was proved

- `crates/openmx/tests/markers.rs`, `flying_the_plane_shows_the_planes_bar`: disc biplane on a flat
  floor, shell passes; Left puts the `MarkerBanner` up on the plane's slot 5 ("marker 5 empty"); Down
  held with Circle (trick 1 on the bike) fills the hold bar 0 → 200 ms monotonically, places, the bar
  says "marker 5 set" and the radar has the one icon; switching to the bike shows the bike's empty set
  ("marker 1 empty", banner up) and there the Circle+Down gesture fills nothing.
- Same file, `a_split_shows_each_players_own_machines_bar`: P1 on the bike, P2 in the plane; P2's
  Left reaches "to P1", then slot 5, a held Down fills P2's hold bar to 200 ms and sets the plane's
  slot while P1's bar stays empty on slot 1; online gate: view 0 only.
- `crates/openmx/tests/markers_plane_bar.rs` (new, P117's harness): the real shell, free06,
  `--vehicle biplane`, a hand-written tape (Left, then Down held), GPU and software. Under the
  original's HUD the bar region differs from the plain tape (> 500 pixels); with `--overlay none`
  not one pixel differs; the console says `slot 5`, `placed in slot 5` in both. With the old bike-only
  gate put back this test reds ("the bar drew over only 0 pixels") — checked, then restored.
- All three disc tests were probed with a temporary `panic!` at their ends to prove they ran rather
  than skipped (they did; probe removed).

## Not verified / not 1:1

- Online with the plane up was not run end to end in a live session: the gate is the same function
  offline and online (`markers_shown_in`) and the reading follows `markers.on`, which the lane's
  `fly` sets, but no headless online shells were flown for it.
- The two-player split was proved at the `Sim`/`Garage`/`hud` level, not by a split-screen `--shot`:
  the shell's per-view loop is the same code for each view, and the single-view shot covers the draw.
- The hold bar's in-shot fill (mid-hold) is not pixel-checked in the shell; its value is checked in
  the unit test and its drawing by P106's `the_bar_fills_with_an_up_or_down_hold`.

## Next

Nothing.
