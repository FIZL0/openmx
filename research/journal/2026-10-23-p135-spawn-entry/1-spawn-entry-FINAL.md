# 1 — spawn entry (FINAL)

Task P135. 2026-10-23.

## What I did

- The marker bar has a **Spawn** entry now, the first place in the bar, drawn apart from the five
  slots: in the corner bar it is a wider box left of slot 1 (a small square inside it when the
  machine has a start to go back to), on the ride panel an "S" disc before a rule, then the slots.
  Left from slot 1 reaches it, Left from it goes to the last player (or wraps to slot 5); Right from
  it is slot 1. The ride still starts on slot 1, not on Spawn, so a first Down still places a marker
  as before — decided so, because P131/P132 tapes and players expect Down-at-once to place.
- In the sim (`game-sim/src/markers.rs`) the selection is `Markers::at = SPAWN` (`u8::MAX`). Picked so
  every old `at` value keeps meaning the same slot/player, and a saved or hashed state from before
  P135 reads the same. `Pick::Spawn` is the first variant; `Did::Cleared` is new.
- **Up on Spawn** rides the machine back to its own start at rest (the P132 home: the bike's
  `Spot::still` on the track start, the plane's `flight.reset` in its bay). This used to live on slot 1.
  **P132's slot-1 spawn is taken out**: `HOME_SLOT` and its branches are gone, so all five slots are
  the player's own again — Up on an empty slot 1 is refused like any empty slot.
- **Down on Spawn** never places. Held `CLEAR_MS = 500` ms (its own constant, not `HOLD_MS` 200) it
  calls `Markers::clear()` — the same clear pause's Clear Markers does, so every slot on every machine
  (bike and plane) goes. Released earlier it does nothing (no Refused either; at 200 ms there is no
  event at all on Spawn). The fire is the tick the held time crosses 500 (`clear_fired`), so holding
  on does not clear twice.
- The hold fill: `holding_on` scales a Down hold on Spawn into HOLD_MS units
  (`held.min(500) * 200 / 500`), so the bar's fill code is unchanged and fills over the longer hold.
- After a clear the banner and bar say **Cleared** ("cleared" in the corner bar, "Cleared / every
  marker" on the panel) until the selection changes or the banner times out. The shell counts clears
  per seat (`Seat::clears`) and `MarkerBanner::cleared` notices a new count.
- Online and split: the clear happens in the sim, per seat, on that seat's own `Markers`, so another
  seat's markers stay. Nothing new is sent; the clear rides in the inputs like any marker command.
- Tests: `tests/markers.rs` P135 section (Up on Spawn rides back at rest; Down 499 ms clears nothing,
  500 ms clears every bike and plane slot; the plane's spawn is its bay on the disc and online; the
  spawn is derived, never sent, and survives a resync; each seat its own spawn; another seat's
  markers survive a clear, split and online through `net::rides::Rides`; the bar shows the spawn and
  the clear). Older Left/Right tests count the extra place. `markers_plane_bar`, `tiled_world`,
  `track_mods` shell tapes press Left onto the spawn before Up (or Right to slot 2), since slot 1 is no
  longer home. `ride_hud` checks the spawn disc is set apart and smaller.

## What the original does

Nothing: markers are the port's own feature; there is no original behaviour to match.

## How it was proved

Integration tests above (`./kit check -- -p openmx --test markers` 136 + the shell tape tests
`tiled_world`, `track_mods`, `markers_plane_bar`; game-sim `hash`/`snapshot`/`teleport_momentum`;
openmx-net `drop_in_markers`). The plane-on-disc test printed its result, so it did run, not skip.

## Not verified / not 1:1

- Not 1:1 by nature: the port's own feature, no original to compare.
- The bar and panel drawing (spawn box width, the "S" disc, the home square, the fill pace) were
  checked by unit/integration geometry only, not looked at on screen: no screenshot taken.
- A clear's write to the markers save file goes through the existing Kept autosave of the sim's
  markers; no test reads the file back after a Spawn clear specifically.
- Online clear proved in-process through `Rides` (and the split shell), not across two real shells
  on a network.
- Hold timing in tests uses 1 ms ticks to hit 499/500 exactly; at the real 60 Hz frame the clear
  fires on the first frame at or past 500 ms (up to ~16 ms late), as HOLD_MS does.

## Next

Nothing; lands as is.
