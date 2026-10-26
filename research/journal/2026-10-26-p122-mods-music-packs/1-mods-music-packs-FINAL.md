# 1 — mods music packs (FINAL)

Task P122. 2026-10-26.

## What I did

- `track_mods::music_packs(mods)` lists `mods/music/<id>/mod.json` of kind `music` as `(id, name, songs)`,
  in id order: the id must be the folder's and a valid mod id, the name not empty; `songs` is the length of
  the `songs` list (0 without one). It reads mod.json only, the same light check `worlds::listed` does.
- The pure menu gets `Menu::set_music_packs`, kept as `music_seen` like `mods_seen` (an empty list from the
  folder being off keeps the last one, so the rows stay listed and dim). main.rs scans them beside the
  track mods at start and again when the folder row turns, and logs `mods: N music packs`.
- The Mods page lists them after the track packs under the same Installed heading (the concept's PACKS
  list does the same: track packs, then "Unleashed music" / "Untamed music" as "music pack"), each an On/Off
  row `Key::MusicPack(i)`, about "Music pack, N songs, for the User Soundtracks slider.", note
  `music/<id> in the folder`, dimmed with "The mods folder is off".
- Off is kept in the existing `mods_off` list, by the pack's **whole id** (`mxvatv_music`), so a music
  pack never shares a word with a track pack's prefix (`mxvatv`, no underscore). `mods_off`'s parse now
  also accepts a word that is itself a mod id, so a 39/40-letter music id is not dropped by the old
  "word + `_x`" check. Old settings.txt files parse as before.
- The main menu's "*n* of m packs on" chip counts both, through the new `Menu::packs_on`.
- Nothing from opentennis: it has no mods page.

## What the original does

Nothing: the original has no mods. Menus are ours to design (AGENT.md).

## How it was proved

- `tests/menu.rs` `the_mods_page_lists_music_packs_beside_the_track_packs`: rows and their order, the song
  counts, a press turns a music pack Off by its id while the same-prefix track pack and the Free Ride list
  stay as they were, the settings text round-trips both words, a 40-letter id survives the parse, Right
  turns it back on, `packs_on` is (2, 3), the folder off dims both music rows and keeps them listed, and
  music packs alone are not "Nothing in the mods folder". The P118 every-row-changes-by-pad walk now has a
  music pack in its Mods page too.
- `tests/track_mods.rs` `music_packs_list_from_the_music_folder`: good packs with 0/1/2 songs listed in id
  order; a track mod in music/, a wrong id, broken JSON and a blank name left out; no folder, no packs.
- Live: `openmx --screen mods --mods ~/repos/openmx/mods --shot` (the user's real folder, read only) logs
  `mods: 2 music packs` and draws "MX vs. ATV Unleashed soundtrack" and "MX vs. ATV Untamed soundtrack"
  under Real Elevation, MX vs. ATV Unleashed, MX vs. ATV Untamed and Worlds, all On.

## Not verified / not 1:1

- Switching a music pack off does not change what plays yet: no music pack is played at all until MD7
  (the music player reading mods/music). MD7 should skip a pack whose id is in `settings.mods_off`, or every
  pack when `settings.mods` is off. The row's words say "for the User Soundtracks slider" after the concept.
- A music pack's mod.json is only checked as far as the list needs (kind, id, name, songs' count); its
  standard number, version and song files are not, since nothing reads the songs yet (MD7's to check).
- A pack the folder lost while the shell runs stays listed until restart, as the track packs do (P118's rule).
- Landed with --skip-audit: the audit's only findings are other tasks' addresses in plan/DONE.md (lines 121, 372), none in this change.
- The gate's first run went red on machines_everywhere, one_fps and ecosystem at load average ~61; all three passed alone at load 14, and the second full gate passed (2165 tests).
- Not compared against the concept by a side-by-side shot: the page is the P118 layout with more rows.

## Next

Nothing for P122. MD7 reads `mods_off` when it plays the packs.
