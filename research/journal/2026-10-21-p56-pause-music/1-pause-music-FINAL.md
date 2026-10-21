# 1 — pause music (FINAL)

Task P56. 2026-10-21.

## What I did

- The pause got a fourth row, **Game Audio Options**, which is the original pause menu's own name for
  it. It opens `Screen::Audio` (`--screen audio`) over the ride, at stack `[Pause, Audio]`, so in a
  split it is driven by the pad of whoever opened the pause. The page draws in P39's style: the dim
  fill, the column, the pill list, arrows beside a row with choices, the logo, the title ("2P Game
  Audio Options" in a split) and the legend. Its ticker reads "Now playing <song>" or the disc's "No
  Song Playing".
- The page has ten rows. Song, Audio Presets and the six sliders are the original's, in its order.
  Skip Song and Play Order are this port's (see below):
  - **Song**: left and right step through every song P42's sources list (the disc's, then the user's
    folder, then any later source). Cross plays the song now. If the music was Off, Cross also turns
    it on, In Order from that song. Under Repeat Song, it makes that song the one repeated.
  - **Skip Song**: under Shuffle or In Order it goes on to the next song. Under Repeat Song it moves
    the repeat to the next song. Under Off it does nothing.
  - **Play Order**: In Order, Shuffle, Repeat Song (the song row's song) and Off. In Order is a new
    `audio::Pick::Order`, saved as `music = order`: the songs in the sources' order, starting after
    the last one played.
  - **Audio Presets**: Game, Rock-n-Roll, Night-time, Engine and Custom Mix, with the original's
    values (below). Choosing one sets all six sliders. Custom brings back the sliders the user last
    set.
  - **Music, Sound FX, Crowd, Engine, Menu FX, User Soundtracks**: 0 to 100 in steps of 5. Moving any
    of them makes the mix Custom.
- In audio.rs, `Play` gained a `bus` field (Effects, Engine, Crowd or Menu), and `Mixer` gained a
  folder, engine, crowd and menu volume.
  - Every engine voice is on the Engine bus: the bike's engine and the biplane's note. The crowd
    bed, the "oh" and the cheer are on the Crowd bus.
  - `Mixer::music_gain` uses the folder's volume for `folder:` songs and the music volume for every
    other song. This mirrors the original, where the disc's 13 songs follow Music and its ripped
    songs follow the ripped slider.
  - `volume()` is now linear (value/100), as the original's is. P42's step² curve is gone.
  - New calls: `Speakers::apply(pick, Levels)`, `play_now`, `skip`, `now` and `mixer`, plus
    `Music::play_now` and an `asked` song that plays ahead of the pick.
- Changes apply while paused. Any change raises `menu.changed`. The shell's ride loop then calls
  `Front::hear`, which now also hands the menu's `Tune`s (Play or Skip) to the speakers, and
  `Front::save`. Before every menu step the shell tells the menu which song is playing
  (`menu.music_now`).
- Settings (P36's file) gained `folder_volume`, `crowd_volume`, `engine_volume`, `menu_volume`,
  `mix` and `custom`.
  - `music_volume` and `effects_volume` now run 0 to 100, and the defaults are the Game Mix.
  - A file from before P56 has no `mix` key. Its two old 0..10 sliders are read ×10 and become a
    Custom mix.
  - The Settings screen's two volume rows step by 5, and its Music row lists In Order too.
- In a split, the settings are global, as the original's are: the original has one profile mix. So
  every player hears 1P's mix (there is one output), and the pauser's pad drives the page.
- Nothing came from opentennis. It has no music page.

## What the original does

Read off the disc and the decompilation:
- **Layout.** `ui\audiooptions.lyt` is in the in-ride UI archives (fstyleui.res, fstlapui.res,
  gstracui.res…), page `[AudioOptions]` of `ui\mxingame.pag`.
  - Captions are right-justified at x 307, there is a separator at 315, and values are
    left-justified at 326. The arrows are at x 33 and 589.
  - Row y positions: Next Artist 48, Next Song 80, Audio Presets 144, Music Volume 176, Sound FX 208,
    Crowd 240, Engine 272, UI 304, Ripped Music 336.
- **Strings.** The English strings (appstart.res `loc\l_ui_eng.bin`):
  - The pause row: "Game Audio Options".
  - Captions: "Artist", "Song", "Audio Presets", "Music", "Sound FX", "Crowd", "Engine", "Menu FX",
    "User Soundtracks".
  - Presets: "Game Mix", "Rock-n-Roll Mix", "Night-time Mix", "Engine Mix", "Custom Mix".
  - Empty states: "No Song Playing", "No Artist".
- **Recipes.** `./kit fn 0x298410` (init), `0x299360` (frame), `0x299400` (input), `0x29a7c0`
  (accept/cancel), `0x2afe90` (preset copy), `0x1fdf90` (a song's volume), `0x102670` (playlist
  index). All are in research/oracle.txt under P56.
- **Rows.** Row 0 is the song: left and right move its index modulo the count and start a 1.0 s
  countdown, and when the countdown ends the song plays as a preview (the music's vt+0x80). Row 1 is
  the preset, cycling through 5. Rows 2–7 are the sliders, at profile offsets +0xb0 music, +0xb2
  sfx, +0xb3 crowd, +0xb4 engine, +0xb5 ui and +0xb1 ripped.
- **Sliders.** Each slider moves by 5 within 0..100. A move sets the preset to Custom and is applied
  at once.
- **Accept and cancel.** Accept plays the chosen song (vt+0xa0) and saves the mix to the profile
  (+0x144). Cancel puts the song and the mix back as they were.
- **Gain.** A song's gain is value/100: the music slider's for playlist index < 13, the ripped one's
  otherwise.
- **Presets.** The table was read from the save state
  `~/.config/PCSX2/sstates/SLUS-20767 (9866FE54).01.p2s` with `./kit state extract --addr … --as
  u32`. The app pointer `*0x41f770` is 0x479990. Its `+0x3d4` gives the table at 0x50d8f0, whose
  entries sit at +0x14 + i·0x18, six ints each, in the order music, ripped, sfx, crowd, engine, ui:

  | Preset | music | ripped | sfx | crowd | engine | ui |
  |---|---|---|---|---|---|---|
  | Game | 70 | 70 | 80 | 45 | 60 | 100 |
  | Rock-n-Roll | 100 | 100 | 100 | 0 | 60 | 100 |
  | Night-time | 0 | 0 | 50 | 30 | 50 | 50 |
  | Engine | 0 | 0 | 80 | 35 | 75 | 100 |
  | Custom | 70 | 70 | 80 | 45 | 60 | 100 (as found; the user's) |

## How it was proved

- `crates/openmx/tests/menu.rs::the_pause_picks_a_song_skips_it_and_its_music_slider_is_kept`
  drives everything by pad reports alone, through the same `Reader` the shell uses. It runs real
  `Speakers` and a real `Music` on a test source of three never-ending songs, each at its own level,
  and mirrors the shell's loop: tell the menu the song playing, step, then `hear` and save on a
  change. It checks:
  - Free Ride → ride → Start → Down×3 → Cross opens the page.
  - Right then Cross plays the next song. The song is named by `speakers.now()` and its samples are
    heard at 0.7 × the song's level.
  - Play Order set to In Order and then Skip gives the song after it.
  - Music Left×2 gives 60, Custom, `Mixer::music_gain() == 0.6`, and samples at 0.6 × the level.
  - The settings that `menu::save` wrote on each change load back as `order`/60/Custom. A new
    `Speakers` built from them has gain 0.6, and a new menu's page reads "Music  60" and "Custom
    Mix".
  - Night-time sets the six sliders and the mixer's bus volumes, and Custom brings the user's back.
- The pills test now includes the Audio page: one pill under the cursor, nothing over the logo, at
  four window shapes, from the disc's art. The reachability BFS reaches `Screen::Audio` by pad.
- The settings round-trip and the old-file migration are covered in the existing settings tests,
  updated for the new fields.
- tests/audio.rs: the two engine tests set the new `engine_volume` to 1.0, as they already set
  `effects_volume`.

## Not verified / not 1:1

- **The song row plays on Cross; the original previews 1.0 s after the cursor stops.** The
  original's preview and its accept go through two music vtable calls (vt+0x80, vt+0xa0) whose
  second argument I did not interpret. Cross-to-play also suits the brief ("pick a track and it
  plays").
- **Circle keeps the changes; the original's cancel reverts the song and the mix.** The port's
  settings screens all apply and keep, as P36/P42 do. Copying a cancel that reverts would need a
  snapshot of the mix and the song.
- **Skip Song and Play Order are the port's.** The original has neither row, and its "Next Artist"
  row (layout y 48, "Artist"/"No Artist") is not ported. The port's sources do not group songs by
  artist, so the artist shows only in the song's label.
- **What the original plays when a song ends was not traced.** I assumed it runs through the list
  in order, so In Order is offered beside P42's Shuffle, and Shuffle stays the default as P42 had it.
- **Menu FX has no sounds behind it.** The shell plays no menu sounds yet. The slider is saved and
  sets `Mixer::menu_volume` (Bus::Menu) for when it does.
- **Sound FX here is every voice that is not engine or crowd** (landings, wrecks, world sounds).
  Which bus the original files each of its sounds under was not traced.
- **Gain is now value/100, linear like the original's,** where P42 had a step² curve. The original's
  sound system may itself map that gain non-linearly (not traced).
- **The page is drawn as a P39 list,** not at audiooptions.lyt's coordinates (captions right at 307
  and values left at 326). The brief asks for P39's style. The labels are written in the code in
  English rather than read from `l_ui_eng.bin`, like every other P39 screen.
- **The settings are shared in a split, not per player:** the original has one profile mix and the
  shell one output. In a split the page is driven by the pad of whoever paused.
- **The exact migration factor for old files (×10)** is the port's choice. Their 0..10 steps had no
  original equivalent.

## Next

Nothing. The task's test passes under `./kit check`.
