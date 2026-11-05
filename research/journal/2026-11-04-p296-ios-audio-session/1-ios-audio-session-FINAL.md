# 1 — ios audio session (FINAL)

Task P296. 2026-11-04.

## What I did

The user's bug: another app's music muted the game, and when that music stopped the game stayed silent.
The cause: the app set no AVAudioSession category, so iOS used SoloAmbient. Another app's playback
interrupted our session, and nothing made it active again or restarted the AudioQueue.

- **opentennis first.** `git -C ~/repos/opentennis fetch; git grep AVAudioSession origin/main` turns up
  only `play_on_silent` (e74bab83). It sets `AVAudioSessionCategoryPlayback` with no options, and has no
  interruption or secondary-audio handling. I took its category, so the game plays with the silent switch on.
  I added `MixWithOthers` and the rest myself; the brief asks for them, and opentennis does not have them.
- `crates/openmx-ios/src/session.rs` (new, built everywhere, like `hold.rs`): the launch category and
  options (`CATEGORY` = Playback, `OPTIONS` = `MIX_WITH_OTHERS` 0x1), and `Session`, which turns the
  session's notices into two values:
  - `other_audio`: the secondary-audio hint, begin or end;
  - `restarts`: a count that goes up when an interruption ends, when media services reset, and on
    DidBecomeActive after an interruption whose end never came or after a trip to the background. A
    resign-and-back with neither (Control Centre) does not restart.
- `sys.rs` (appended section): `start_audio_session()`. It calls `setCategory:withOptions:error:` with
  AVFAudio's own constant, reads back `category` and `categoryOptions` and says in the log line whether
  they hold, then calls `setActive:YES`. It observes the Interruption, SilenceSecondaryAudioHint and
  MediaServicesWereReset notices, DidEnterBackground and DidBecomeActive, on the main queue. The notice
  names and userInfo keys are AVFAudio's linked `extern` statics, not strings. A reset sets the category
  again. DidBecomeActive reads `secondaryAudioShouldBeSilencedHint` again, because the hint is not sent to
  an app that is not active. `activate_audio()`, `other_audio()` and `audio_restarts()` are callable from
  any thread.
- `ios.rs`: `main` calls `start_audio_session()` before the shell starts and logs the result.
- `audio.rs`: `Mixer::other_audio`. While it is set, `music_gain()` is 0 and the menu's loop is silent.
  The buses (effects, engine, crowd, menu clicks) are not touched. On iOS, each AudioQueue fill copies
  `openmx_ios::other_audio()` into the mixer. The output thread watches `audio_restarts()`. When the count
  moves, it drops the queue, calls `activate_audio()` and starts a new queue on the same mixer; a failed
  start is tried again 1 s later.

- **Not P296, but it was in the way:** the gate failed in `disc_worlds`'
  `world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it` ("built from cold: its start file is
  stale"). P299 committed `crates/stitch/worlds/world_mxu_free.start.json` under cache key
  `f06b0c54375601ea`, and MD95, merged alongside it, changed world_mxu_free's layout, so the key is now
  `8a2309e7bba75399`. Following the test's own instruction, I copied in the start the cold build kept. It
  is the same start (all 1.0, no after/dented); only the key line changed. Any other branch making the
  same copy merges cleanly.

- `./kit done` ran with `--skip-audit`: the audit fails on four leaks already on main, in files this
  task does not touch (`crates/game-sim/src/atv_wheel.rs:62,66`, `plan/DONE.md:519,533`). This task adds
  no addresses.

## What the original does

Nothing to port: this is the platform layer. The PS2 had no other apps.

## How it was proved

- `crates/openmx/tests/audio.rs` `another_apps_music_silences_only_ours`: a folder song plays (rms > 3000).
  Under the hint the mix is exactly 0, the sliders are unchanged, and the picked song is still the one
  picked. A looping effect started under the hint is heard at its full volume, `bus_volume(Effects)` = 1.
  Once the hint ends the song is heard again over the effect, and on its own after the effect stops. The
  menu's loop is also silent under the hint and comes back after it.
- `crates/openmx-ios/src/session.rs` tests: the launch category is Playback and the options are exactly
  MixWithOthers. The hint changes only `other_audio`. An interruption's end and a media-services reset
  each restart once. DidBecomeActive restarts only after an interruption whose end never came, or after
  the background.
- `tools/ios-build.sh build --release -p openmx --bin openmx` links. `llvm-nm -u` lists the six
  `_AVAudioSession…` symbols, and the binary loads `AVFAudio.framework`.

## Not verified / not 1:1

- **Not run on an iPhone.** There is no device here, so none of the behaviour has been seen on one:
  another app's music mixing with ours, our music going quiet, the silent switch, a call or Siri, coming
  back, or the read-back log line. The user's next iOS log should show `ios: audio session
  AVAudioSessionCategoryPlayback with options 0x1, active, …` and `audio: … (OtherAudio(true))` lines.
- **Whether the hint fires for a MixWithOthers session is from Apple's documentation, not tested.** The
  docs say `secondaryAudioShouldBeSilencedHint` is true while a non-mixable app plays, so it should fire
  for the Music app. If another app also mixes (a podcast app that mixes), no hint comes and our music
  keeps playing under it. That is iOS's design, not something this code chooses.
- **AVAudioSessionInterruptionNotification is deprecated in the iOS 27 SDK.** Apple's replacements are
  DidBecomeInactive and ResumptionRecommendation, which exist only on iOS 27 and later. The app's minimum
  is iOS 15, so it uses the old notice, which still links and is still posted. Not tested on iOS 27.
- **Music silenced by a hard cut, not a fade.** The hint arrives as a notice, and the port's cut lands at
  the next buffer (about 21 ms). The original had no such case to match.
- **A music-only silence keeps the song moving.** The song plays on silently under the hint, rather than
  holding its place, so it comes back further into the track. The brief says "silence", not "pause".
- **A restart is a new AudioQueue, not `AudioQueueStart` on the old one.** I chose that because a fresh
  queue always works after `setActive`. The cost is a gap of a few buffers on each restart.

## Next

Nothing in this task. The user's next iPhone log confirms it, or does not.
