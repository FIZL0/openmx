# 1 — race flags (FINAL)

Task P328. 2026-11-16.

## What I did

- `ride_hud.rs`: `RaceFlag` (White, Checkered), `RaceFlag::at(run, laps)`, `FLAG_UP` = 4 s.
  `RaceShown` gains `run`, the laps the view's card has run. `Tracker::note_race` watches it each frame:
  a count that goes up waves the flag it fits (white at laps − 1, checkered at laps) and takes the last
  one down when it fits none; no race resets it. `Tracker::race_flag(now)` says which flag is up and for
  how long. `flag()` draws it under the lap/place pill: a thin brown pole rocking ±0.35 rad from its foot
  on a 1.44 s swing, the cloth (70×52 layout px) trailing the swing and rippling, white with grey folds
  or 6×6 checkered. `Drawn::flag` keeps the flag and its box for tests.
- `main.rs` (two lines, outside the task's file list, needed to feed the HUD): `run` from the seat's
  lap card, and `note_race` called beside `note_event`.
- `tests/ride_hud.rs`: `the_last_lap_waves_white_and_the_finish_checkered` checks the timing (none at
  laps 0 and 1 of 3, white at 2, 4 s up, once per count, checkered at 3, reset on no race), the place
  (under the pill, near the middle) and the look (a white cloth with no black; black and white squares).
- Nothing from opentennis: it has no race HUD.

## What the original does

- The lap line's listener slots don't draw the flags. The manager's listener (mgr+0x30) is the
  supercross activity at +0x160 (vtable 0x00458690, the same object `race_drop` reaches through slot
  0x78). Its Lap slot (+8, thunk 0x00300500 → `./kit fn 0x2fc160`) formats the lap time and, for a card
  with flag 4, sets the per-player word activity+0x4d0 and queues an interface message (0x0037abf0, 7 or
  8). Its Finished slot (+0x14, thunk 0x003004f0 → `./kit fn 0x2fc410`) does the winner message (10),
  camera speeds and the hall-of-fame record checks. Neither touches a flag.
- The flags belong to the race overlay (RacingOverlays, vtable around 0x004557b0). Its bind slot
  (`./kit fn 0x2a6c60`) finds the children by name: +0xb8 `WhiteFlag`, +0xbc `CheckerFlag`. Its tick
  (`./kit fn 0x2a7da0`) polls its card (overlay+0x118 → +0x14): with card flag 4 and laps (+0x30) above the
  laps it last saw (+0x130), it sets +0x124 and a 10 s clock (+0x128 = 10.0). While that runs: below 6 s
  both flags are hidden (vt+0x54/+0x50 with 0). On the new count (+0x124), +0x130 = laps, and in event type
  2 (single race): laps == mgr+0x94 → CheckerFlag shown and its movie restarted (vt+0xbc); laps + 1 ==
  mgr+0x94 → WhiteFlag the same. So each flag stands 4 s from the lap that fits it.
- Layout (`sglracui.res`, `ui\racingoverlays.lyt`): both at x=97, y=79, centred, under the Lap Overlay
  at 14,24 (horizontal split 97,79 and 97,70; vertical 2 at 173,79 left-justified). The art is the Flash
  movies `ui\flag_white.fsh` / `ui\flag_checkered.fsh` (and `_2p` versions).

## How it was proved

- **Seen live in PCSX2** (copy 4): C31's `context/states/c31-sx01-race-start.p2s` (AllAI sx01, 3 laps),
  the three AI cards (0x8fc940, 0x8fcd00, 0x8fcb20 that session) given flag 4 and laps 2, then 3, over PINE;
  frames by System → Screenshot over AT-SPI. Laps 2: a white waving flag on a thin pole under LAP, up at
  0.5 s and 2 s, gone at 5.5 s. Laps 3: the same, checkered. Without flag 4 nothing shows (the AI cards
  lack it), which agrees with the tick's gate. Shots in `context/shots/p328/` and PCSX2's snaps folder.
- The port's flag was drawn to PPM by a throwaway test at 0.2–1.25 s and compared by eye.
- The timing and look are held by the new test.

## Not verified / not 1:1

- **The flag's picture is ours, by eye.** The original plays Flash movies (`flag_white.fsh`); the port
  has no player for those, so the cloth, pole, colours, swing (1.44 s, a guess off the movie header's
  first float, 1.44) and ripple are drawn off five PCSX2 frames, not off the movie.
- **The place is ours.** The original stands the flag top left at 97,79 under its lap readout; the
  port's lap/place pill is top centre (P327), so the flag stands under that. Split-screen views draw it
  in each view's own layout like the rest of the HUD, not at the split layouts' places.
- **Event type is not checked.** The original waves flags only in event type 2 (single race); the port's
  menu race has no practice mode, so every race waves them.
- **Card flag 4 is not modelled.** The original's overlay only reacts for a card with flag 4 (the
  human's, as far as seen); the port feeds the seat's own card (card 0), which is that card.
- **Not seen from a real lap.** The live check poked laps; no captured ride ends a lap with the human
  (C31 notes the human never laps on the track). The overlay polls the laps word, so a poke is what it
  sees either way.
- **The lap message and the Finished slot's work** (interface messages 7/8/10, hall-of-fame checks,
  camera speeds) are not ported here: not flags, outside this task.
- **No flag in a ghost run's or a time-limit race's own HUD** was looked at: only the supercross race.

## Next

Done.
