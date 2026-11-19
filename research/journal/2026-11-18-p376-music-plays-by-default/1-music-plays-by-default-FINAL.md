# 1 — music plays by default (FINAL)

Task P376. 2026-11-18.

## What I did

- Why the music started held: at launch and on every return to the app, `crates/openmx-ios/src/sys.rs`
  reads `secondaryAudioShouldBeSilencedHint` (after `setActive:`, and before `Active`), and the
  session took that read's `true` as "another app is playing". Both phone logs
  (`~/Downloads/phone-160542-openmx-log.txt`, `research/p340/openmx-log-iphone-2026-11-13.txt`)
  show `other audio true` at launch and every foreground with nothing else playing. The read is
  not trustworthy for our Playback + MixWithOthers session. A likely cause is that a paused
  Spotify/Music app keeps its non-mixing session active, so the hint stays true while nothing is
  heard. This is a guess, not proved (see below).
- Fix (`crates/openmx-ios/src/session.rs`): a hint *read* can only let the music go (`false`).
  Only the hint *notice* (`AVAudioSessionSilenceSecondaryAudioHintNotification`, Begin/End), which
  iOS sends when another app's sound really starts or stops, holds the music or lets it go. The
  launch default is playing. The session's `took_over` flag is gone: it only gated reads.
- Log: every read now also logs `isOtherAudioPlaying` beside the hint
  (`audio: read hint X, other audio playing Y`). The next phone log will show whether either read
  was true with nothing playing.
- Mixer (`crates/openmx/src/audio.rs`) is unchanged. Android, desktop and web never call
  `hear_other_audio`, so they never hold. New test `music_plays_by_default_without_other_audio`
  in `crates/openmx/tests/audio.rs` pins the default. New session test
  `music_plays_by_default_and_a_read_alone_never_holds_it`.
- opentennis has no secondary-audio handling to take.

## What the original does

Not the original's: iOS audio session handling is ours.

## How it was proved

- Unit tests: session (openmx-ios, 19 pass) and mixer (openmx tests/audio, 48 pass).
- `cargo check -p openmx-ios --target aarch64-apple-ios` builds.

## Not verified / not 1:1

- Not tried on the phone: no iPhone here. Three checks remain: launch with nothing playing shows
  `other audio false`; Spotify starting holds the music and shows it paused; Spotify stopping
  brings it back.
- Why the hint reads true is not proved. The new log line is there to show it.
- Another app that starts while ours is in the background sends no notice. Coming back, ours then
  plays mixed with it, as before P339, until that app stops and starts again. The player can
  pause ours. Trusting a `true` read there is what caused this bug.

## Next

On the next phone log, read the `audio: read hint … other audio playing …` lines. If
`isOtherAudioPlaying` is right where the hint is wrong, a read could hold again through it.
