# 1 — outro armed in play (FINAL)

Task S480. 2026-11-21.

## What I did

- `race::Race::tick` hears the laps' `FinishNotice::Over` and runs a new private `Race::race_end`: the cards
  in list order (place, and for each human card its seat, preloads and clutches) go to
  `Supercross::race_end` (single race, activity 2). Its tips set `Race::tip_words` (new `race::tip_words`,
  the popup's eight word slots). Its cue picks the scene: with a length, `Race::stage` takes it and the
  tip arms on the next tick (the S334 code already there); with none, the activity's `cue_ended` and the
  race's tip clock at 7 s at once.
- `Supercross::outros` (on the wire): the eight outro scenes' lengths, `None` for one with nothing to play.
  `Supercross::outro_length(cue)` reads it.
- `race_start::outro_lengths(event.aud, streams.txt, streams.juk)` reads them off the disc; main.rs reads
  `aud\event.aud` from the event's res (as for the start calls) and the stream list from `appstart.res`,
  prints `race: outro scenes …`, and sets them on the start's activity.
- `race_start::Watch::wait(dt, outro_up)`: on a race with an activity the outro's wait runs out on the
  first tick `Race::outro_up` is true, or 20 s after the Over (`OUTRO_BACKSTOP`) if the popup never
  comes (no laps, no scene); without one, the old 7 s clock.
- A seat is a human card's rank among the human cards by player number (seat 0 is player 0, as
  `race_player` numbers them).

## What the original does

- The race end (`./kit fn 0x002fc970`) picks the stage 2/4/6 by the best player place and cues the scene
  `+0x44c[stage]` through its vt+0x1c; a cue that returns 0 arms at once with 7.0.
- The cues are sounds built by `./kit fn 0x002fdc40` from the event's audio ini, `[RaceEndDitties]`
  `Sound1`..`Sound8`, into `+0x450 + 4*(k-1)`, so stage s is `Sound<s>`; made by `./kit fn 0x00165b90`
  with flags 0x42 (0xc2 for Sound8), the stream path `./kit fn 0x00165ef0`, whose length `+0x24` is
  the stream's Duration (`./kit fn 0x0019d8c0`).
- The stream index (`./kit fn 0x0019a4c0`): `aud\streams.juk` `[Song N] Duration`, names from
  `aud\streams.txt` line N-1. A name is found (`./kit fn 0x0019a270`) as `Aud\` + name with the extension
  made `.vag`, then `.vig`, compared case-blind. Not found: index −1, length −106.0.
- The race manager (`./kit fn 0x002fcff0`) arms its tip clock `+0x410` at `max(0.001, len − 0.75 − 1.3)`
  once the stage's length is nonzero.
- On the disc every race event's `event.aud` has the same `[RaceEndDitties]`: `UI\1st_game`,
  `UI\1st_race`, `UI\2nd_game`, `UI\2nd_race`, `UI\3rd_game`, `UI\3rd_race`, `UI\no_place`,
  `UI\postloop`. `UI\3rd_race` is 8.514920 s.

## How it was proved

- S13d's sx01 race (`context/capture/s13d/sx01-finish.jsonl`) ends third with stage 6 and arms `+0x410` at
  6.46 one tick after the Over: 8.514920 − 2.05 = 6.46492. The new test
  `the_races_over_cues_the_outro_and_arms_the_tip_from_its_length` (tests/race.rs) drives a Race to its
  Over with those lengths and checks the stage, the tip clock bit for bit against the formula, and the
  7 s case for a scene with nothing and for no lengths at all.
- `race_start` tests: `the_outro_lengths_are_the_ditties_streams_durations` (inline ini text: case,
  `.vag` before `.vig`, missing names, missing Duration, missing section) and
  `the_outro_wait_runs_out_when_the_races_outro_is_up`.
- `./kit check --all`: green but for game-render's `whip_tricks` 360 tests, which this task does not
  touch (they read the slot's git-ignored tricks pack, and main has since moved the 360 in MD248).

## Not verified / not 1:1

- Not run in a live ride on screen, and no capture of the whole path (Over → stage → tip → popup → leave)
  in the port beside the original; only the tip value is matched to S13d.
- The Duration text is parsed by Rust's f32 parse; the original's parse (atof-like) was not compared. Bit
  for bit for 8.514920 only through the test's formula.
- When the scene's play (vt+0x1c) fails was not traced (e.g. sound off, stream missing on the disc): the
  port takes a scene not on the stream list as "nothing to play" (7 s), while the original's length there
  is −106.0 and whether its play then fails is assumed.
- The popup's words are taken as clear before each race; in the original they live on the popup and are
  only ever set by the race end, so a second race in one sitting could carry the first's.
- `alt_outro` is always false in play (single race, activity 2); the 1/3/5 scenes are never picked.
- Seat numbering by rank among human cards is assumed from `race_player`; not checked with two humans.
- A stream with no (or a zero) `Duration` is taken as nothing to play (7 s); the original's stage would
  be 0, which never arms the tip. The shell's 20 s backstop has no original counterpart.
- A disc without `[RaceEndDitties]` would leave the original with no cue (likely a crash); the port gives
  7 s.
- The shell's leave is the first tick the popup is up; the results hand-over after it is unchanged.

## Next

Nothing.
