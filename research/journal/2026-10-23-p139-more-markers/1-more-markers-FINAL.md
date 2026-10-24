# 1 — more markers (FINAL)

Task P139. 2026-10-23.

## What I did

- `game_sim::markers`: the slots are a growing list per machine (`Vec<Option<Box<Mark>>>`), five to
  start (`SLOTS`) and up to `MAX_SLOTS` = 64. Once every slot is set, `Pick::More` (the +) follows
  the last; Down held on it appends a slot holding the marker there and moves the selection onto it
  (`Did::Placed(len - 1)`, bike and plane); Up on it is `Refused`. `Markers::has_more` says when the
  + is there. Clear Markers (P99) goes back to five slots in every set: a selection on slot 1-5
  stays, one on slot 6+ or the + goes to slot 1, one on a player keeps that player.
- Hash: the first five slots walk exactly as before (so a five-slot ride hashes as it did); slots
  past five walk as a `more` list only when there are any.
- Snapshot: the markers section's tag is now `P139mrks` and each set's slots are length-prefixed;
  decode refuses a count outside 5..=64. I chose a new section tag over bumping the snapshot `MAGIC`:
  bumping `OMX1` would orphan every saved file that embeds a snapshot, while a new tag only makes a
  state from an older build (with the old fixed-five layout) be refused as corrupt.
- Saved markers file: `FILE_VERSION` 3 writes a slot count; versions 1 and 2 (always five slots)
  still load.
- `ride_hud` panel (top right): the row of slots, +, and player pills is one strip. It widens
  leftward until its left edge would come within an EM of the score bar's right (worked out per
  view from the view's layout width), and past that it scrolls: a window grown out from the selected
  entry, so the selection is always drawn, with a chevron on each side that has more off it. The
  pills scroll with the slots, so many players in a narrow split view fit too. The label for the +
  is "Marker N new". Stateless: the window comes from the selection each frame.
- The software HUD's top-centre bar (`hud.rs`) got the same + box and a window round the selection
  with dim arrows, so it never runs off the frame.
- Colours: `overlay::marker_colour(slot)` keeps `MARKER_COLOURS` for 1-5 and generates 59 more:
  hues a golden angle apart with four saturation/brightness steps. The start hue and steps were
  searched (offline, a scratch Python script over the same HSV formula) for the widest closest pair
  among the 64 and the original's icon/rider colours: 30 apart in RGB, up from 15 with my first guess.
- `portal`, `saved_markers`, `main.rs` follow the variable count.
- No opentennis counterpart: this is the port's own marker menu; nothing there to bring along.

## What the original does

Nothing: session markers are the port's own feature (P94 onwards). No addresses.

## How it was proved

- `crates/game-sim/tests/more_markers.rs` (7 tests): no + until five set; Down on + adds slot 6
  holding the spot and Up rides back to it; 64 set shows no +; Clear Markers back to five; a 40-slot
  set survives a snapshot round trip and a saved file; too long/short sets refused; old v1/v2
  five-slot files load.
- `crates/openmx/tests/ride_hud.rs` `sixty_four_markers_scroll_short_of_the_score_bar_with_the_selection_in_view`:
  1920x1080, 1280x720 and three split views (960x1080, 960x540, 480x540), at 5, 6, 40, 63 and 64
  slots, selection on first, second, middle, last two slots, the + and two players: the panel's
  left edge is never within an EM (scaled) of the score bar, the selection is inside the panel, the
  left arrow matches the scroll, 64 always has an arrow, five slots never scroll.
- `crates/openmx/tests/overlay.rs` `sixty_four_marker_colours_are_all_apart`: 64 opaque colours,
  the first five unchanged, each at least 28 RGB apart from the others and from the original's.
- `crates/openmx/tests/markers.rs` `the_software_bar_scrolls_to_the_selection_among_sixty_three`.
- The existing marker, saved-marker, portal, online-marker and ride HUD tests still pass.
- `tests/hash.rs`'s size pin: `Sim` is 16 bytes smaller (11336), the leaf count unchanged (909).

## Not verified / not 1:1

- Not 1:1 by nature: the port's own feature, no original to match.
- Never looked at on screen: the scrolled panel and the chevrons are proved by geometry in the
  tests, not by eye in a running shell.
- Colour distinctness is RGB distance ≥ 28, not a perceptual measure; some of the 64 will look alike
  to some eyes, more so to the colour-blind.
- Slots 6+ get no number word on their start-zone portal: the disc only has `startzone_1..5`.
- Online with an older build: its states use the old markers tag and are refused as corrupt, so
  mixed builds cannot resync (they were already not meant to mix).
- The selected slot's grown ring can spill a pixel or two past its cell; the test checks the
  disc against the panel, not against neighbouring cells.
- P135 (a Spawn entry) has not landed, so nothing here places it among the + and the pills.

## Next

None.
