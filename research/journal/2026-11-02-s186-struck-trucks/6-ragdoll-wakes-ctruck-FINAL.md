# 6 — the ragdoll in the way wakes ctruck, held to the original (FINAL)

Task S273. 2026-11-03.

## What I did

- **The test.** `crates/game-sim/tests/replay.rs`
  `a_ragdoll_in_the_way_wakes_the_truck_driving_into_it_as_the_original_does` uses S252a's `doll`
  run: `context/capture/s252a/doll.csv` and the image `context/states/s252a-doll-11957.eeMemory.bin`
  (which is the capture's 11958 row). It sets up the down rider like S252a's bike test does (reason 5,
  the wreck bike's body 0x0196f020 from the state) and adds the rider's ragdoll. `Down::enter` builds
  the ragdoll on the kit, and `s273_doll` then copies in the state's ragdoll from body actor 0x009ffc50:
  - all 18 bones' bodies, masses, inertias and references;
  - the 17 joints' settings and all 21 pins (holds, then joints);
  - the 9 bone collisions' switches, checked bone by bone against the records;
  - the holds, the throw, and the wreck flags (`entered`, `stiff`, `riding`, `off_bike`, `flung`,
    `settled`, `roll`, `pull`).
  This copy follows `tests/ragdoll_wrecks.rs`'s `ragdoll_of`. The test then runs among the state's
  free06 cars (`s252_traffic`) with no pad, three ways:
  - **Pinned:** each bone's position and velocity are put on the capture's row before every tick up
    to the wake. The truck's path point matches the capture on every tick before the wake (within
    1e-3). ctruck wakes on tick 4 (11962) and stops at the original's point.
  - **Free:** the port's own bones, starting from the state's, also wake ctruck on tick 4.
  - **Lifted:** the bones are pinned 100 ft up and the wreck bike is left where it is. The truck drives
    on, so the strike is a bone's (`ragdoll_scene::PathCar::struck` → `Prop::strike_heard` →
    `Traffic::heard`) and not the wreck bike's.
  No port code changed. S252's wiring holds for the ragdoll as it is.
- **doll.csv not recorded again.** Its wreck columns read the wrong body (0x0195c6a0). The test does
  not use them: the wreck bike comes from the state, which holds the right body, and is never pinned.
  So I did not record again with the fixed watch file.
- **research/oracle.txt.** Four of MD82b's notes (`atv_follow_fields` .. `atv_camera_dt`, lines
  8523-8526) were written as name lines with no address. The oracle parser rejected them, and so did every
  test that reads names, among them S252's and S252a's. I turned them into `#` comments and left
  the text as it was.

## How it was checked

- `cargo test --release -p game-sim --test replay wakes_the_truck -- --nocapture`: the doll, bike
  and plane tests all pass. Doll prints `Some((4, true, ..))` for both pinned and free, and `None` for
  lifted.

## Not verified / not 1:1

- **The bones' tumble is not compared to the capture.** In the pinned run, the bone quats and rates
  are the port's own, from the state's, and only position and velocity are put on the rows. The free
  run's bone paths are not held to doll.csv's rows. Only its wake tick and car are.
- **Not every part of the ragdoll is seeded:** the body actor's mode machine, its contacts and the bones'
  search start from `Down::enter`, not from the state. So the search is built from the test's
  second tick, and a strike on its first tick would be missed. The wake is on tick 4.
- **Which bone struck, and the mid-tick stand-down for a bone strike** (the ragdoll records at
  0x00a57e64 / 0x00a580f0) are not compared.
