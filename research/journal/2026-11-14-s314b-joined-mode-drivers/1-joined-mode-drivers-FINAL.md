# 1 — joined mode drivers (FINAL)

Task S314b. 2026-11-14.

## What I did

- Joined `ai::drive::mode_two` (the race driver) and `ai::drive::mode_four` (the gate driver) into
  `ai::Racer`'s `Steps`. `Join::mode_two`/`mode_four` no longer tell "mode two"/"mode four" missing
  when the tick has a race to drive by.
- `ai::Around` gained `racing: Option<Racing>`. `Racing` holds the brain's `drive::Race` and
  `drive::Card`, the AI path net, the race track's own paths, the card's next gate (`race::Gate`, for the
  crossing test), the track's line gate centre and normal (for the path reach's re-spot), and the race
  word (route follower side step). With `None` the join still tells "mode two"/"mode four" missing.
  The Sim's `Pilot::think` passes `None` (follow-up S332).
- `ai::Racer` gained two fields. `follower: follow::Follower` holds the path follower's own words (spot,
  draws, drift, kick, period, cruise, look). `driver: DriverWords` holds the drivers' words that had no
  home: gate gain +0x258, goal word +0x31c, goal speed +0x320, last speed +0x324. Every other driver word
  maps to its existing home:
  - flags → `Brain.flags`
  - speed, forward, position → `senses`
  - aim, heading, target speed → `seeker`
  - assist pair → `assist_x`/`assist_z`
  - angles → `controls` aim/heading/roll
  - reach → `reach`
  - gate speed → `min_speed`
  - gate lead → `seeker.lead`
  - last position → `senses.last_position`

  The drivers' calls (`drive::Calls`) are answered by a `Driven` adaptor over the join. Before each call
  it puts the driver's words into their homes and takes them back after:
  - point query → the race track's paths (`TrackPaths::spot_point`)
  - place → `follow::spot`
  - vt+0x78 → `follow::reach`
  - vt+0x7c → `drive::path_reach` on the follower's spot
  - vt+0x8c → `follow::route_follow`
  - crossing → `race::Gate::crossed`
  - distance and look-ahead → the net's own
  - seek reach and hand-off → `seek::reach`/`seek::hand_off`
  - avoidance → the motocross `avoid` for a mode-2 brain whose card has the bike word, else
    `avoid_vehicle`, else "vehicle avoid" told
  - trick plan → `plan_trick`
  - assist → `assist`
  - speed step → tells the new `Told::missing` entry "speed step" (S314a)
- `Racer::drive` runs just the brain's mode driver on the join, for the proof. `Racer::tick` runs the
  same `Join::drive` inside the base update.
- `ai::spawn` now fills the follower from the wander it builds (draws, drift, kick, period, cruise).
  `Racer::take` puts the character's drift low/high/rate/max (+0x17c..+0x188) and the no-track look-ahead
  and min turn (+0x2f0/+0x2f4) in. `take` now runs after the follower fill.

## What the original does

S314's journal (2026-11-13-s314-ai-mode-drivers) has the drivers themselves. New here, read on sx01's
race image (s282c-sx01-race.ee.bin):

- **The race track and the AI net are two objects.** The race's track (race +0x68) has 2 paths. The brain's
  AI net (+0xdc) has 38. The card's kept spot (card +0x94) and the line gate's spot are on the race
  track's paths, and the follower's spot (+0x220) is on the AI net. So the race driver's goal point
  (`path_spot_point` on race +0x68) must ask the track's paths. In the port a spot is an index into one
  `TrackPaths`, so `Racing` carries both.
- All three race brains have the card's bike word (+0x40) set, so their avoidance is the motocross pass,
  and all have AVOIDS and ASSIST set and SLOWS clear. No card has a next gate (+0x80 is 0).
- The rider is at the interface's +0xc (recording2's layout holds on sx01 too).

## How it was proved

