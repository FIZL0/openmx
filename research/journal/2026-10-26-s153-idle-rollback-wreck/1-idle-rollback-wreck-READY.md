# 1 — idle rollback wreck (READY)

Task S153. 2026-10-26.

## What I did

- **Tried to make it happen in the port, with nobody on the pad.** Every run below uses `--trace`, an idle
  pad and race125p:
  - free09's steep slope (4347,3368), facing up and facing down: rolls back to −52 ft/s, no wreck.
  - free06's and free09's default starts, 3000 ticks each: no wreck.
  - All 8 of free06's start spots, 3000 ticks each: the lowest speed is −13.9 ft/s, and no spot wrecks.
  - Gas for N ticks, then let go (`--trace-switch`). On free09, N = 20/30/40/60/90. On free06,
    N = 15…300: it rolls back to −40 ft/s and never wrecks.
  - `--tricks mxvatv`, and the headless shell (`Sim::poll` + `Sim::step`, car contacts included) with 1
    and 2 players on free06 for 3000 frames: no wreck.
- **Swept both tracks.** This is `sweep/jobs.sh` and `sweep/sweep.sh`. It drops an idle bike at every
  300 ft over x, z 1000..6000, at four headings, for 450 ticks: 2312 runs. There were 9 wrecks, listed in
  `sweep/wrecks.txt`, and every one is an impact:
  - free09 2200,2500 heading 180 is Front, at +20 ft/s into an obstacle. free09 2800,4300 heading 90 is
    Scenery, rolling forwards.
  - free06 4000,4000 is a bump at about 3949,4103. Rolling back into it gives Back(4) at −40 ft/s, after
    pitch jumps −17.8° in one tick. Heading 0 wrecks at the same bump going *forwards* at +36 ft/s. Other
    spots on free06 show the same thing:
    - 2500,3700 heading 90 is Back at −41 ft/s.
    - 2500,4300 heading 0 is Scenery at −38 ft/s.
    - 2500,4300 heading 180 is Scenery going forwards at +46 ft/s.
    - 2500,3700 heading 270 is a forward 40 ft/s crash. Its tick-before "signed −0.34" is the bounce.
  - free06 2500,3100 heading 90 is the only slow one: Scenery at −0.49 ft/s. At tick 40 the bike runs into
    a prop at 7 ft/s and bounces off. Crawling back, the head ray (`air::HEAD`) still touches the prop, and
    `air::hit_wreck` calls any head-ray contact on scenery a wreck, whatever the speed. This is a prop
    contact, not a bike rolling back on open ground.
- **Read the wreck test.** `air::wreck_test_in` has no clause that depends on direction: g loads, ground
  pitch, airborne attitude, landing crossup and trick endings only. A bike rolling backwards meets the
  same limits as one rolling forwards. Backwards it bails as Back where forwards it bails as Front. I
  found no sign bug.
- **Captured the original doing the user's thing.** The capture runs at a quarter speed with S68's
  recipe, unchanged (`capture/setup.py 0.25`, `capture/capture.sh rise 700`, pad script `capture/rise.pad`):
  - From first_recording's free09 standstill, the gas is held for one ridden second, then let go.
  - The bike runs down to about 55 ft/s, climbs the rise to z ≈ 3730, stops near frame 3250 and rolls back
    to −14.7 ft/s.
  - The result is `context/capture/s153-idle/rise.csv` (git-ignored): 700 rows, frames 2775..3474, no torn
    rows and no gaps. wreck_reason is 0 on every row.
- **Wrote the test** `the_captured_idle_roll_back_never_wrecks` in `crates/game-sim/tests/replay.rs`. It
  steps the capture through `Sim::poll` + `Sim::step` with the pad aligned as S68's test aligns it:
  - It asserts the original never wrecks, the port never wrecks, and the port rolls back as far as the
    original.
  - It also asserts the port's roll, pitch and speed stay within S68's tolerances (0.03 rad, 1 ft/s)
    through tick 699.
  - Result: "rise: no wreck in 699 ticks, agrees through tick 699; rolls back to -14.68 ft/s, the
    original -14.68".
  - S68's `the_captured_backwards_turns_never_wreck` passes on this HEAD too.

## What the original does

When nobody is on the pad, the rider stays seated: the stopped bike rolls back down the rise with
wreck_reason 0 the whole way, and the port does the same down to the hundredth of a ft/s. The port's foot
down is render-side only (`foot_down.rs`) and has no say in the wreck test, so the "foot-down state" the
brief asks for has no part in a wreck. I found no clause that wrecks a rider for rolling backwards.

## How it was proved

Through a save-state capture, compared tick by tick: `context/capture/s153-idle/rise.csv`, 700 rows, run
by `the_captured_idle_roll_back_never_wrecks`. Separately, 2312 idle drops in the port on two tracks, plus
the targeted traces listed above.

## Not verified / not 1:1

- **I did not reproduce the user's wreck.** I don't know the spot, the seat, the settings or the history
  of the run they saw. Their settings.txt on 10-23 said free06, mxvatv tricks, wrecks on, 8 bikes, split
  screen. Neither the trace nor the 1-2 player headless shell wrecks an idle bike on open ground.
- **The backward-impact wrecks were not compared with the original.** Rolling back into free06's bump at
  4000,4000 at −40 ft/s gives Back(4). The original might take that bump without a wreck. I did not
  capture it: that needs a free06 state at that spot, by teleport.
- **The slow Scenery wreck at free06 2500,3100 was not compared with the original.** Its head-ray reach
  test has no speed gate. Whether the original also bails a rider whose head grazes a prop at 0.5 ft/s is
  unproven.
- **The shell and the trace part ways.** The shell's path (car contacts, frame_bumped scenery rays) differs
  from `--trace` over 3000 idle ticks on free06. Only 1- and 2-player shells were tried, not 8 bikes in
  split screen with AI or other riders nearby.
- **There is only one capture.** It is free09's rise, at a quarter speed, on race125p. free06's start hill
  in the original was not captured.

## Next

The user has to say where it happened: the track, the spot (or a shot), the number of players, and
whether the bike had hit something first. A `.p2m2` or an openmx `--record` tape of it would be better.
With that, rerun `--trace-at` there. If it is the bump or prop case, capture the original at the same spot
with the free06 teleport recipe and compare its reason and tick.
