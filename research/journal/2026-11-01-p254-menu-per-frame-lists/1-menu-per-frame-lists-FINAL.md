# 1 — menu per frame lists (FINAL)

Task P254. 2026-11-01.

## What I did

- `menu.rs`: a `Lists` (the shown mods' places in `mod_tracks`, each track's category, the game
  filters, the lit filter, the categories in it, the lit category, the lit filters' tracks), made by
  `Menu::make_lists` and kept in `Menu::lists` (`ListCache`: a `Mutex<Option<Arc<Lists>>>` and a build
  counter; equal to any other, so `Menu`'s `PartialEq` and V30's look comparison are as before). It is
  dropped by `set_mod_tracks` / `set_mod_games` (through `arrange_mods`) and `set_mod_categories`, and
  made again when `settings.mods_off`, `track_filter` or `track_category` differ from the ones it was
  made with: those are written straight into `settings` from many places, so they are compared,
  not tracked. `track_count`, `track_at`, `track_index`, `track_list`, `track_groups`,
  `track_categories`, `track_filter_group`, `track_category_group`, `track_group` keep their
  signatures and read the kept lists; `track_at` is an index now, not `nth` over a filter.
  `Menu::list_builds` counts the builds for the test.
- `menu.rs`: `Menu::catalogue` is an `Arc<Catalogue>`, so a copy of the menu (`look()`, `cued`'s
  before-press copy, `view`) shares the riders and bikes instead of copying them.
- `main.rs` (the front end's loop): `look()` is made in the CPU branch only. The GPU branch never
  compared it, but made it every frame.
- `menu_screens.rs`: the track pick's draw takes the lists once a frame and borrows them.
- `menu_art.rs`: `Type` keeps each face's strings laid out (`Line`: each glyph and its pen, and the
  width), so `measure` is a lookup and `text` places kept glyphs. It forgets them all past 4096
  strings, so a clock or a counter cannot grow it without end.

## How it was proved

- `tests/menu.rs` `a_steady_track_pick_does_not_make_its_lists_again`: on a pick with three mods, 120
  idle frames each drawn as the GPU's sprites (`Art::sprites`, at 1280x800, from the disc here) make
  the lists no more times; a `look()` copy shares them; a mod added is in the pick and `track_list` the
  next frame, and 30 more frames make no more; a game filter and a category set straight in
  `settings` list their tracks; a pack switched off leaves.
- `cargo test -p openmx --test menu`: 218 tests, all pass in s1, with the disc.
- Timing. A throwaway `#[ignore]` bench, not committed: 300 mod tracks over four games and three
  categories, on the track pick, 2000 frames of `step` plus `Art::sprites` at 1280x800, release build,
  this machine at load ~13. HEAD (ea435693) took 1391.5 µs a frame, plus `look()` 1500.5 µs, which is
  what HEAD's GPU path did each frame. This branch takes 24.5 µs, and 100.6 µs with `look()`, the CPU
  path's cost.

## Not verified / not 1:1

- The Deck's own before/after was not measured: no Deck is reachable from this machine. The
  numbers above are this desktop's, on a made-up 300-track pick, not the Deck's profile.
- The CPU path still clones the menu each frame for V30's look comparison. The catalogue is
  shared now, but the mod lists are still copied (`mod_tracks`, `mods_seen`, `mod_games`,
  `mod_categories`, about 100 µs with 300 mods). Making them `Arc`s would change `pub mod_tracks`'s
  type. The Deck draws on the GPU, so it was left.
- `Pen::fit` and `Pen::wrap` still build the prefix strings they try. Each one's measure is a
  lookup now.

## Next

none
