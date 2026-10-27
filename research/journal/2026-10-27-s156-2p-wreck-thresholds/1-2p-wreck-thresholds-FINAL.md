# 1 — 2p wreck thresholds (FINAL)

Task S156. 2026-10-27.

## What I did

- Read S155 (the 2p wrecks call by call), S150 (crossup is an arcsine, so a landing ~135° crossed up
  reads as straight) and S154 (Air Wrecks Off). S153 is a bail-option design note with no clause
  in it. S144 is still open ("landings are too generous", a tail-first touchdown rides out): it has
  no journal yet. Its question is answered in part here, under "For S144".
- Listed every clause of the original's wreck test (`./kit fn 0x002522c0`, `bike_wreck_test`) with
  its threshold and where the threshold comes from (table below).
- New `crates/game-sim/tests/wreck_thresholds.rs`, three tests:
  1. `the_2p_bikes_hold_the_ports_limits`: in C10's RAM image c10-128466, both riders' bikes hold
     +0x754..+0x790 and the reach flag +0x588. These are bit for bit what the port's `Phy::read_bytes`
     makes of `Bike\Race500P.phy` (both riders ride it).
  2. `every_wreck_clause_either_side_of_its_limit`: 57 cases on P1's real bike in that image. The bike
     is first made quiet (on the ground, square, no loads). Then one clause's quantity is set **on**
     the limit and **one float step past** it. The original's own `bike_wreck_test` runs, with the
     bike's vt +0xfc "came off" slot hooked. The port's `air::wreck_test` + `air::hit_wreck` run with
     the port's own `.phy` limits, composed as rider.rs composes them. Both must give the case's answer.
  3. `the_2p_ride_sits_either_side_of_the_limits`: every tick of C10's capture (`2p.csv`, both
     riders) where a clause judged the bike. The gate is the original's: not wrecked going in, mode 1
     or 3 the row before, plus the clause's own gate. Ridden-out ticks must be at or inside the port's
     limit, and wrecks must be past it. The bracketing pairs are pinned. The crossup wrecks' hang
     times and the trick-ending wrecks are counted.
- No change to air.rs, wreck_page.rs or phy.rs: no gap turned up (see "How it was proved").

## What the original does

Every clause, in the order the original asks them. "Limit" is Race500P's (both 2p riders), as held on
the bike: speed limits are the file's mph × 22/15 = ft/s, angles in radians.

| # | clause | quantity (bike word) | test | limit (Race500P) | source | reason |
|---|---|---|---|---|---|---|
| 0 | out of the map | bounds query on the chassis centre | outside | — | the track's bounds (off_the_map.rs) | 13 |
| 1 | fore-aft load, back | g +0x558 | `> L` | 21.9999 (15 mph) | [Wrecks] BackWreckSpeedLimit +0x758 | 4 Back |
| 2 | fore-aft load, front | g +0x558 | `< -L` | 18.3333 (12.5) | FrontWreckSpeedLimit +0x754 | 1 Front |
| 3 | side load | g +0x550 | `> L` / `< -L` | 19.0666 (13) | SideWreckSpeedLimit +0x75c | 2 Left / 3 Right |
| 4 | vertical load, no-handed | g +0x554, no-hander +0xa44 | `> L` | 29.3332 (20) | NoHanderWreckSpeedLimit +0x77c | 5 Flat |
| 5 | vertical load, racing | g +0x554, racing +0x810 | `> L` | 58.6664 (40) | RacingDownWreckSpeedLimit +0x760 | 5 |
| 6 | vertical load, freestyle | g +0x554, !racing | `> L` | 65.9997 (45) | FreestyleDownWreckSpeedLimit +0x764 | 5 |
| 7 | slope, nose down | pitch ch+0x90, on ground +0x574 | `< -L` | 70° | GroundPitchDownWreckAngle +0x768 | 1 Front |
| 8 | slope, nose up | pitch, on ground | `> L` | 70° | GroundPitchUpWreckAngle +0x76c | 11 Tail |
| 9 | air pitch down / roll | pitch, roll ch+0x98, airborne +0x57c, !flipping +0x4f8 | `< -L` or `\|r\| > R` | 80°, R 85° | AirbornePitchDownWreckAngle +0x770, AirborneRollWreckAngle +0x778 | 8 NoseDive |
| 10 | air pitch up / roll | same | `> L` or `\|r\| > R` | 85° | AirbornePitchUpWreckAngle +0x774 | 11 Tail |
| 11 | tail first: backflip torquing on a touch | +0x4f4, record touched +0x40 | both set | — | — | 11 |
| 12 | landing crossup | crossup +0x788 (0..1, 0.5 square), was airborne +0x580, !airborne, hang +0x4f0 | hang `> T` and (`c > C` or `c < 1-C`) | T 0.75 s, C 0.99 | CrossupWreckTime +0x790, CrossupWreck +0x78c | 2 Left (c > 0.5) / 3 Right |
| 13 | trick ending | stunt +0xa0c on the ground or touching, not let off +0xa10, not no-handed | set | trick SafeTime (trick.rs) | — | 12 NoHander |
| 14 | no-handed, rolled over | up.y +0x424, no-hander, pose not reached (+0xa78 ≠ +0xa84 or blend +0xa2c ≤ 0.95) | `< cos M` | cos 59.98° = 0.50030 | NoHanderMaxRollAngle +0x780 (held as its cosine off the curve table) | 12 |
| 15 | pose reached out of a trick | +0xa78 = +0xa84 and blend | `> 0.95` | 0.95 | code constant | 7 Joust |
| 16 | body ray hit, scenery | first own (not mirrored) entry reaching above the reach | `> reach` | 4.0 (Race500P sets the 50cc flag +0x588), else 6.0 | code constants | 6 Scenery |
| 17 | body ray hit, a rider | the other chassis' height − ours | `> H` | 2.5 | JoustWreckHeight +0x784 | 7 Joust |
| 18 | body ray hit, an object | its body's mass vs the chassis' mass ch+0x210 | ours `<` theirs | — | — | 7 |
| 19 | struck by a biplane | striker +0x7d8 category & +0x398 | non-zero | — | flight_search.rs | 5 |

