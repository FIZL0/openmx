# 1 — spliced pick (FINAL)

Task P149. 2026-10-27.

## What I did

- `menu::SPLICED` (`("free_all", "Every free ride")`) and `menu::DISC_PICK` (the towns plus it, 6).
  The pick's row after the five towns is the splice: `track_count`, `track_at`, `track_groups` (the
  MX Unleashed filter is `0..DISC_PICK`; mods start at `DISC_PICK`), `track_game` (MX Unleashed),
  `Settings::track_index` (a saved `track = free_all` opens the pick on it) and `start_loading` (its
  name, MX Unleashed, its picture) all know it. `spliced::ID`/`NAME` now read `menu::SPLICED`, so
  the word lives in one place.
- Its card: `Art::track_image("free_all", i)` turns through each map's stills in turn, west to east
  (picture i is map `i % 5`'s still `i / 5`), skipping a map with none. No new art.
- Online keeps to the towns: `online_tracks` and the lobby's `room_for` enumerate `TRACKS` only,
  unchanged — a world is not ridden online (as for mod tracks until MD6).
- Riding it needed no shell change: the pick's Cross is `Act::Ride("free_all")`, and the shell's
  ride already turns `--track free_all` into P71's spliced world (`main.rs`, `spliced::world`).
- Tests: `tests/menu.rs` `the_pick_lists_the_spliced_maps_after_the_five_and_rides_them` (walks Down
  five rows, Cross rides `free_all`, saved and reopened on it, loading screen, mods after it, online
  without it); the card test checks its card and ten of its turning pictures against the disc's
  stills; MD2/MD11/P70 tests updated for the sixth disc row. New `tests/spliced_pick.rs`: the shell
  from a settings file of its own with `track = free_all`, front end on the pause, which starts the
  settings' track through the same `Act::Ride` path the pick's Cross takes — the load says
  `world: free_all … 5 tiles`, `world holes: none`, and rides (`ride hash`).
- opentennis: not consulted; the change is one row in this front end's own pick, which already had the shape (the mods' rows).

## What the original does

Nothing: the front end is ours to design (AGENT.md), and the splice is our own track (P71).

## How it was proved

Menu unit tests over the pick (tests/menu.rs) and one headless shell run (tests/spliced_pick.rs),
both green in `./kit check -- -p openmx --test menu --test spliced_pick` (206 tests).

## Not verified / not 1:1

- The shell test does not press through the pick in the shell itself: the shell has no way to feed
  menu taps from a test, so the pick's walk to the row and its Cross are proved on the library menu
  (`Act::Ride("free_all")`), and the shell part starts the same `Act::Ride` from the pause screen's
  settings track. The step between them (the shell handing an `Act::Ride` id to its ride) is the one
  every track already takes.
- The card and loading pictures are the five maps' stills, not a picture of the spliced world itself:
  there is no still of it on the disc, and the shell's spawn-still job (`track_previews`) is for mods.
  Presentation only.
- Not checked by eye in a window: the card was checked by its `Part::Card` layout and image indices.

## Next

Nothing.
