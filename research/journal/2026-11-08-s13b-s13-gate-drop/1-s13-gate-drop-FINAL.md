# 1 — s13 gate drop (FINAL)

Task S13b. 2026-11-08.

## What I did

- `race::Race` (crates/game-sim/src/race.rs): the gate, one `Seen` per rider (player order), the music
  clock (+0x4c0) with its hold, the tip clock (+0x410) with the lesson stage (+0x470/+0x474), the
  score manager's countdown (length +0xbc, elapsed +0xc4, the 0x80 flag), the scene word the drop
  draws, and the race's own `Rng`. `Race::tick` runs the race manager's tick in its order;
  `Race::drop` is the drop. `race::push` (stop the chassis, then velocity = push) and
  `race::release` (every chassis axis free again) are the two bike messages.
- `chassis::stop` / `chassis::reset_history`: the chassis' own stop virtual and the history reset it
  tail-calls.
- `Sim::race: Option<Box<Race>>`, with `race_frame` straight after `event_frame` on all three paths
  (`step`, `step_all`, the bumped step in rider_contact.rs): the tick, then each catch pushed and its
  penalty put on the rider's card, then `release` for every rider on the drop tick. Clone, snapshot
  (tag `S13brace`, also in the `section` list) and the hash walk carry it; `None` adds nothing to
  any of them, so every existing state's bytes and hash stay the same.
- research/oracle.txt: the clocks under race_manager_tick, and race_drop, race_reset,
  race_countdown_listener, score_manager_countdown_end, bike_race_start, bike_message, chassis_stop,
  chassis_reset_history, and the presentation callees the clocks oracle hooks.
- Follow-ups S290 (the start hold) and S291 (a live capture).

## What the original does

The race manager's tick (`./kit fn 0x002fcff0`), in order:
1. The music clock. It runs only while above 0 (`c.ole` with 0 false), while the track's state is
   not 2 and while the listener's +0x5c bit 2 is clear. It does `c -= dt` and stores the result. At 0
   or below it starts the music (only if app+0x570 is set) and stores 0.
2. The start-gate watch (S13a), only while the gate pointer is set and its flag 1 is up.
3. The tip clock runs the same way, and running out shows the "PreloadAndClutchTipPopup".
   Otherwise, if a lesson stage is named and the clock is not armed yet, it arms the clock once at
   `max(0.001, (len − 0.75) − 1.3)`. This is skipped when the stage length compares equal to 0.
4. `score_manager_tick(dt, race+0x50)` (`./kit fn 0x0025fcd0`):
   - It first runs the card penalty tick (`p > 0 → p -= dt`), unless manager flag 8 is set.
   - Then, while the countdown flag 0x80 is set and flag 1 is clear, it fires the countdown listener
     every interval (presentation only).
   - When `!(elapsed < length)` it ends the countdown, in this order:
     - flags `&= ~0x80`, `|= 1`;
     - the cards' "racing" bit;
     - the listener's vt+0x78, which is the drop (`./kit fn 0x002fbdd0`);
     - each card's bike's vt+0x68, the start message (`./kit fn 0x0027f5e0`).
   - Then `elapsed += dt`, in both cases.

The drop, after its overlays: it sets +0x4c0 = 1.0, clears the gate's flag 1, and makes one rand,
`scene+0xf0 = 8 + 5·unit`.

The start message for a human bike sets the chassis' six freedom words to 1 and bike+0x610 = 1. A
computer rider's bike also draws three or four times.

The bike's message 2 (`./kit fn 0x0027da00`): chassis vt+0xc4 (stop), then `lq`/`sq` of the push
into the velocity, keeping the facing's `w`.

Stop (`./kit fn 0x0015c3b0`):
- Force, torque, both accelerations, the velocity in both frames and the angular velocity in the
  body's frame go to (0,0,0,1). The speed goes to 0.
- Then vt+0xb0 (`0x0015c420`) zeroes the position, velocity and angular-velocity histories. The
  orientation history's two quads go to (0,0,0,1). That loop's second pass overlaps the first by one
  word, and the result is still dx = prev = (0,0,0,1).
- Impulse, correction and the world angular velocity are left alone.

