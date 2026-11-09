# 1 — songs from toggle list (FINAL)

Task P317. 2026-11-09.

## What I did

Settings → Audio's soundtrack now picks its games from one toggle list, **Songs From**, shown inline (a
heading and one On/Off row per game) above the Soundtrack rows. I chose inline over a separate checklist
page: it is four or five rows, needs no new screen or navigation (so the merge stays small), and on the
phone layout (2340x1080) it reads as well as on the desktop (shots below).

- **Dynamic list** (`audio.rs` `Music::games`, `Game`): read off the music sources, nothing named in code.
  Every song every source has, grouped by game: the disc (`DISC_GAME`), each music pack by its `mod.json`
  `source.game` (falling back to the pack's `name` when the source does not say, `music_packs.rs`), then
  the music folder (`FOLDER_GAME`, "Music Folder"), each with its song count. A newly installed pack adds a
  row on its own: the shell copies `sound.games()` into `menu.music_games` wherever it already refreshed
  `music_choices` (start, store rescan, `hear` when the switches change the list).
- **Order**: `Music::tracks()` now lists the disc's songs, then the packs', then the folder's (it was disc,
  folder, packs, by source order). In Order play and the Song list follow the same order.
- **Rows** (`menu_pages.rs` `songs_from_rows`): label "MX vs. ATV Untamed · 22 songs", On/Off; pack rows
  greyed "The mods folder is off" with the folder off; the side panel's note names `music/<id>`.
- **All off is the music off**: `no_music_games()`; Music Plays shows Off greyed ("Every game under Songs
  From is off, so the music is off"), Song/Skip/Previous/Pause greyed "The music is off", and the shell
  prints `music: off, every game under Songs From is off`.
- **Player reads the picked games only**: `Music::set_sources_off` (disc/folder) beside `set_packs`;
  `Pick::Track` of a game switched off is not repeated (Song row then says "Its game is off"). Song's
  dropdown lists only the picked games' songs, and with more than one game it shows each game's name as the
  hint beside that game's first song, the songs without their " (Game)" suffix (`song_headings`).
- **Settings**: `mods_off` still keeps packs; new `music_off` (`disc`, `folder`) keeps the others,
  written only when non-empty, so an old settings.txt reads the same (every game on).
- Dropped `Key::MusicFrom`, `MUSIC_FROM`, `MUSIC_MIX` ("A Mix"). The Mods page's per-pack rows
  (`Key::MusicPack`) are unchanged.
- Shell: `music: songs from <game> (<n>), …` on start and on a change; `music: playing <id>` on the
  desktop's stdout too (it was the browser console's alone). New `--menu-row TEXT` puts an option page's
  cursor on a row for shots (`Menu::point_at_row`).
- opentennis: nothing to take, it has no music-source picker.

## What the original does

Nothing to match: front-end design (AGENT.md, "Menus and front-end: yours to design"). MX Unleashed has
only its own songs.

## How it was proved

- `tests/music_packs.rs` `the_settings_choose_which_games_music_plays` (rewritten): the games list
  (disc, two packs by game name, folder, counts), the rows' order and labels, no Music From/A Mix, the Song
  list's game hints, only-MX-vs.-ATV ids and a shuffle that never picks a switched-off game, settings.txt
  round trip and an old file without `music_off`, the last one off → Music Plays Off and greyed, nothing
  plays, only the disc, One Song on an off game silent, mods folder off greys pack rows only.
  `tests/menu.rs` settings round trip carries `music_off`.
- Menu shots (scratch dir, private headless weston, release build, installed `mxvatv_music` and
  `untamed_music` plus a one-song music folder): desktop 1920x1080 and phone 2340x1080 with the cursor on
  MX vs. ATV Untamed; phone with every game off (Music Plays Off, greyed); desktop with Song's dropdown open
  on an Untamed song (hint "MX vs. ATV Untamed" beside its first song).
- Ride lines (windowed free06 rides, sound piped to a stand-in pw-cat), three runs each:
  - `music_off = disc folder`: `music: songs from MX vs. ATV Unleashed (20), MX vs. ATV Untamed (22)`,
    then `playing mod:untamed_music/collapse_saosin`, `…/shapeshifter_celldweller`,
    `…/soul_crusher_operator`.
  - `mods_off = mxvatv_music untamed_music`, `music_off = folder`: `music: songs from MX Unleashed (13)`,
    then `playing disc:am`, `disc:itsover`, `disc:lonelyday`.

## Not verified / not 1:1

- Song's dropdown has no real heading rows: each game's name is the hint beside its first song (cut to
  "MX…" in the list's width; whole in the side panel). Headings as rows of their own need non-selectable
  dropdown items in `drop_spots`/`walk_list`/`click_page`; filed as P317a (low).
- The side panel's title cuts long rows ("MX vs. ATV Untame…") as it cuts any long label; the row on the
  page shows it whole.
- Not shot on a real phone or the web build: the phone layout is the desktop shell at a 2340x1080 window.
- A pack's songs whose own `game` differs from the pack's `source.game` are still counted under the pack's
  first song's game name (no installed pack does this).
- The ride checks used shuffle: three songs per setting, not every song; the filtering itself is the unit
  test's.

## Next

- One full `./kit check --all` went red on `hud.rs` `a_plain_launch_starts_on_the_ride_hud` ("1698 pixels
  off --overlay hud's frame") under load; it passed alone. That is K46's open flake, not this change.
