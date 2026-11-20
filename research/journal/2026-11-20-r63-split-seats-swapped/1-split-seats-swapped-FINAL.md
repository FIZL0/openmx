# 1 — split seats swapped (FINAL)

Task R63. 2026-11-20.

## What I did

- Got the original into split-screen free09 with player 1 in the Dune Buggy and player 2 on a bike, all by
  two-pad tapes from the user's split state (`research/r63/step.sh`, `research/r63/fe/*.pad`). The route is
  in research/oracle.txt `# R63`.
- First tried R22's poked boot (`research/r63/boot.py`, R22's boot2.py with ev+0x11c set). It hangs on the
  loading screen with frame 0, so I dropped it and went through the front end instead. The file stays as a
  record of the dead end.
- Watched player 2's way on the gas (`research/r63/path.toml`), then put the buggy on that line with
  `research/c34/place.py` (`research/r63/place_save.sh`), facing the bike, settled (slot 34).
- Captured the ride with `research/r22/capture.sh` and `research/r63/r63-split-car.toml` (R22's spec with
  the addresses swapped): player 1 holds the brake, player 2 the gas. Strike at frame 9735. Captured twice
  (try1.csv, strike.csv): equal on every column but dt over all 240 rows.
- Saved a RAM image at frame 9729, six ticks before the strike (`research/r63/save_pre.sh`, using R22's
  play_save.sh), into context/states/r63/pre.ram. Its chassis and car words equal row 9729 exactly.
- New test `crates/game-sim/tests/ai/split_seats.rs`:
  `r63_the_seat_0_buggy_steps_before_the_seat_1_bike_that_strikes_it`. It runs the image whole in the
  interpreter for nine ticks (9730..9738) and checks each tick bit for bit against the capture. It also
  checks what ran and in what order.

## What the original does

Each tick:

1. Every record of the level node collides, in node order: truck, car, 0x01362d30, player 2's bike.
2. The node's dynamics world steps the truck and the buggy: both preforces, then four passes of
   (resolve the car's record, resolve 0x01362d30), then the buggy's measure.
3. The player updates run: player 2's ridden bike first (its record resolved twice inside), then player 1's
   parked bike.
4. The bike world steps.

So the seat-0 car steps before the seat-1 bike, as F77 says. The riders run last seat first, which is
F77's `pass_order` ("the rest last seat first"). R22 shows the same rider order: player 2's update before
player 1's.

On the strike (9735), the car's first resolve, in its own step, takes the bike from 26.57 to 2.38 ft/s and
the car to 2.00 ft/s. The lists: bike [k1, k1, k0] naming the car, car [k0, k0, k1] naming the bike,
both struck [1, 1, 0, 1].

## How it was proved

- A save-state capture: context/capture/r63/strike.csv, frames 9657..9896, repeated once (try1.csv),
  equal on all 240 rows.
- A bit-exact replay of the original in the interpreter: 9 ticks from context/states/r63/pre.ram. Every tick
  matches on the bike's chassis centre and velocity, the car's place and velocity, both records' lists and
  struck words. Run with `./kit check -- -p game-sim --test ai r63_` (1 passed, not skipped: it printed the
  strike speeds).

## Not verified / not 1:1

- The buggy's place was poked into RAM (place.py), not driven there. The original never puts it there by
  itself, though everything after the poke is the original's own stepping.
- R22's poked boot hangs for this setup (loading screen, frame 0). I did not find out why.
- 0x01362d30: pointed to from among player 1's parked bike's structures (0x013614c0..0x01362afc), beside
  its rider record 0x01360c00. Whose record it is was not proved.
- Player 2's bike knocks 0x01362d30 on frames 9716..9728, before the image. After that, the node's world
  resolves it beside the car's record on every pass. Whether a knocked parked bike joins the node world like
  this, and whether the port does the same, was not checked: it is outside R63. Filed as C45.
- The test checks the original's order and numbers. It does not run the port's split-screen step
  (main.rs `pass_order`) against this capture. F77's own split_contact test pins the port's order; that the
  two orders agree is read off by eye here, not tested.
- The rider order was seen with one ridden bike and one parked bike. Two ridden bikes beside a car (three
  players' worth) cannot happen in two-player split screen, so it was not tried.

## Next

Nothing for R63. C45 follows up the knocked parked bike in the node world.
