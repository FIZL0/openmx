# 3 — the idle sweep after S151 and S152 (FINAL)

Task S153, third pass. 2026-10-28, on main 994b805b.

The task came back because S151 and S152 landed. The second pass left two questions for them: are path
cars movers or scenery to a parked bike, and does the head ray still sweep props after the frame. S151
made a path car a mover on the bike's collision record, and the wreck test still reads it as scenery.
S152 retired the frame-end head sweep where the body-ray search runs. No new word from the user is in
the plan (no spot, no tape).

## What I did

- **Reran the 2312-drop idle sweep** (`sweep/jobs.sh`, release build, race125p, 450 idle ticks, free09
  and free06) on this main. The result is in `sweep/wrecks-2026-10-28.txt`: 17 wrecks, up from 14. I ran
  it twice. The first run's binary was rebuilt under it twice, so I rebuilt from clean source and ran it
  again. The two runs agree line for line.
- **The 14 spots of 10-27 all still wreck**, on about the same ticks (±2). Three spots are new:
  free09 2800,4300 at h0 and h90, and free06 3700,3100 at h180.
- **Instrumented the body-ray contacts** with a temporary `eprintln!` in `rider_contact::scenery`,
  printing each contact's mover and point, and reverted it afterwards. I also put one in
  `mover_contact::struck`, which listed nothing at free06 2500,4000. Every mover these show is a path
  car: `world::Prop::moving` is set only for those (S136). What the instrumented reruns show:
  - **free06 2500,4000 h90 (Scenery, tick 258).** On 10-27 this read as a car through a rolling-back
    rider's head. Now it is mover 362: from tick 204, the bike, almost stopped at 3.1 ft/s, is pushed
    for 54 ticks on flat ground (y 135.4) up to 27 ft/s, and then the head's contact on the car wrecks
    it. h270 is the same car at tick 194.
  - **free09 2800,4300 h0 (tick 230), h90 (tick 275), h270 (tick 234).** Movers 279/280 arrive at head
    height (contact points at y ≈ 107, with the bike's ground at 103.4). They take a bike sitting at
    0.13 ft/s to 12 and then 35 ft/s in two ticks. This is the "kick" the second pass could not name: a
    road car, not a toy.
  - **free06 3700,3100 h180 (Front, tick 82).** From about tick 30, mover 374 holds the bike and carries
    it at 20–29 ft/s. The bike leaves the ground, and lands at −75° pitch on tick 82.
  - The others are as the second pass describes: ledge falls, and fast backward rolls into free06's
    bumps (S162).
- **Reran this task's test and S68's** on s1's captures (`GAME_SIM_FIXTURES=<s1>/context`, because this
  slot has no `s153-idle`). The log reads "rise: no wreck in 699 ticks, agrees through tick 699; rolls
  back to -14.68 ft/s, the original -14.68", and S68's flat (330) and slope (359) are both wreck-free.
- **Filed S204**: capture the original with a bike parked in a path car's way, and match its reason and
  tick or its no-wreck. S170 asks this for the train's nose only.

## What the original does

Nothing new on the original's side in this pass. The first pass's capture still stands: on free09's
rise, an idle stopped bike rolls back to −14.7 ft/s with wreck_reason 0 on every row, and the port
matches it.

## How it was proved

- A save-state capture compared tick by tick: `the_captured_idle_roll_back_never_wrecks` (s1's
  `context/capture/s153-idle/rise.csv`, 700 rows), and S68's `the_captured_backwards_turns_never_wreck`.
- On the port's side only: the deterministic sweep (two identical runs), and instrumented reruns of
  every new spot.

## Why it lands now

The brief's test asks two things. The captured idle roll-back must ride as the original's, and it does,
to the hundredth of a ft/s. S68's backwards captures must stay wreck-free, and they do.

On the repro side, three sweeps and every hand trace agree. No idle bike rolling back on open ground
bails in the port, and no wreck clause depends on which way the bike rolls. What the port wrecks an
idle rider for is always something striking or throwing them: a path car (now the most common), a
ledge, or a fast roll into a bump. Comparing those with the original is S162 (bumps), S170 (the train)
and S204 (path cars). Leaving S153 open would hold the same question a third time, waiting on a report
nobody can add to.

## Not verified / not 1:1

- **I did not reproduce the user's wreck itself.** Their spot, seat count and settings are unknown. The
  likeliest match is a path car running into a bike left idle on or near a road (S204). That is an
  inference from the sweep, not from the user's ride.
- **The path car hits are not compared with the original.** That covers the push and carry of an idle
  bike by a car's mover contact, and the head contact then wrecking it as Scenery or Front. The original
  might wreck the rider at first touch, push the bike without a wreck, or do what the port does. S204
  captures it.
- **I did not identify the cars.** I did not check which path car props 279/280 on free09 and 362/374 on
  free06 are (a road truck, the train, or a scraper), only that each is a moving path car.
- **The ledge falls and free06's backward impacts are not compared with the original.** Those are
  S162's.
- **Only one capture of the original's idle roll-back exists.** It is free09's rise, race125p, at a
  quarter speed. It lives in s1 only, and in the other slots the test skips and still passes.
- **Only race125p and one rider were swept.** There was no split screen, no AI riders and no other bike.

## Next

Done here. S204 (path cars into a parked rider), S162 and S170 hold the comparisons with the original.
If the user reports the bail again with a spot or a tape, rerun `--trace-at` there first.
