# 1 — head hits on cars (FINAL)

Task S191a. 2026-11-01.

## What I did

- **Found where stand's lone head entry came from.** I put temporary prints on `mover_contact::struck`
  and on the wreck test's list (since removed). Stand's tick-65 entry was not a ray hit. It came from
  `inside`, the port's own rule: if the head ray's end lies inside a car's root box and the car's
  forward end came over it this tick (S216's `leading`), the port adds a contact. Without `leading`,
  the rule's other half (`enclosed`, the end shut in by the car's mesh) fires on tick 75 instead. The
  original has neither: it carries the bike to tick 93 and wrecks there with its own rays' head hit.
  So I removed `inside` and `enclosed` whole. Stand now agrees on every lane to 93 and enters 6 on 93,
  as the original does.
- **Found why roof ends 6 instead of 8.** With `inside` off, roof's tick-158 entry is a real ray hit:
  the head ray (reach 6.674) meets car 9's rear end face near its top, from outside, coming in. So the
  original's ray most likely strikes the car too. What differs is the order of the verdict.
  `bike_wreck_test` (`./kit fn 0x002522c0`) enters the nose-dive through the bike's vt+0xfc, and
  then, at its end, asks the hit loop. That loop re-reads the record (+0x7d0) and asks +0x40 first.
  The entry chain is `player_wreck_entry` (`./kit fn 0x0027b810`), then `bike_wreck_entry`
  (`./kit fn 0x00252ca0`), which calls 0x0026f720(bike+0x60c, record). That unfile's first call is
  the record's vt+0x74, which is `collision_record_clear` (0x002702e0, read from the image's vtable
  0x00454a00): it zeroes +0x100 and +0x40..+0x4c. So once any landing clause has entered a wreck, the
  hit loop reads nothing. S37's oracle line said the hit's reason stays. It came from Ee harnesses that
  hook vt+0xfc to log codes, so the real entry never ran there. Corrected in research/oracle.txt.
- **Port:** `rider.rs` asks `air::hit_wreck` only when the landing test (with the port's own backwards
  landing) answered nothing, and composes `landed.or(hit)`. `air::hit_wreck`'s doc says so.
- **Tests:**
  - `tests/replay.rs` `S191_RIDES`: stand (93, 6, 6) and roof (158, 8, 8).
  - New `tests/ride.rs` `a_wreck_entry_clears_the_riding_record_before_the_hit_loop`. It takes the
    s191-roof RAM image (gp set to boot_gp), gives the record a touched flag and a count of 1, and
    runs the bike's own vt+0xfc with reason 8 in the original's code: both come out 0 and +0x748 is
    8. As a control, the same test with `collision_record_clear` hooked to a no-op fails, (1, 1)
    against (0, 0).
  - `crates/openmx/tests/train.rs`: "a bike inside a car comes off at once" checked only the removed
    stand-in, with no capture behind it, so I dropped it and left a comment saying why. The
    from-behind ride used to require a scenery wreck within 100 frames, which only the stand-in gave
    at that spot. It now requires what both captures show: the nose carries the bike, about 47 ft by
    frame 100, with no wreck over the bars. Without the stand-in it rides 400 frames with no wreck.
  - `tests/moving_vehicles.rs`: the module doc no longer names the stand-in as current.

## What the original does

- A wreck's entry takes the riding bike's collision record out of the broad phase, and clears its
  count and touched flags as it does. Within one `bike_wreck_test`, then, a landing clause's reason is
  never replaced by the hit loop's. Only the trailing striker clause (+0x7d8, reason 5) can still
  follow, because it does not read the record.
- A head ray whose end is inside a car's mesh adds no entry by itself: the ray meets only back faces
  there, and nothing stands in for it.

## How it was proved

- `cargo test --release -p game-sim --test replay the_train_wrecks_and_carries`: headon parts on 60
  (an ulp, the wreck's own tick), stand on 93 and roof on 158. The original's wreck and the port's are
  (60, 1), (93, 6) and (158, 8) in both.
- `cargo test --release -p game-sim --test ride a_wreck_entry_clears`: the original's entry run on
  the image, plus the control above.
- `cargo test --release -p game-sim --test replay --test traffic --test bike_col --test
  moving_vehicles` and `cargo test --release -p openmx --test train`: all pass in s2, with the
  S129/S146/S162/S190/S191 captures present.
- The joust stacks (tests/ride.rs `stacked`, tests/seat_rivals.rs `stacked`, openmx main.rs
  `rider_at`) start the stock stand-in bike in the air, and its airborne clause answers `NoseDive`
  on that first frame (`Sim::new`'s note); they had read 7 only because the hit used to override
  the landing clause. They now ride with `air_wrecks` off, so the touch is still what they test.
- `./kit check`: TESTS PASSED, 3650 tests.

## Not verified / not 1:1

- **The original's contact list on stand's ticks 65–92 and roof's 158 was not read live.** The task
  suggested recording it from copy 2's slots 18/19. Two things made that unnecessary for the verdicts.
  Stand's entry was shown to be port-made, and the port without it replays the original bit for bit to
  its wreck. Roof's verdict follows from the entry's clear, which was run in the original's code. The
  list itself (entries, on_body, other/mover) was never compared, entry by entry, with the port's.
- **The entry's clear was run on a RAM image, not inside a live `bike_wreck_test` call** where a
  landing clause and a head hit come together. The order (entry, then a re-read of +0x7d0, then the
  +0x40 gate) is read off the decompile.
- **A bike the train already holds** (rays starting inside a car): the original's behaviour there is
  not recorded, and the port now does what its rays find (in the shell, the train test's spot gave a
  reason 4 bail at about frame 60, no scenery wreck at once).
- **The from-behind shell test's spot** is neither capture. Only its carrying is asserted, not when
  (or whether) its head hit comes.

## Next

none: FINAL.
