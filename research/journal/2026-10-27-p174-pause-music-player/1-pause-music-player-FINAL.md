# 1 — pause music player (FINAL)

Task P174. 2026-10-27.

## What I did

- **The player** (`crates/openmx/src/menu_player.rs`, new): a glass box under the pause's side panel
  with the song playing ("NOW PLAYING" / "PAUSED" / "MUSIC" when the music is off) and three discs —
  back, pause/play, skip — each with its pad button under it. It shows on the pause whenever the
  pause's own screen is up and no confirm is over it (`Menu::player_shown`), for every seat's own
  pause in a split.
- **Pad**: on the pause, L1 = back, Square = pause/play, R1 = skip (`player::press`, read in the
  pause's step before its rows). None of the three did anything on the pause before.
- **Mouse**: the windows now report left-button presses — X11 selects ButtonPress and reads the
  event's window x/y (`window.rs`, with a unit test on a fake event); winit tracks `CursorMoved` and
  takes `MouseInput` Left/Pressed (`gpuwin.rs`). `Screen::clicks` hands them over; the ride loop maps
  each through `cursor::to_frame` (the frame centred in the window at its integer scale, as both
  renderers present it) into the pause view it falls in (`Menu::pause_views`) and calls
  `Menu::click` under that seat (`with_seat`). The hit test uses the same stage boxes the draw does
  (`screens::fit`, factored out of `Pen::new`), so a click lands where the disc is drawn. A click
  makes the Accept click sound, a refused one (music off) the Locked one.
- **Back and hold in the music** (`audio.rs` `Music`): `played`, the last `HISTORY` = 20 songs
  started, newest last; `back()` drops the one playing and asks for the one before it (not pushed
  again), or with nothing before replays the one playing from its start; Shuffle / In Order then go
  on from it as the pick says. `hold(on)` stops `mix` reading the stream, so the song stays exactly
  where it was; `Speakers::skip`, `back` and `play_now` let a held song play again.
- **Menu → shell**: `Tune::Back` and `Tune::Hold(bool)` beside Play/Skip, handled in `Front::hear`;
  the menu's `music_held` is told from the sound each ride frame as `music_now` is. A control sets
  `changed`, so the shell hears it that same frame — the change takes while the pause is up.
- Under Music Plays: One Song, back steps the Song choice back one (as Skip steps it on).
- Settings > Audio gets the same two as rows after Skip Song: Previous Song and Pause/Play Music.
- The pause's "Now playing" chip went: the player says it now.
- From opentennis: nothing taken. Its pointer handling (`hst/src/play/main_menu/pointer.rs`) is
  Bevy's picking, which this shell does not have; its audio has no skip/back history.

## What the original does

Nothing like it: the original's pause has no music controls (the user's own addition, front end —
"yours to design" in AGENT.md). The original's music is untouched with the player unused.

## How it was proved

- `tests/menu.rs` `the_pauses_player_goes_back_holds_and_skips_by_pad_and_click`: a real Menu walked
  into a ride's pause by pad reports, wired to `Speakers` on the Hum songs: R1 moves on, L1 comes back
  to the same song, Square holds it (silent, same song) and plays it on, a click at each disc's drawn
  centre (`menu::deck_point`) does the same, a click elsewhere misses, R1 from held plays; the
  Audio tab's two rows are there; music off → no tunes, the click refused; no player under the
  Main Menu confirm.
- `tests/audio.rs` `the_music_goes_back_a_song_and_holds_where_it_is`: back from the first song,
  back from the second, In Order going on after a back, a 0.5 s song held a full second is the same
  song when let go, the history capped at 20.
- `tests/cursor.rs` `a_click_is_read_in_the_frame_the_window_shows`, `window.rs`
  `a_left_button_press_is_a_click_where_it_went_down`.
- Looked at: a `--headless --software --screen pause` shot at 1280x720 — the box sits under the side
  panel, clear of the rows, the chips, the speedometer and the hint tab.

## Not verified / not 1:1

- **A real mouse click in a real window was not tried**: the desktop here takes no synthetic input
  (memory: no keystrokes/GUI automation reach the window), so the X11 ButtonPress path is proved only
  on a fake event and the winit path not at all beyond compiling. The coordinate mapping is tested
  as a pure function; that both renderers centre the frame at its integer scale was read from
  `window.rs` `present` and `gpuwin.rs` `draw`, not measured.
- **Not heard by ear**: the hold and back were checked on the mixer's output in tests, not through
  speakers.
- **The front end's own menu loop (main menu, not a ride) reads no clicks**: the player is only on the
  ride's pause, which is the only screen that has it.
- **The held state lasts past the pause**: hold the music, resume and it stays silent until Square /
  a click / Skip / a song pick plays it — and it stays held into the next ride. Chosen on purpose (a
  pause button means paused), written here in case it reads as a bug.
- Back under Shuffle goes back through the history; after a back, the next song is a fresh shuffle
  pick, not the song that was skipped back from (no forward history).
- A split-screen click was not exercised in a test (the mapping through `pause_views` /
  `with_seat` is the same one the draw uses, read not run).
- Landed with `--skip-audit`: the audit's seven leaks are all in plan/DONE.md (radar_update, the
  biplane_tire_* names), none from this task's files; the test gate itself passed.
- The GPU shot of the player was not taken (the software one was); both draw the same sprites.

## Next

Nothing; the task is done if the gate is green.
