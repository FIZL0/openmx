# 1 — map categories (FINAL)

Task MD66. 2026-10-30.

## What I did

- **mod.json `category`** (openmx-mods standard §1, optional). `track_mods::categories` reads it from each
  mod, cleaned by `category_word` (no control characters, at most 24 characters, trimmed). The shell gives
  the menu the list beside the game list (`Menu::set_mod_categories`, `main.rs::mod_categories`).
- **Pick.** `Menu::track_list()` holds the tracks in the lit game and category. The pick, the
  "k of n" count, `track_index` and the online track list all go through it. A category pill sits beside
  the game one, with its L1/R1 chips. L1/R1 step through All Categories and each category in the lit game,
  ordered Free Roam, Freestyle, Supercross, National, Supermoto, Machine Challenge, Opencross, World,
  then anything else by name, then Other. Disc towns are Free Roam. Disc worlds and pack worlds are World.
  A mod with no category lists under Other. The choice is saved as `track_category` in settings.txt,
  which writes it only when it is set. The filter shows only when mods are listed and the lit game has
  at least two categories. A disc-only pick stays as it was, so P177's "L1/R1 change nothing" and the
  worlds-fit test still hold.
- **Browse.** `Meta.category` is in meta.json, and so in index.json's entries, which embed Meta. The
  upload's `package()` copies it from mod.json. A world with none reads World, anything else Other. On
  the Tracks tab with two or more categories, Up from the top row of cards lights a filter pill on the
  tab row. Left/Right step through the categories, and Down goes back to the cards. Other tabs ignore it.
- **stitch** (crates/stitch/src/build.rs) writes `"category": "World"` into the world meta.
- **openmx-mods** (branch `openmx-MD66`, 2d7d8dd, pushed): standard §1 row; import-mxvatv
  `category(origin, id)` with a test; elevation writes Free Roam; stitch build.rs/ultra.rs write World.
  Its own journal is `journal/2026-10-30-md66-categories`. Its check: 145 passed.
- **Existing mods updated in place:** 105 mod.json in openmx-mods `out/mods/tracks` and the 105 installed
  in the main checkout's `mods/tracks` (git-ignored data). Each got a `"category"` line after `"name"`,
  the same word the importers now write. I did not reimport them: no other byte changes.
- **Store copies:** the Hugging Face store (FIZL0/openmx-mods) has no mods/ and no index.json on main,
  only README/.gitattributes. Its one open PR (#1, 19th Hole mxvatv_mc08) belongs to another user, so I
  left it alone. There were no store copies to update.
- Tests: `the_pick_filters_by_each_mods_category` (tests/menu.rs): Freestyle lists only fs maps, and a mod
  with no category is under Other. `browse_filters_the_tracks_by_category` (tests/store_menu.rs): the
  filter keeps only that category's cards, and a card with none is under Other. `meta_json_carries_the_category`
  (modstore_tests.rs).

## What the original does

Nothing to decompile here. The category words come from the games' own activity types, read off the
Untamed disc's `Master.atyList`: AM_* misc/free ride, BT_* opencross, MT_A misc/free ride, FS high-score
challenge, Nat national, SM supermoto, SX supercross. MX vs. ATV Unleashed's mc maps load
`MachineChallenge.aty`.

## How it was proved

The three tests above, plus `./kit check --all` through `./kit done`. openmx-mods: `./check.sh` (145
passed) with the new `tests/category.rs`.

## Not verified / not 1:1

- MX vs. ATV's `mc` maps are "Machine Challenge", not the brief's "Motocross/National". The game's own
  event type for them is the machine challenge, and the brief asks for the games' own types.
- Untamed's `am`/`mt` are "misc" plus free ride in that game. "Free Roam" is the brief's word, not a UI
  string I checked.
- I did not compare the category names with the games' menus in PCSX2.
- The MXU disc tracks follow the disc list as Free Roam (towns `TRACKS`) and World (disc worlds). The disc
  has no per-track event type beyond free ride, so nothing more was read.
- No screenshot of the pick's category pill or the Browse filter pill. Their layout is covered only by
  the code paths the tests drive.
- The openmx-mods branch `openmx-MD66` is pushed but not merged into openmx-mods main. That repo's runner
  or the human merges it, as with MD37c.
- No `version` bump on the edited mod.json files. The importers' VERSION is unchanged too, so a reimport
  gives the same version. Tiles inside built worlds got no category, because the pick never lists a tile.
- No store copies were re-published (there are none of ours).

## Next

Merge openmx-mods `openmx-MD66`.
