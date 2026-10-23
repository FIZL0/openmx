# 1 — track pick by game (FINAL)

Task MD11. 2026-10-23.

## What I did

- P70's pick already had categories that Left/Right step through, but they went by the id's pack prefix
  (`mxvatv_`, `untamed_`, `world_`). They are now **filters by source game**, read from each mod's
  `mod.json` `source.game`:
  - `track_mods::source_games(mods, ids)` reads it for each listed mod (trimmed; a file that doesn't
    say is left out). `main.rs`'s `mod_games` calls it next to `mod_tracks`, both at start-up and when
    the Mods row turns, and passes the result to the new `Menu::set_mod_games`.
  - `Menu::track_groups()`: with any mod shown, **All Games** (every track) comes first, then **MX
    Unleashed** (the disc's five, plus any mod whose source is "MX Unleashed", placed right after
    them), then **Worlds** (the combined and ultra maps: every `world_` mod, whose source.game is
    "Combined"), then each other game that has a track, sorted by name. With no mods the list is the
    disc's alone, as before, so there is no filter to step.
    `Menu::track_game(id)` names a track's game; a mod with no source falls back to its pack's name.
    `arrange_mods` sorts the mods into that order (stable, so within a game they stay in folder order).
  - The lit filter is `Settings::track_filter`, key `track_filter = <game name>` in settings.txt and
    written only when it isn't All Games. Left/Right change it and mark the settings changed. A saved
    game the pick doesn't list right now shows All and keeps the saved value, so the filter comes back
    when the mod does. `track_index` (where the pick opens) is now the ride's track inside the lit
    filter, or the filter's first track.
  - The cards (`menu_screens.rs`) draw the lit filter's range. The line under each card is now the
    track's game (it used to be the pack name), and the hint reads "Game" where it said "Category".
  - Online room: the host's Track row (`in_lobby`) steps through `Menu::online_tracks()`, which is the
    lit filter's disc tracks, or every disc track when the filter has none (mods aren't ridden online
    until MD6).
- Every seat: there is one filter, in the shared settings. Free Ride, a local game's pick (the same
  Tracks screen after the join screen) and the lobby all read it.
- Tests: new `the_pick_filters_by_each_mods_source_game_and_keeps_the_filter` in tests/menu.rs. It
  writes mod.json files for two generated source games, with interleaved folder order and one id
  whose prefix names the other game, plus a world and a source with stray whitespace. It checks:
  - All Games lists every track, walked by pad.
  - MX Unleashed lists exactly the disc's five, and each game lists only its own.
  - The filter is saved, and the next session opens on the saved filter and track.
  - The online list is right.
  - A bad filter value reads as All.

  I updated P70's/MD8's category tests for All Games coming first (an extra Right; the worlds group
  is now index 2) and added `track_filter` to the settings round-trip test.
- Taken from opentennis: nothing. It has no mod tracks or source games.

## What the original does

Nothing to compare: the front end is ours (AGENT.md), and the original has no mods.

## How it was proved

- `./kit check -- -p openmx --test menu`: 134 tests pass, the new MD11 test among them.
- I looked at it: `openmx --screen tracks --mods <scratch folder linking one real openmx-mods track
  each from MX vs. ATV Unleashed, MX vs. ATV Untamed, Real elevation> --shot`. It shows "All Games"
  between the arrows, "1 of 8", the disc's cards tagged MX Unleashed, and the ◀▶ Game hint.

## Not verified / not 1:1

- **Stepping filters on the real build.** I only saw All Games on screen. Left/Right on the window
  wasn't driven, because `--shot` takes the first frame and this desktop delivers no keystrokes. The
  pad walk through every filter is proved in the unit test only.
- **Online room.** Only `online_tracks()` is unit-tested. A real two-peer lobby stepping the Track row
  was not run. Until MD6 a mod game's filter gives the room every disc track, so "the same in an
  online room" only means the disc tracks for now.
- **A mod whose source is "MX Unleashed"** goes in the MX Unleashed filter after the disc's five (it is
  that game's), so that filter is exactly the disc's only while no such mod is installed. None exists
  in openmx-mods today.
- **The filters are one per game, not one per pack.** Packs from the same game share a filter.
  "Real elevation" is a game of its own, as its mod.json says. The Mods page still lists by pack (P118).
- **No filter for a single game.** A game named exactly "All Games" is filed under its pack's name.
  With no mods there is no filter at all (All and MX Unleashed would be the same list).

## Next

Nothing for MD11. MD6 should widen `online_tracks()` to the mod tracks peers share.
