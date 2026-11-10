# 1 — s12 ai target seeker (FINAL)

Task S282a. 2026-11-10.

## What I did

- **Ported the free-ride target seeker** as `ai::seek` in `crates/game-sim/src/ai.rs`. It adds only new
  items, and no existing code moved. The pieces:
  - `seek` is one tick of the seeker: retarget, approach states 1/2/3, the landing learning, the hand-off.
  - `reach` is VehicleAI vt+0x80, the steer-ahead.
  - `hand_off` is VehicleAI vt+0x94: aim point, heading and target speed.
  - `crossed` is the crossing test `./kit fn 0x003832f0`.
  - The data is `Leg`/`Jump`/`Target`/`Course`/`Set`/`World` for the map side, `Seeker` for the brain's
    fields and `Ride` for what is read off the rider, interface, race and bike.
  - The result is a `Seek` that says how the tick ended: `Still`, `Lost`, `Path` or `Steered`.
- **Oracled it** in `crates/game-sim/tests/ai.rs`. The test runs the original's `ai_tick_flag` on
  generated worlds and seekers. It stops at the path branch and the tail, and it hooks vt+0x80/vt+0x94
  with the original's own functions.
- **Named the addresses** in `research/oracle.txt`: ai_seek_reach, ai_seek_hand_off, ai_seek_crossed,
  ai_seek_sequence_on, ai_seek_sequence_root, ai_seek_list_build, ai_seek_miss_start,
  ai_seek_path_branch and ai_seek_tail, plus S282a notes on ai_tick_flag.
- Reworded S282a's own plan line, leaving the checkbox alone, to say ai_seek_crossed/_reach/_hand_off in
  place of their addresses. Once named, those addresses would have failed the audit.
- Took nothing from opentennis, which has no seeker.

## What the original does

The seeker runs inside `ai_tick_flag` (`./kit fn 0x00383580`), from 0x00383634 to the tail at 0x00384970.
`./kit dis` was used where `kit fn` has no function (0x00283560, 0x00284ba0).

- **The race hold.** With a race, phase 4 needs the bike's flag 0x100, and any other phase needs flag 2.
  Without it, steer, pitch, throttle and brake (+0x1f0..+0x1fc) are zeroed and the tick returns.
- **Retarget.** A rider takes a new target when it has none, or when its target is done for its player on
  a course. The target comes from one of three places:
  - If the sequence word at 0x004e2658 is set, it is the next one in the fixed sequence (root 0x004e2778
    → +0x44 → +0x50 → +0x48, count +0x30, items +0x38).
  - Otherwise, if the set has a course (+0x114), it is the nearest target not yet done, scanned from
    +0x348. The course's chance (+0x94) is rolled against `rand`, and the roll sets or clears the
    headlong bit 0x1000 in flags2.
  - Otherwise it is drawn at random from the set's pick list (+0x11c → +0x48), with up to 100 draws until
    the pick differs from the last target. A set with no list is built by 0x003a9f10.
  - A new target resets state 0, tries, the +0x358 count, and the miss to (0,1,0,1) from 0x00426db0. It
    also sets the learning steps, which differ in sequence mode. A take-off with velocity 0 is given 150
    along the jump.
