# 1 — s13 live gate (FINAL)

Task S291. 2026-11-16.

## What I did

- `research/s291/gate.py`: loads C31's start state (context/states/c31-sx01-race-start.p2s) into kit copy
  5. When the countdown reaches 4 s it pauses the VM over AT-SPI. It moves the chosen bikes 3 ft through
  the closed gate along its normal: every triple near the bike's position in the bike, its chassis and its
  frame object, as research/s134_tele.py does. It can relink the card list. It saves slot 9 and resumes.
  It records one row per race manager tick on one PINE connection. `research/s291/show.py` prints a capture
  around the move.
- Two captures in context/capture/s291:
  - `human.csv`: the human alone.
  - `four.csv`: all four riders, with the list relinked to 2/0/3/1.
  - Each has 196 rows and 60 ticks after the move. Each was recorded twice, with identical rows.
- The port, from what the captures show:
  - `Sim::race_frame` now runs after the riders' frames on all three paths (`step`, `step_all`, the
    seat step in rider_contact.rs). It used to run before them, at `event_frame`'s place.
  - A caught rider's penalty is set already run down one tick (`penalty - dt`), as the score manager's
    tick inside the race's does.
  - `Race::tick`'s watch walks the cards in the laps' list order. Without laps it walks player order,
    and a rider with no card yet goes last.
- Tests in crates/game-sim/tests/race.rs: proof 51, `the_gate_catches_match_a_live_sx01_start`, and
  `on_the_sim_the_race_sees_the_riders_after_their_frames`.
- S333 (one generator for a race) noted with the draw counts.
- Nothing taken from opentennis: it has no races.

## What the original does

Seen live, in the captures:

- **The race manager's tick comes after every bike's update in the frame.**
  - A row read with a frame_counter trigger caught one frame half done. The bike had already moved, but
    the card and the countdown had not.
  - On every row read at the end of the race tick, each card's `now` equals its bike frame's position
    (+0x30) bit for bit, for all four riders, AI riders too.
  - On the catch tick, the chassis velocity (+0x260) at the tick's end is exactly the push (xyz the
    scaled facing, w the facing's 0). So nothing after the race tick touches it, and the bike's physics
    first integrates it on the next tick.
- **The penalty**: on the catch tick (+0x1c0) it is `1 + unit − dt`, then `− dt` each tick after.
- **Card order**: with four cards caught in one tick, the draws go push then penalty per card, in the
  list's order (2, 0, 3, 1), not by id. Before the drop nothing re-sorts the list: the relinked order held
  for all 60 ticks.
- **Draws**: the catch's draws are the last of the frame. 6 other draws come before them (24 with four
  moved riders). On most ticks 0 draws happen, with a cycle of 11 every 6 ticks.
- **A held bike creeps**: its x and z move by 2 ulps a tick even with linear freedoms 0,1,0 (S290's area).
- **Timing**: frame_counter (app+0x374) does not mark the end of the simulation's tick, so reading on it
  tears rows. Polling the score manager's elapsed word (+0xc4) and reading when it changes gives one clean
  row per tick until the drop.

## How it was proved

All in crates/game-sim/tests/race.rs. `./kit check -- -p game-sim --test race --test camera_flash` gives
80 passed.

- **51, `the_gate_catches_match_a_live_sx01_start`**: both captures replayed through `Race::tick` from the
  paused frame after the move. Each tick gets the captured bike positions and looks as its feeds, and the
  laps' list in the captured order. Bit for bit, on every one of the 60 ticks:
  - every card's now and last;
  - every card's penalty;
  - the gate's flags and the countdown's elapsed time;
  - on the catch tick, each caught card's push against its chassis velocity;
  - the generator, after the frame's other draws (counted from the rows; the catches' draws are the last).
  - The catches are asserted to be exactly [0] and [2, 0, 3, 1], on the first tick.
- **Mutants**:
  - Watching in player order fails proof 51: four row 136, card 0's push.
  - The Sim test checks every tick that the card saw this tick's place, so moving `race_frame` back in
    front of the riders fails it.
- **`on_the_sim_the_race_sees_the_riders_after_their_frames`**: a stand-in ride into a closed gate. Each
  tick, the card's now equals the bike's transform after the tick. On the catch tick:
  - the chassis velocity equals the push drawn from the race's generator, bit for bit;
  - the penalty equals `1 + unit − dt`.
- The S13b tests (rolling through, the drop, the image's tick) still pass.

## Not verified / not 1:1

- **The Sim is not replayed on the capture.** Proof 51 runs `Race::tick` on the captured bike positions,
  not a whole `Sim` from the captured state. There is no seeded Sim of a 4-rider sx01 start with AI
  riders (S290/S12). The Sim's order is proved on the stand-in ride only.
- **One generator.** The port's `Race::rng` is its own stream. The test puts the frame's other draws
  (6 or 24) into it from the capture's counts instead of producing them. Noted on S333.
- **Where the penalty runs down.** The original runs every card's penalty down inside the race tick's
  score manager tick. The port does it in `Score::grow` in each rider's frame, before the race tick, and
  sets a fresh catch's penalty already run down once. The values match at every point a rider reads
  them and at every tick's end. The order inside a tick differs, and nothing in between reads it. The
  manager's flag 8 (skip the run-down) is not ported (as in S13b).
- **Where the race tick falls against everything else.** The captures show only "after every bike". Where
  it falls against props, traffic or cars is not measured. The port puts it straight after the riders,
  before the props' step.
- **The list order after the drop** (the place re-sort) is not part of this capture. These rows end
  before the drop (elapsed 6.03 of 9.08).
- **The teleport is a RAM poke**: every float triple within 0.5 ft of the bike's position in three
  regions was moved. If some copy of the position was missed, it did not show: every row's card equals
  the frame object's position.
- **The held bike's 2-ulp x/z creep** under freedoms 0,1,0 is not looked into (S290's hold).

## Next

Land.
