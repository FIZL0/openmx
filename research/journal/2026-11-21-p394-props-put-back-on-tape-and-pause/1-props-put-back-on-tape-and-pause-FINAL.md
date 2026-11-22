# 1 — props put back on tape and pause (FINAL)

Task P394. 2026-11-21.

## What I did

- Tape (`crates/game-sim/src/replay.rs`): a `props_put_back FRAME on|off` line, built like
  `backwards_bail`'s: `PadTape::props_put_back` / `props_put_back_at`, parsed and written, `Recorder::props_put_back`,
  `Head::props_put_back`, and `play_from` puts it on `sim.traffic.props.put_back_with_rider`.
- Shell (`crates/openmx/src/main.rs`):
  - a ride recorded with the switch on (`--props-put-back` or the Game Modifiers row, offline) writes
    `props_put_back 0 on`;
  - a played tape starts with the switch off on every seat (the original's), then takes the tape's lines on
    every seat's prop set, written on again to a tape recorded during the playback;
  - the pause closing on a changed row puts the switch on every seat's prop set (split screen too), and on the
    recorded tape; held by a played tape (the same hold as Air Wrecks), and never online;
  - a local player joining mid-ride takes the first seat's switch, so a pause change reaches them too.
- Menu (`crates/openmx/src/menu_pages.rs`, not on the task's file list but its note was now wrong): under the
  pause the row says "Changes take when the pause closes." and is held under a played tape, like Backwards
  Landings Bail's.
- Nothing taken from ../opentennis: it has no prop reset switch.

## What the original does

Nothing: Props Put Back is the port's own (S475). Off, the default and Original mode, nothing changes.

## How it was proved

- `game-sim --test prop_set` `the_props_put_back_switch_rides_on_a_tape`: written only where set, read back,
  bad lines refused, the head hands it out before its frame, `play_from` takes a frame-0 and a mid-ride switch.
- `openmx --test menu` `the_props_put_back_row_takes_when_the_pause_closes`: note under the pause, held by a tape.
- By hand, headless on free09: `--props-put-back --record` wrote `props_put_back 0 on`; `--play` of that tape
  printed `props put back: on (the tape's, before frame 0)` and re-recorded the same line.
- That the switch on puts a knocked prop back on the next poof is S475's test
  (`a_poof_puts_knocked_props_back_with_the_switch_on`); the pause sets that same field.

## Not verified / not 1:1

- The pause path was not ridden end to end in the shell (a ride opened from the front end, paused, row
  flipped, unpaused, then a poof by pad): no headless way to drive the pause menu in one shell run here. The
  code sets the same field the tape and start paths set, and those are covered.
- A tape's replay with knocked props was not compared hash for hash against its ride: the 60-frame ride
  knocked none ("props moved: 0 of 62").

## Next

none