- **States.**
  - 0 becomes 1, unless the race has a path net and +0xdc is set (the path branch 0x00383a84, S282b's).
  - 1 rides to the setup leg. It becomes 2 once the setup is crossed, the rider is on the ground and it
    faces the jump.
  - 2 is the run-in to the lip. It becomes 3 within 20 ft of the lip. The take-off sets CLOSING
    (0x08000000) and LEG_DONE.
  - 3 is the flight. Landing on the ground with bike +0xfc > 0.5 and NO_LEARN (0x400000) clear measures
    the miss from the landing point. The target is dropped when the landing is within tolerance, the
    tries are used up, or the target is done. Otherwise it learns and goes back to 1:
    - The launch direction turns one step toward the miss side, at most 45° off the jump line.
    - Speed and the learnt count step with the along-miss.
    - All steps halve when the miss changes sign.
    - A learnt count past 100 re-aims the lip within 10 ft, using two `rand` draws.
- **The hand-off.** Each state gives vt+0x94 a point, a direction (in state 3 always landing − position;
  the on-ground direction computed before it is dead), a speed, a distance, a lead, and vt+0x80's reach.
  - The speed is the leg velocity's length × the interface scale. It is `slow_speed` inside the setup
    radius, and 150 when the leg has none or the rider is headlong.
  - vt+0x94 sets or clears CLOSING by where the rider is along the line. It writes aim +0x1b0, the flat
    unit heading +0x1d0 and speed +0x1e0, slowing toward the minimum by the heading's turn off forward
    when nearer than the lead.
- **The crossing test 0x003832f0.** The rider is past the leg's point along its facing now and was not
  last tick, and the crossing point on the step (pos +0xc0, last +0x290) is within +0x35c of the point.

## How it was proved

`./kit check -- -p game-sim --test ai`: all 11 tests pass, one of them new and needing no image.

- `the_target_seeker_matches_over_ten_thousand_ticks` (seed 0x5282_a7a2_93e7_5eed): 10,000 generated
  cases.
  - Each case lays out a world in RAM: sets, pick lists, courses, the sequence chain, targets with
    per-player done words, and jumps with three legs each. It also lays out a rider, interface, race and
    bike with random flags, phases, positions near each leg, and tolerances. Then it runs the original's
    `ai_tick_flag` from its start until the path-branch or tail stop hook, or a return.
  - Compared bit for bit: how the call ended; every seeker field (flags, set, target, last, jump, index,
    state, tries, tolerance, learnt, three steps, last miss, miss, lead, distance, +0xdc follow word, aim,
    heading, speed); the four controls; every leg of every jump, which carries the learning; and the
    `rand` state.
  - Branch counts in one run (taken before the last generator tweak, which only adds same-point steps):
    - Outcomes: still 1696, lost 1534, path 441, steered 6329; 2324 cases drew from `rand`.
    - Picks: 813 from the sequence, 769 from a course, 972 from a pick list.
    - State moves: 1→2 132, 2→3 237, 3→1 682 (429 learnt), 3 dropped 171, 13 landed.
    - Other branches: flat out 2555, CLOSING flipped 2538, steps halved 79, lip re-aimed 388.
    - The test asserts a minimum for each count.
- Mutants, each made, run, then restored with cp + touch:
  - All killed: the learning's z read from the old x, halving keyed on the along-miss, `tries > 3`, the
    reach fed the distance, and the lip radius at 21 ft.
  - `then <= 0` in `crossed` at first survived. It was killed once the generator sometimes puts last
    position exactly on the landing point.

## Not verified / not 1:1

- **No real world was compared.** The only AI image, recording2 on free09, has three AI riders in state 1
  with real target sets. They were not read into a port `World` and stepped against the original. Reason:
  reading a whole set/target/jump graph out of RAM is a reader of its own, and the generated oracle
  already covers every branch bit for bit. So the field meanings (setup / lip / landing, "done", "chance")
  come from how the code uses them, not from data. Filed as S305.
- **The pick-list build 0x003a9f10 is not ported.** `Set::picks: None` picks nothing, and the oracle
  hooks the build as a no-op. Reason: it builds the list out of the map's objects, which is the map
  loader's job, not the seeker's. Filed as S306.
- **The path branch (0x00383a84) and the tail after the hand-off (0x00384970 on)** are S282b's. The
  oracle stops there, and `Seek::Path`/`Seek::Steered` say so.
- **Reads of memory the original does not own** are modelled as "no target":
  - an empty list's entry 0;
  - a course scan with no candidate, which reads items[-1];
  - the sequence mode, which does not null-check its target or jump and would crash;
  - phase 4 with no bike, where the head reads bike+8 unchecked and the port reads the flags as 0;
  - a state outside 1–3, which has no leg, so the velocity is zero and the speed takes the 150 case.

  The oracle puts a zero word on either side of each list, so the original reads the same "none" there.
  The crash cases are not generated.
- **Not traced:**
  - who writes the sequence word 0x004e2658 (no writer in the ELF);
  - who sets NO_LEARN 0x400000;
  - what bike +0xfc is (the port calls it `settle`);
  - who increments +0x358, the landings count the seeker reads and zeroes.

  The seeker only reads them, and the port takes them as inputs.
- **`seek` is not wired into the AI tick or the Sim.** Reason: the tail (S282b) sits between it and the
  controller. S12e already covers the wiring.
- **MX vs. ATV's PC exe was not read.** Reason: time. The PS2 oracle is the binding proof.
- **No live capture** of a free09 AI rider choosing a jump was compared, because the emulator was not
  started this session.

## Next

S282b (the path follower and the tick's tail) and then wiring `ai::seek::seek` into the AI tick. S305 (the
recording2 world check) can go first: it needs nothing else.