All in crates/game-sim/tests/ai/drive.rs. On sx01's race image the original's drivers run **whole in
the interpreter, nothing hooked**. Every callee is the original's own: point query, path reach, route
follower, own path reach, gate crossing, net distance and look-ahead, seek reach and hand-off, motocross
avoidance, trick plan, the interface's assist. They run against `Racer::drive` on racers read from the
same image (`s12e_racer`/`s12e_bike` from sim.rs, the follower and nets from path_branch.rs).

Each brain drives as it stands, then 200 more times, moved along its own heading at its own target
speed between drives (the card's step follows it). After each drive the test compares, bit for bit:
- the answer and nothing told missing
- flags, controls, aim, heading and target speed
- the assist pair, goal words and last speed
- all of the follower's words, the seeker's words and the last position
- the trick clocks
- the body's yaw rate and velocity (the assist's writes)
- the AI net's scratch words and pools
- the generator

- `the_joined_race_driver_rides_sx01s_race_image`: mode 2, 603 drives, all with a kept goal and
  the avoidance. The rider changed path 8 times.
- `the_joined_gate_driver_rides_sx01s_race_image`: the brains put in mode 4 with the gate watch, a
  stand-in gate (below). 603 drives, 57 with the turning-back flag up (crossing beside, distance both
  ways, look-ahead), 7 path changes.
- `the_joined_gate_driver_aims_at_the_gate_on_sx01s_race_image`: mode 4 with the AI net taken away
  (+0xdc zeroed). It aims at the gate, and the seek hand-off changed the seeker on 567 of 603 drives.

## Not verified / not 1:1

- **The speed step is not ported** (S314a). Under SLOWS the join tells "speed step" and leaves the
  target speed as it is. No sx01 brain has SLOWS, so the proofs never reach it.
- **The Sim does not drive by it yet.** `Pilot::think` passes `racing: None`, so the Sim's race riders in
  mode 2 still tell "mode two" missing. Filed as S332.
- **Mode 4 has still never been seen live.** The gate proofs are sx01's race brains with their mode word
  set to 4 and WATCH_GATE set, on both sides alike. No card has a next gate, so a copy of the track's line
  gate stands in at free RAM 0x01f8_0000, its spot set to the brain's own spot on the AI net.
- **The generic avoidance in mode 4 is not proved.** In mode 4 the original's vt+0x74 takes
  `avoid_vehicle`, whose scene the test does not build, so AVOIDS was cleared on both sides for the
  mode-4 proofs. The join's routing (mx for mode 2 with the bike word, else generic) is read from the
  original, not oracled on the generic branch.
- **The join's avoidance has no 45° cone.** The route's tail gates the avoidance on its cone, but the
  drivers call vt+0x74 directly, so the join runs the avoidance on every call. That matches the
  original's driver, which calls vt+0x74 directly too, and the mode-2 proof ran it 603 times.
- **The trick plan in the proofs never rolls.** The port's side has `tricks: None` and every sx01 brain
  is on the ground. It is proved only on the plan's clock (since, air) and the flags.
- **A failed point query reads as zero.** If `path_spot_point` fails, the original goes on with whatever
  its stack buffer holds, while the port's `spot_point` (and vt+0x7c's) start from a zero point. A failed
  query never happened in the proofs.
- **The point query's clamp of the spot's distance along is lost.** The original writes the clamp into
  the card's kept spot in place. The port's `drive::Card` is a copy, so the clamp does not reach the
  race's card. The clamp only acts on an `along` outside 0..1, which sx01 doesn't have.
- **The seeker's set and target words** were zeroed on both sides. They index the free-ride seek world,
  which a race has none of. The race driver doesn't read them. The gate driver's hand-off ran with them
  zero.
- **Spawn's new follower values are not oracled.** That covers the follower fill from the wander, the
  character's drift and look words in `take`, and `driver` starting at zero (gate gain etc.). The proofs
  read every word from RAM instead of spawning.
- **The cone constant** handed to the seek hand-off is ignored by `seek::hand_off`, as in S314. The
  no-net proof matched with it ignored.
- **The PC build was not read.** No live PINE capture of a driving tick. The proof is the interpreter on
  one RAM image.

## Next

Nothing for this task. S332 feeds the Sim's race into the join. S314a ports the speed step.
