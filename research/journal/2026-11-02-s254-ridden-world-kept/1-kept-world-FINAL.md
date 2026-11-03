# 1 — kept world (FINAL)

Task S254. 2026-11-02.

## What I did

- `flight_search::refresh_ridden`: `kept_world(props, trees, None, &[], field, centre, radius, true, …)`
  with `lend`/`give_back`, the same world (same stamp) as `refresh_flown` with no bikes and the prop
  probes' `Scene::probed`. The kept world is forgotten when its stamp changes (the props, the trees,
  the field), and brought up to moved path cars by `Kept::update`, as before.
- The ignore list (a ghost's `GHOST`) is not part of the world: it is put on the lent list's
  `objects[RAY].categories` for the gather and put back to the world's own before `give_back`, since
  `lend`'s fast path only restores flags/group/radius. Without that the ghost list would stay on a
  spare list and a later `refresh_flown` in the same world would gather without the cars.

## What the original does

Nothing ported; a cost change only. The search is the same.

## How it was proved

- New `tests/kept_world.rs` `the_ridden_refresh_gathers_as_the_fresh_one`: 30 ticks of driving cars,
  three fields, with and without plants, a ghost ignore list every third tick: the kept refresh's
  nearby list equals a fresh `world_flown` + gather's; then `refresh_flown`'s gather and
  `Scene::flown`'s ray hits in the same kept world equal those on a new thread (no kept worlds).
  Mutant: dropping the categories' put-back goes red (the plane's gather after the ridden refresh).
- `cargo test -p game-sim --no-fail-fast` in this slot: 1056 passed, 1 failed (`the_humans_tricks`,
  the known red on C6's capture). The same tree run against s3's context: 1057 passed, 0 failed; against
  s2's context: 1056 passed, 1 failed (`the_humans_tricks` again). Those contexts hold the S195 knock,
  S129/S191 train, S146 roof and S171 wreck states that skip in s1.
- Timing (release, `OPENMX_THREADS=1 openmx --headless --track free06 --players 2 --timings --frames
  600 --no-sound`, three runs each): sim 1.65/1.62/1.64 ms before, 1.06/1.06/1.06 after.

## Not verified / not 1:1

- Not timed on the Steam Deck itself, only on this machine (the ~35% cut matches the profile's ~37%).
- The openmx crate's tests (netplay, shell) were not run; they share game-sim's search code, which the
  game-sim replay tests above cover.

## Next

none