In the s196-sx01 image the human bike waits at the gate with chassis +0x2a0..+0x2a8 = 0, 1, 0, which
means it can only move in height. The original therefore never lets a rider roll through its gate
on its own. The watch is a backstop, and on that image the only way through is a teleport.

## How it was proved

All in crates/game-sim/tests/race.rs (8 tests, all run, none skipped):
- `the_race_clocks_match_the_race_managers_tick`: the real race manager's tick against
  `Race::tick` over 20,000 generated cases.
  - Inputs: clocks drawn from 0, −0, negatives, denormals, values under one tick and larger; the
    track state, including 2; the pause bit; app+0x570 on and off; the lesson stage index and its
    length (including 0, −0 and denormals); the armed flag; random `dt`.
  - Hooked: the score tick and the presentation callees.
  - Compared bit for bit: both clocks, the armed word, whether the music start and the tip lookup
    were called, and the generator.
- `the_push_matches_the_bikes_message`: the real bike message 2 through the real chassis vtable,
  over 2,000 chassis filled with stray words. Every word of chassis +0..+0x330 is compared with
  `race::push`.
- `the_drop_and_an_early_rider_match_a_captured_sx01_start`: the real, unhooked race manager's tick
  on context/states/s196-sx01.ee.bin.
  - Set-up: the countdown's elapsed is set to its length, and the bike's transform is moved five
    feet through the closed gate along the gate's normal.
  - Compared against `Race::tick` + `push` + `release` + `Score::grow`: the gate flags, the card's
    now and last, the penalty (set and then run down one tick in the same call), both clocks, the
    scene word, the countdown's elapsed and flags, every chassis word from +0 to +0x330 (including
    the freed axes), and rand_state. All match, so nothing else in that tick draws.
- On the Sim:
  - A ridden bike with gas held into a closed gate 3 ft ahead is caught once, gets a 1–2 s penalty,
    goes backwards and makes no more draws for 20 ticks.
  - A 1 s countdown ends on tick 30–32, opens the gate, frees the axes, and the music starts a
    second later.
  - A snapshot round trip keeps the hash through 40 more ticks.

## Not verified / not 1:1

- **No live PCSX2 capture of an early rider.** The brief's "captured sx01 start" is the S196 sx01
  save-state RAM image, run through the real tick in the interpreter with the bike teleported through
  the gate. A rider cannot roll through in the original: the bike is pinned to height-only motion
  until the drop. There is no uinput here (no pad). Filed as S291.
- **Where the race tick falls among the riders.** The port runs `race_frame` before the riders, at
  `event_frame`'s place. The card penalty runs down in `Score::grow` after each rider's frame, not
  inside the race tick. Over a tick the count matches, but a rider's frame on the catch tick sees
  the penalty already set; the ordering against the original's scene walk is not measured. S291.
- **Card order with several riders.** The port watches in player order. The original walks its
  card list (+0x84 / +0x1c8), and its order with more than one rider is not captured (S291).
- **The start hold is not ported.** Who pins the bike's chassis to (0,1,0) before the drop is not
  traced, and the port's riders are free behind the gate. `release` frees them as the original's
  start message does. Filed as S290.
- **Computer riders' start draws.** A computer rider's start message draws three or four times. The
  port's races have no computer riders yet (S12, S290).
- **bike+0x610 and the cards' "racing" bit** (set at the countdown's end) are not ported; no
  reader of either is traced. S13c/S13d will meet them.
- **Presentation only.** The music start and the tip are returned as `Frame` flags and nothing in
  the shell reads them yet (P313). The countdown listener's beeps and gate-bar physics flags, the
  drop's overlays, and the music track's load state (`music_held`) are the front end's.
- **The countdown's length and reset** (race_reset: a table entry + 1.5, chosen by rand) are S13a's.
  `Race::new` takes the length it is given, and nothing creates a Race on a real race load yet
  (S13a/S13e/P313).
- **The scene word's reader** (scene+0xf0, 8–13 s) is not traced. It is drawn and kept so the
  generator stays in step.
- **Interval / accumulator** of the countdown (+0xc0/+0xc8) are not modelled. They only pace the
  listener's presentation, and the listener draws nothing.

## Next

Land. Then S290 (the start hold) and S291 (the live capture).
