# 1 — music held for other audio (FINAL)

Task P339. 2026-11-13.

## What I did

**Looking for the change since v0.12.0.** P317's Songs From was the suspect. It is not the cause.
I ran the v0.13.0-era desktop shell (this branch, release build) windowed on a private headless
weston. Its mix was piped to a file through a stand-in `pw-cat`, with an empty `ALSA_CONFIG_PATH`.
The iso was symlinked into a scratch folder, `mods/` was linked to the main checkout's (both
packs), and cwd was the worktree, so settings came from the scratch folder. Each run lasted 40 s;
RMS per second:

| settings | menu | ride (free06) |
|---|---|---|
| none (fresh) | ~1500 (UI_001 loop) | ~3000-4500, `music: playing disc:itsover` |
| v0.12.0's defaults, `mods_off = untamed_music` | ~1500 | ~3800, `playing mod:mxvatv_music/mudmen_animal` |
| the same with `music = off` (control) | — | ~175 (engine alone) |

So the desktop plays music in the menu and in a ride with both kinds of settings. The user's
iPhone log (research/p340/openmx-log-iphone-2026-11-13.txt) also picks songs
(`music: songs from MX Unleashed (13), MX vs. ATV Untamed (22)`, `music: playing disc:downfall`).
But every line in that log says `other audio true`. Since P296 the mixer gives the songs and the
menu loop a gain of 0 under iOS's `secondaryAudioShouldBeSilencedHint`. The menu kept saying
"NOW PLAYING" through that silence, and nothing let the player get the music back. That explains
"no music in the menu or in game" with the effects still heard. P296 was already in v0.12.0, and
openmx-ios has had no audio change since (only P320). So 0.12.0 would have been just as silent
whenever the hint was on.

**The fix (the user's ruling: show it paused, unpausing takes over):**

- `audio::Mixer::hear_other_audio(on)`, called by the iOS output on each buffer in place of the
  plain `other_audio =` write. When the hint comes on, it holds the music (`Music::hold(true)`), so
  the pause player's eyebrow reads PAUSED and Settings → Audio offers Play Music. It remembers
  that it made the hold itself.
- If the music is let go while the hint is still on (Play Music, the player's hold disc, Skip,
  Previous, picking a song), the mixer takes over. `music_silenced()` turns false, the songs and
  the menu loop play at their sliders, and `take_over_asked()` tells the platform once.
- When the hint ends, the mixer lets go of a hold it made itself (the music comes back as it did
  before). A pause the player made stays. The next hint holds the music again.
- iOS: the output's thread sees `take_over_asked()` and calls the new
  `openmx_ios::take_over_audio()`. That sets Playback with no options (not mixing), makes the
  session active, which interrupts the other app and stops its music, and then sets
  MixWithOthers again. `session::Event::TookOver` clears the hint and bumps `restarts`, so the
  AudioQueue starts again in case the category change stopped it. Hint reads (launch,
  app back, `activate_audio`) are now their own `Event::HintRead`. After a take-over, a stale
  `true` from a read is passed over. Only the hint's own notice (another app began) holds the
  music again. Without that, the restart's own read could pause the music again straight after
  the take-over.
- The front end now copies `sound.held()` into `menu.music_held` every frame, as the ride already
  did. Settings → Audio's row therefore shows Play Music for a hold the sound made.

**Decided (unattended, nobody to ask):** the brief says "(the music control, or Resume)". I read
Resume as the music's own play control (the hold disc / Play Music), not the pause menu's Resume
Ride. If Resume Ride took over, any pause in a ride would stop the player's podcast. That is the
opposite of keeping their music, which the user OK'd. If the user meant the ride's Resume, it is
one line where the pause's Resume is handled: `sound.hold(false)` when the hold was the hint's.

opentennis: nothing taken. Its `play_on_silent` category is where P296's came from; it has no
other-audio handling.

## How it was proved

- `crates/openmx/tests/audio.rs` `another_apps_music_pauses_ours_and_playing_takes_it_back`:
  the hint from launch holds the song (silent, held, nothing asked). Play under the hint asks for
  one take-over and the song is heard (rms > 3000). The hint ending and coming again pauses it
  again, and its end lets go. A player's own pause survives the hint's end. The menu loop is
  silent while held and heard after a take-over. P296's old test still passes as it was.
- `crates/openmx/tests/music_packs.rs` `the_music_plays_with_fresh_and_v0_12_settings`:
  `Settings::default()`, v0.12.0's default `settings.txt` text, and that text with every pack in
  `mods_off` (0.12's "MX Unleashed Only") are each applied to a `Speakers` as `start_sound` does.
  In each case a song plays, is not held, and is heard at its slider.
- `crates/openmx-ios/src/session.rs` `taking_the_music_back_ends_the_hint_and_restarts_the_output`.
- The desktop listen runs above (fresh and v0.12.0 settings, menu and ride).
- `tools/ios-build.sh check -p openmx-ios -p openmx` builds for aarch64-apple-ios.

## Not verified / not 1:1

- **Not run on an iPhone.** These are untested on a device: that `setActive` with no MixWithOthers
  really stops the other app's music, that setting MixWithOthers again straight after leaves it
  stopped, and that the AudioQueue restart after `TookOver` is clean. Also untested: whether
  `secondaryAudioShouldBeSilencedHint` reads `false` afterwards. If it stays `true`, the
  take-over still plays our music, because a read is passed over after a take-over. No device was
  available to this run.
- **Why the hint was `true` on the user's phone is unknown.** It may be a real player app, or one
  holding a non-mixable session while paused. Either way the music now shows paused and plays on a
  press, which is what the user asked for.
- **Not shown that v0.12.0 played music on that phone.** No 0.12.0 log from it. By the code, 0.12.0
  had the same silence under the hint.
- The pause menu's Resume Ride does not take the music back (decision above).
- The desktop listen runs used this branch's build, not the published v0.13.0 binary. The music
  path (audio.rs, music_packs.rs, the settings) is the same as v0.13.0's apart from this change,
  and this change does nothing off iOS.
- The menu loop under a hold the *player* made still plays in the front end, as before this task.
  Only a hold made for the hint silences it, through the hint itself.

## Next

On the iPhone, with another app's music playing: Settings → Audio should say Play Music, and pressing it
should stop the other app's music and start ours.
