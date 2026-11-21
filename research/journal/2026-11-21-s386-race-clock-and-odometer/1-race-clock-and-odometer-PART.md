# 1 — race clock and odometer (PART)

Task S386. 2026-11-21.

## What I did

- `Race::clock`: the score manager's clock, zero at `Race::new`, `dt` on it in `Race::tick` once the
  countdown is over, after the laps' tick and before the flashes'. It stands still once the tip's
  popup is up, as the whole tick does (S334).
- `Tracker::odometer` with `Tracker::ride(dt, speed)` (under zero counts as zero), zeroed by
  `Tracker::placed` (the grid). `race::odometer_speed(&Ride)` / `race::forward_speed(...)` measure the
  speed the odometer reads; `Laps::ride(player, dt, speed)` hands it to a card's tracker.
- The Sim calls it (`race_ride`) for every rider just before its frame, so the speed is the one the
  last frame left, as in the original's bike update (it runs before the physics).
- The shell: the stats commit's clock is the race's `clock` (the shell's own drop-to-leave clock is
  gone), and each seat's feet are its tracker's odometer, set at hand-over and again every tick
  until the ride is left (the original's commit reads the tracker as the ride is left). Split-screen
  guests' trackers get their odometer in `race_guests_in`.
- Snapshot wire and state hash carry both words (the hash only once they are not zero).

## What the original does

- Clock: `./kit fn 0x0025fcd0` (the manager tick). With flag 1 set and flag 8 clear, after the mode
  handler and two more calls, `+0xf8 += dt`; the children (the flashes) after. The countdown branch
  (flag 0x80) adds nothing; its last tick sets flag 1, so the clock starts the tick after the drop's.
  `./kit fn 0x0025fad0` (the reset) zeroes it.
- Odometer: bike `+0x133c` (the tracker's `+0x78`). `research/oracle.txt` S386 has every writer: the
  ctor and the grid reset zero it, and `./kit dis 0x27c8e0 0x27c9cc` (top of the player bike update,
  before the base update and so before the physics) adds `max(s, 0) * dt` while the bike has a card.
  `s` is the bike's speed (`+0x438`) with the front wheel touching; otherwise the chassis' velocity
  along the transform's forward row, flattened and made unit on VU0 (the up row when the forward row's
  y is <= -1 or >= 1), as `mula.s`/`madd.s` of x and z.

## How it was proved

- Clock: `the_score_managers_clock_runs_from_the_drop_to_the_tips_fade_as_s13d` (tests/race.rs) on
  `context/capture/s13d/sx01-finish.jsonl`: zero through the countdown and the drop's tick, then bit for
  bit on 2441 ticks (11 late or repeated rows, each one tick off), and standing on 96 rows once the
  popup is up.
- Odometer: `the_odometer_counts_forward_feet_and_starts_over_at_the_grid` (rule and grid reset) and
  `the_odometer_holds_s386s_live_sx01_race` on `context/capture/s386/sx01-odometer.jsonl`
  (research/s386/run.sh: research/c31/boot.py 35 4 1 1, then research/s386/odo.py; four AI-ridden
  bikes). 1240 odometer moves bit for bit from the row before's words (821 with the front wheel
  touching, 419 off it, so both ways of `forward_speed`). Bike 3: all 408 moves. Bike 2: 415 of 428.
- The tick into a row of this recorder uses that row's own `dt` (S13d's recorder: the row before's).
- The grid's zero (`./kit dis 0x27fae0 0x27fb20`) is in the same run of stores as the tracker's other
  counts, so `Tracker::placed`'s freestyle rule (S319) holds for it too.

## Not verified / not 1:1

- Bikes 0 and 1 miss on 43 of 188 and 105 of 377 moves. On most misses the odometer is a tick
  ahead of the other words (the speed of the row after, or two ticks' worth): the recorder reads
  those bikes while the game's tick is going over them. Not a formula difference (bikes 2 and 3 run
  the same code), but those rows prove nothing.
- The capture stopped at its run's 15-minute timeout: 715 rows, 14 s of racing, a cut last line
  (left out by the test). No row has a downed rider (`+0x748` is 0 throughout), so the odometer
  during a wreck is not proved. The port counts it every frame the bike has a card, wreck or not, as
  the original's update runs then too, but the port's ride words during a wreck may not be the
  original's.
- The up-row branch (forward row's y at or past ±1) never happened in the capture; it is read off
  the disassembly only.
- Split-screen guests (seats 1-3 of a local game): their trackers' odometer is fed in the shell's
  `race_guests_in`, after their frame, so it reads the speed the frame left: a tick late against
  the original's top-of-update read.
- The stats commit's feet are refreshed every tick while the gate is open (as the clock is). A seat
  with no race card keeps what the hand-over set.
- The score manager's flag 8 (the race stopped) is not ported: it never set in S13d. The clock
  goes on whenever the countdown is over and the tip's popup is not up.
- The state hash walks the clock and the odometer only once they are not zero, to keep the hashes
  of everything before a race's drop. The snapshot wire grows by both words (old snapshots don't
  read).
- `crates/game-sim/src/lib.rs` and `hash.rs`/`snapshot.rs` are not in the task's file list: the Sim's
  call before each rider's frame (`race_ride`) and the two new words' hash and wire lines need them.

## Next

- Nothing for S386. A capture with a downed rider (bike `+0x748` set) would close the wreck point.
