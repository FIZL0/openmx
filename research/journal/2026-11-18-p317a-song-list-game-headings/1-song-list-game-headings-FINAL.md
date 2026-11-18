# 1 — song list game headings (FINAL)

Task P317a. 2026-11-18.

## What I did

- `menu_pages.rs`: `Row` has `heads`, a heading line over each choice in its open dropdown where it is not
  empty. The Song row's game names go there (each game over its first song), not into `tips` beside the
  song as P317 had them.
- `menu_screens.rs` `drop_spots` takes `heads`: it lays out headings and choices as lines, scrolls round
  the cursor's line, keeps a game's heading in sight when the cursor sits on that game's first song at
  the top, and returns heading boxes apart from `items`. The cursor (`walk_list`) and clicks only ever
  see `items`, so headings are never picked. A click on a heading keeps the list open and picks nothing.
  `drop_list` draws headings as a page's heading rows do (upper case, the HEADING colour, smaller).
- The side panel's radio list has no heading lines: it still shows the game beside its first song
  (`heads` used as a hint there).
- `Menu::list_head_point` (a heading line's middle, like `list_point`) for the test.
- `pad.rs` needed nothing: pad presses go through `walk_list`, which steps over choices only.
- Nothing in `../opentennis` does dropdown headings, so nothing was taken.

## What the original does

Nothing like this: the menus are ours (AGENT.md).

## How it was proved

- `tests/music_packs.rs` `the_settings_choose_which_games_music_plays`: `heads` names the four games;
  with the list open on One Song each game has a heading line above its first song; Down steps song to
  song (0→1→2→3→0) over the headings; a click on a heading leaves the list open on the same song; a
  click on a song picks it (`mod:untamed_music/b`).
- Menu shots (release build, private headless weston, scratch iso symlink, `--mods` with the installed
  `mxvatv_music` and `untamed_music`, `music = disc:am`): `--screen audio --menu-row Song --open-list`
  at 1920x1080 and 2340x1080 (phone layout). "MX VS. ATV UNLEASHED" is a line of its own above
  Revolution - Authority Zero, set apart from the songs.

## Not verified / not 1:1

- Not shot on a real phone or the web build: the phone layout is the desktop shell at a 2340x1080 window.
- Not tapped on a touch screen: the click path is the one taps use, proved by the unit test only.
- The side panel's choice list still names the game beside its first song (cut to "MX vs. ATV…" there),
  not as a heading line: it has no room for extra lines in its fixed count and the task named the
  dropdown only.
- A heading line takes a full choice's pitch, so a list with headings shows one or two fewer songs at
  once. This is by design, not checked against a person's eye on a phone.

## Next

none