Clauses 1–15 run only while the bike is not wrecked (+0x748 = 0). 16–19 always run. Every comparison
is strict, so a quantity exactly **on** its limit rides out (test 2 proves which side each is on).
There is **no roll clause on the ground**: the 85° roll limit is judged only while airborne.

Race500P counts as a 50cc for the hit reach, because its name contains "race50". The original's +0x588
is 1 on both 2p bikes, and the port's `track::fifty_reach` says the same.

## The 2p ride against the limits (C10's capture, test 3)

Ridden out = the largest value on a tick the clause judged that did not wreck. Wrecked = the clause's
value on its wreck tick.

| rider | clause | limit | ridden out (tick) | wrecked (tick) | margin ridden out |
|---|---|---|---|---|---|
| P1 | air pitch down | 80° | 79.683° (129944) | 80.528° (129945) NoseDive | 0.32° |
| P1 | air pitch up | 85° | 84.483° (130965) | 85.328° (130966) Tail | 0.52° |
| P1 | air roll | 85° | 84.845° (128994) | 85.034° (128995) NoseDive | 0.15° |
| P1 | air roll | 85° | 84.590° (131483), then lands at 85.95° (131484) and rides out: no ground roll clause | — | 0.41° |
| P2 | air roll | 85° | 81.126° (133799) | 85.141° (133800) NoseDive | 3.9° |
| P2 | ground pitch down | 70° | 66.340° (131594) | 70.807° (131595) Front | 3.7° |
| P1 | ground pitch down | 70° | 67.430° (130420) | — | 2.6° |
| P2 | ground pitch up | 70° | 60.486° (128953) | — | 9.5° |
| P2 | air pitch up | 85° | 77.060° (133797) | — | 7.9° |
| P2 | vertical g | 66.0 | 64.094 (133538) | — | 1.9 (3%) |
| P1 | vertical g | 66.0 | 55.874 (129486) | — | 10.1 |
| P2 | side g | 19.07 | 4.70 (130414) | — | — |
| P1 | fore-aft g front | 18.33 | 3.03 (131484) | — | — |
| both | fore-aft g back | 22.0 | ~0 | — | — |
| P1 | crossup (hang side) | 0.75 s | — | 1.385 s (132997 Right), 0.884 s (139659 Left) | — |
| P2 | crossup (hang side) | 0.75 s | — | 2.019 s (140363 Left) | — |
| P1 | trick ending | — | — | 6 NoHander (133313 … 136559) | — |
| P2 | trick ending | — | — | 1 NoHander (134315) | — |

