# 1 — mxu world pick (FINAL)

Task P280. 2026-11-03.

## What I did

- Checked the user's desktop setup, read-only: the main checkout runs from `kit.toml` (the disc is
  `MX Unleashed (USA).iso` there; `~/.local/share/openmx` doesn't exist). Its `settings.txt` has no
  `track_filter`, `track_category` or `mods_off`. `mods/tracks` holds 105 mods, including
  world_mxu and world_mxu_free (both `source.game` "Combined", category World) and world_mxvatv,
  world_ultra and world_untamed.
- Probed the pick with those 105 ids, games and categories (a throwaway test, now removed), printing
  the worlds each filter/category lists:
  - All Games / All Categories and / World: world_mxu, world_mxu_free, world_mxu_nat, world_mxvatv,
    world_ultra, world_untamed
  - MX Unleashed: world_mxu, world_mxu_free, world_mxu_nat
  - **Worlds: world_mxvatv, world_ultra, world_untamed** — no MX Unleashed world.
  So nothing shadows or drops the disc rows. The installed world_mxu{,_free} are deduped into them as
  P232 intends. The release cfg lists them on every non-wasm build (v0.10.0 has P253's DISC_WORLDS).
  They are missing only from the filter where the other worlds are, which is where the user looked.
- Cause: `Lists::groups` are ranges of pick rows. The Worlds filter's range is the world mods after
  `DISC_PICK`, while the disc's worlds are rows `TRACKS.len()..DISC_PICK` inside the MX Unleashed range.
- Fix (`menu.rs` `make_lists`): when the lit filter is Worlds, its tracks are the disc's worlds, then
  its range. The categories and the list come from that set, so up/down, the count, the cards and
  `track_index` all follow. `track_group` also counts the lit list. The rows keep their places, their
  MX Unleashed tag and their loading (P260's load path is untouched). On wasm, `DISC_WORLDS` is empty,
  so nothing changes there. With no world mod installed there is still no Worlds filter, and the disc's
  worlds are in All Games and MX Unleashed as before.

## How it was proved

- New `tests/menu.rs` `the_pick_lists_the_mx_unleashed_worlds_in_every_filter_that_has_worlds`. It
  covers four setups: no mods; world_ultra; world_mxu + world_ultra; and the user's five installed
  worlds. In each, All Games, MX Unleashed and every World category list the three disc worlds; the
  Worlds filter lists them first and then the other installed worlds, and nothing else. The pad opens
  the filter on world_mxu, walks to world_mxu_free, and Cross rides it.
- Two older tests asserted the old Worlds filter (only installed worlds) and are updated:
  `left_and_right_step_the_picks_categories_with_the_worlds_first` and
  `the_pick_filters_by_each_mods_source_game_and_keeps_the_filter`.
- `tests/menu.rs` passes except `two_choice_rows_toggle_and_only_many_choice_rows_drop` (Ride
  Handling row hints), which fails alone too and has nothing to do with the pick. Filed as a follow-up.
  `spliced_pick` and `lobby` pass.

## Not verified / not 1:1

- The Deck (release v0.10.0): I couldn't see its disc.txt or its mods folder from here. Its code has the
  same pick, so the same Worlds-filter gap applies if it has world mods installed. With none, the disc's
  worlds are in All Games and MX Unleashed after the five towns.
- I didn't look at the pick on screen; the check is the menu's lists, which the cards draw from.
- I didn't ride the worlds from the new filter. Cross makes the same `Act::Ride(id)` as from the
  MX Unleashed filter, and P260 covers that load.

## Next

none
