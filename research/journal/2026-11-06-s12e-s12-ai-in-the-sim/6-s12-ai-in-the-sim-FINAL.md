# 6 — s12 ai in the sim (FINAL)

Task S12e. 2026-11-13.

## What I did

- S318, S319/S322/S323 and C33 have all landed, so this part does the replay the task asks for, in
  `crates/game-sim/tests/ai/race4.rs` (`s12e_free09s_four_racers_ride_as_the_capture`). The test:
  - loads C33's `free09-ai4.ee.bin`;
  - seeds the four riders in actor-list order (ai0, ai1, the human, ai2). Each is built as the shell
    builds a rider (`Bike::new`, free09's BikeCol, `Race250A.phy` for the AI and `Race250P.phy` for
    the human, since class 2 is 250cc). Each is then seeded off RAM with S174's `Image` seeder;
  - reads the three brains into `ai::Pilot`s as S323's lane test reads recording2's;
  - gives `Sim::ai` an `ai::Lane` holding free09's seek world and a generator seeded from the image's
    rand_state;
  - steps the four with `Sim::step_all`, which goes from the last player to the first, so the actor
    head is the last player.
  - holds every agreed C33 row's generator, and each rider's centre, quat, velocity and spin, against
    the port's.
- **Alignment.** The image's RAM is row 3 (frame_counter 2, saved before the increment). The test
  asserts row 0 equals the image, and step k gives row 3+k. The step into row N runs at row N−1's dt.
  The velocity difference settled that: with the row's own dt it parts on the first step.
- **Torn rows.** Some rows two recordings agree on are still torn. A lone mismatch followed by a
  matching agreed row is taken as torn and skipped, and two mismatching agreed rows in a row part.
- **The lever lag (`Sim::think`, tick.rs).** The bike's frame now rides the levers as they stood
  before this tick's think. The hand-off lands after the bike has read them, so each AI rider launches
  a tick after its gate opens. Without the lag the marks are ai0 0, ai1 17, ai2 13. With it they are
  ai0 14, ai1 18, ai2 16. The lag is kept on that evidence.
- **The ratchet** `C33_AGREES_THROUGH = [14, 18, 0, 16, 6]` (ai0, ai1, the human, ai2, the generator).
  A regression fails the test, and a raised mark prints "raise C33_AGREES_THROUGH".
- `tests/ai/lane.rs`: `s323_free09` (the free09 world off the disc) is made `pub(crate)` for the new
  file. Nothing else there changed.
- Follow-ups filed: S328 (the non-AI draws), S329 (the human), S330 (the launch partings), S331 (the
  lane at a real map load). S326 (moving objects in the AI's walk) and S327 (the lane in the snapshot)
  already existed.

## What the original does

- Each AI rider's brain hands its levers to the rider (`./kit fn 0x00283a10`, the hand-off at rider
  +0x628/+0x62c/+0x63c/+0x640/+0x648). The replay says the bike tick that frame has already read the
  previous frame's levers. This was not traced through the actor walk's order in the interpreter.
  The evidence is the four marks above.
- The shared generator draws more than the AI in this frame window: 3 draws at tick 7 (the three AI
  riders' first landing off the line), 1 at tick 8, 4 at tick 15 (ai0 and ai2 launch), 6 at tick 17,
  and 10–29 a tick from tick 18. None of the ported AI draw sites fire at a first landing: seeker
  pick/roll, plan_trick's roll, bump, swerve, the 1 s brilliant draw, lag, cross-up and the gate. The
  draws look like presentation (dirt or landing effects), as S321's camera flashes were.

## How it was proved

- The C33 save-state capture is replayed through the Sim: `free09-ai4-agreed.csv`, compared bit for
  bit on every agreed row from tick 3 up to each lane's mark. ai0 is compared through 14, ai1 through
  18, ai2 through 16 and the generator through 6. The human parts at 4.
- `./kit check -- -p game-sim --test ai -- s12e_free09s_four` passes.

## Not verified / not 1:1

- **The lever lag is inferred, not traced.** It raises all three AI marks, but nobody has followed
  the original's frame to show where the bike reads the levers relative to the hand-off.
- **The generator parts at tick 7.** The original's non-AI draws (above) are not made by the Sim, so
  from tick 7 on the AI lane's generator is short of them. Every later AI draw is then compared on a
  different state. Filed as S328.
- **The human parts at tick 4** on its pitch spin (original 0.517 → 0.694, port 0.517 → 0.181). A
  and P physics give the same numbers. The seeded human (no pad, on the line) lacks something not
  found. Filed as S329. Its mark is 0.
- **ai0's early centre drift was the lag.** Without the lag, ai0's centre parts at tick 4 (z about
  +0.032 a tick, roughly twice the original's). With the lag, all four words agree through 14. This
  is noted in S330 in case it comes back.
- **The AI bodies part at launch:** small quat and velocity-x differences at ticks 15 (ai0),
  19 (ai1) and 17 (ai2). Filed as S330.
- **Only the first 14–18 ticks are proved,** not the 1800 recorded. The replay does not reach any
  route-follow, avoidance or trick decision past the start.
- **The roster is not spawned at a map load.** The Lane is built from C33's image in the test, not
  from `Pilot::spawn` and grid slots in the shell. Filed as S331.
- **The AI's walk sees no moving objects** (S326), and the lane is not in the snapshot or hash (S327).

## Next

S328 first: the missing draws hold the generator, and every AI decision after tick 7 depends on it.