So the human's ride comes within 0.15–0.5° of the air limits and rides out, then wrecks a tick later
0.03–0.53° past them. That is what "sensitive" feels like here: the air limits are sharp to a degree, and
nothing else in the ride got close except the vertical g (97 % of the limit on P2's 133538). The g loads
sideways and fore-aft never reached a quarter of their limits.

Two of the trick-ending wrecks (P1 136559, P2 134315) show the bike on the ground with the trick still
running one row *before* the wreck row. The capture reads rows at one of three phases (memory
"capture rows read at three phases"), and S155's runs entered those two a row early too. Test 3 lets
the wreck be on the row or the next.

## For S144

The original has no landing clause beyond the table. A tail-first touchdown wrecks only through:

- the vertical load (> 66.0 freestyle, 58.7 racing, 29.3 no-handed);
- the fore-aft load (> 22 back);
- the slope nose-up limit on the ground (> 70°);
- the air pitch-up limit (> 85°) on a tick still airborne;
- a backflip still torquing on the touch;
- the crossup.

Nothing judges pitch *rate*, speed, slip or the shock bottoming out. The port's clauses and limits are
the original's to the float (tests 1 and 2). So if the port's tail landings ride out where the
original's wreck, the difference is in the loads and attitudes the port's ride *produces* (the g peak
+0x550..+0x558 and the pitch at touchdown), not in the wreck test.

## How it was proved

- **Oracle either side** (test 2): 57 cases, original vs port, on a real 2p bike with the ride's limits.
  Every clause 1–18 is covered on its limit and one float step past it (both signs where it has two).
  The cases also cover the excuses: a backflip turning (no air clause), a mirrored entry, a trick let
  off, the crossup in the air. Clause 0 is off_the_map.rs's and clause 19 flight_search.rs's. Neither
  has a threshold of its own here.
- **Constants**: test 1, 16 words × 2 bikes, bit for bit.
- **2p margins**: test 3 over all 12,917 rows × 2 riders.
- Mutations, each failing, then put back:
  - air.rs's back clause made `>=`: "fore-aft g on BackWreckSpeedLimit: the port answers [4], the
    original []".
  - phy.rs's AirborneRollWreckAngle × 1.01: all three tests failed (the limit word, "rolled to
    AirborneRollWreckAngle: the port answers [], the original [8]", "P1 air roll: 128995 is not a wreck
    past the limit").
- No gap found: the port's limits and comparisons are the original's on every clause, so nothing in
  air.rs or wreck_page.rs changed.

## Not verified / not 1:1

- **The landing crossup's own value is not in the 2p capture.** The capture holds the air step's
  crossup +0x600, not the landing's +0x788, so for crossup only the hang-time side of the three crossup
  wrecks is checked against the ride. The crossup threshold itself is proven only by test 2's oracle
  cases and S150's captures. S155's runs read +0x788 on the calls they make, but this slot has no
  context/states/s155 (they are in s3's).
- **The g-load clauses, ground pitch up, the hits and rider-vs-rider have no wrecked side in this
  ride.** No tick past those limits, no rider-rider contact (C10), and P2's two Scenery wrecks are on
  the reach clause, which has no speed in it. Their wrecked side is proven only by test 2's oracle
  cases, not by the human's ride.
- **Test 3 reads the capture's end-of-tick row as what the wreck test saw.** S155 showed the test runs
  once at the end of the vehicle update, and that on runs that stay on the capture the row's values
  are the test's inputs (within 0.0015°/g). Rows read at another phase can be a tick off. That is why the
  trick-ending check allows the next row. For the angle and g clauses no ridden-out row was past a
  limit, so no phase question came up.
- NaN inputs: the original tests `!(x <= L)` in places and the port `x > L`. These differ only on a
  NaN, which the PS2 FPU does not produce. Not tested.
- The clause gate in test 3 uses the capture's mode column (1 or 3 the row before). Mode 3 is the
  riding body with a trick or lander pose, which C10's index called "riding"; the column's other
  values were not reviewed.
- `./kit audit` fails on 7 leaks, all in plan/DONE.md: other tasks' addresses (biplane_tire_* and
  others), the same ones S155 recorded. None of them is from this task, which is committed with
  `--skip-audit` for that reason.
- Test 2 runs on P1's bike only. P2's limits are the same words (test 1), so P2's bike would only
  repeat the cases.

## Next

None. S174 (the whole frame from each image) and S175 (the port's own 2p ride) are the open work around
this. Once S175 runs the port's bike over the 2p ride, its ticks could be fed through test 3's margins
to show the port's ride on the same side of every limit.
