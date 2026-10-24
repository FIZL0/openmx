# 1 — menu sound (FINAL)

Task P65. 2026-10-24.

## What I did

- Found what the original's front end plays: its own music loop, `UI\UI_001` (two mono halves), and
  five clicks out of `aud\ui\ui.aud`'s `[UIResponseEmitter]` (up_down, left_right, accept, cancel,
  locked). Notes in research/oracle.txt under P65.
- `audio.rs`: `MenuSounds` reads the `.aud` and the clips off the disc (`MenuSounds::load`), joining
  the music's halves with `stereo`. `Click` numbers the clicks the way the `.aud` and the original's
  play calls do. The `Mixer` gained `menu_music` and `front_end`: while the front end is up, the loop
  plays under the music slider and the ride's songs hold where they are. When it changes, the two
  cross-fade over `MENU_FADE` (1 s). `Speakers::menu_sounds`, `front_end` and `click` are the shell's
  side of it. Clicks go on `Bus::Menu`, the Menu FX slider.
- `menu.rs`: every press goes through `Menu::cued`. It compares the menu after the press with the
  menu before it and queues a `Cue` (Move, Side, Accept, Back, Refused); `take_cues` hands them to the
  shell. This one place covers every screen and both drivers: pads, the keyboard, the splash, the
  join screen's seats and the pause (P56). The bookkeeping lives in a `Cues` field that always
  compares equal, so the existing state-equality tests and `look()` see nothing new.
  - Refused is a sideways press on an option row that didn't change (a slider at its end), or any
    press on a dimmed row. A press that means nothing where it is (right on the main menu) is
    silent, as the original was in the capture.
  - A held refused press sounds once: the same press refused again within 25 frames (the d-pad's
    first repeat comes at 20) stays quiet.
  - A ride running is silent, and so is the Start that pauses it.
- `main.rs`: the front-end loop loads the sounds when anything renders audio, and calls `Front::cue`
  with `true` each front-end frame and `false` each ride frame. The front-end loop covers the splash,
  main menu, settings and join screen.
- Tests:
  - `tests/menu.rs` `the_menus_play_their_music_click_on_every_move_and_hand_over_to_the_ride`:
    the loop alone on the main menu at the music's 70; Down is one Move and one more Menu FX voice;
    Right on the main menu is silent; Cross is Accept, Circle Back; the ride starting is Accept, the
    ride and its Start are silent, the pause clicks; front_end off fades to the song at 70 with the
    loop gone; back on, the loop again.
  - `a_slider_clicks_each_step_and_refuses_at_its_end_once` checks Side, Refused, and a held repeat
    staying quiet.
  - `tests/audio.rs` `the_menus_sounds_come_off_the_disc` checks the real files. It was proved to run,
    not skip, with a temporary panic.
- I took nothing from opentennis: it has no menu clicks or menu music to borrow.

## What the original does

The UI manager (the app's +0x3d0) holds the menu music at +0xd8 and two response emitters at
+0xdc/+0xe0, one per player. `./kit fn 0x241b20` is the init that 22 front-end screens share. It makes
the music with `0x165b90(bank, "UI\UI_001", 0xc1, 1, 0, 0)`, but only when the app's +0x3cc is 0
(no ride and no attract demo), and loads both emitters from `UI\ui.aud`. Screens start the music if it
isn't playing (`./kit fn 0x2cab50` and three others). Movies and the audio page's song preview stop it
and start it again.

`./kit fn 0x242410` plays sound N for a player and has 251 callers. The pause's audio page
(`./kit fn 0x299400`) shows what each press plays:
- up/down: 1;
- left/right: 2 when the slider moved, 5 when it did not, on a fresh press only;
- accept: 3;
- back: 4;
- the song row with fewer than two songs: 5.

The music is a loop: block 1 of each half carries the SPU's loop-start flag, so it goes back to
frame 28.

## How it was proved

- **RAM over PINE** (PCSX2 copy 2), using the scratch scripts menu_music.sh and to_ride.sh:
  - the music sound's voice (+0x34) is -1 until the main menu, then one live handle for more than
    30 s;
  - the attract demo has +0x3cc set and no music sound;
  - a training movie drops the voice, and the list after it has a new one.
- **Audio of the original:** Video Capture over AT-SPI at the main menu, presses sent through
  `./kit pad`, correlated against the disc's clips with numpy.
  - The capture's music is UI_001: normalised correlation 0.90–0.93 left and 0.80–0.88 right at
    2, 5, 10, 20 and 26 s, keeping steady time.
  - With the music subtracted, each press window's best click match:
    - the two Downs: up_down at 0.89 and 0.69;
    - Up: up_down at 0.39;
    - Cross: accept at 0.41;
    - Right on the main menu: no click;
    - Circle: unclear.
- **The disc's files:** `tests/audio.rs` `the_menus_sounds_come_off_the_disc` checks 32 kHz stereo,
  1,202,124 frames, five clicks, and up_down at 44.1 kHz with 280 frames.
- **The port's behaviour:** the two new tests in `tests/menu.rs`; `./kit check -- -p openmx --test
  menu` passes (138).

## Not verified / not 1:1

- **The 1 s cross-fade** between the menu loop and the ride's songs is the port's. The original stops
  one and starts the other, and its ride music is the game's own playlist, not P42's songs.
- **The ride's song holds while the front end is up** and resumes where it was. Whether the
  original restarts its ride music each ride wasn't checked.
- **The menu loop sits under the music slider.** Which slider (if any) scales the original's UI_001
  wasn't traced.
- **The clicks are under Menu FX, not Sound FX.** That's from P56's page, which lists Menu FX as a
  slider; the emitter's own volume path wasn't traced.
- **Clicks proved in the capture:** only up_down (Down, strongly) and accept (Cross, weakly). Up was
  weak at 0.39; Circle's sound is unclear, and the screenshot after it was truncated, so the screen it
  landed on is unknown. left_right, cancel and locked are placed by the decomp's pause page alone.
- **Cues come from comparing states, not from the original's per-screen calls.** Each of its 251 call
  sites wasn't matched one by one. Two examples:
  - a tab change (L1/R1) plays left_right here;
  - a press that opens or closes a screen plays accept or cancel, whatever the original's screen did.
- **Refused:** the original plays locked on a fresh press, but this port suppresses the same refused
  press for 25 frames. So a second deliberate tap inside about 0.4 s is quiet too.
- **Pausing a ride (Start) is silent here.** The original's pause-open sound wasn't checked.
- **Not used:** the 1st/2nd/3rd_game stings (Sound6–8), and the other `.vig` intros.
- **Both emitters play as one here.** The original has one per player; every seat's click plays the
  same, centred.
- **Music stops and restarts around movies and the song preview** don't apply: the port has no
  movie viewer and no menu song preview.
- **The shell itself wasn't heard running** (a real window with sound out); only the tests drive
  the mixer.

## Next

Nothing for this task. Possible follow-ups: the place stings (Sound6–8) on a result screen, and
the original's own ride-music handover if the port ever plays the game's playlist.
