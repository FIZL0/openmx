# 1 — kiowa capture ground (FINAL)

Task C14a. 2026-11-02.

## What I did

- `crates/game-sim/src/kiowa.rs`:
  - `Surroundings::post(&Body)`, a defaulted hook, called first in `Lists::post`: the body wrapper's
    post comes first in the post listener.
  - `Collision`: the helicopter's collision object. It has:
    - a collider from `[Mass Properties]` and the body's inertia (`wreck_bike::collider`);
    - a record with the ray probe out of `helicopter\col\kiowam.col` (22 rays, capacity 22);
    - masks off the image: collidable 0x202058, actors 8, collides-with all;
    - the near-field search.

    `search` works as `Flight::search` does: clear, sweep back, terrain rays, `collide` over
    `Scene::flown`. `refresh` is `refresh_flown`. `entries()` gives the touchdown's entries; `ignored()`
    reads the struck surface against `CRASH_MASK`.
  - `Grounded`, the `Surroundings` built on it:
    - resolve is `contact::resolve` on the record's contacts (impulses on);
    - touching and entries come from the record;
    - post refreshes the nearby list;
    - strike asks the world's ground under the rotor's wreck-ray ends (`rotor_in_ground`), with
      `struck: false`.
- `crates/game-sim/tests/kiowa_capture.rs`:
  - The per-row compare is moved out into `compare`/`moved_by`, which C14's test now uses too.
  - New test `the_ground_at_both_ends_replays_bit_exact`. It seeds the Kiowa, ride and measure from
    the image alone: the controller block, the rotor and aero states, the body, the measure's figures and
    matrix (+0x470..+0x510), and the last-touching flag (+0x420). It checks the port's collision object
    against the image's (`check_collision`). It builds free09 as the shell does (terrain, trees, props
    from free09.res/free09_2.res, all through `context/iso/MX.PSR`). Then it runs every row 1205..=2057:
    search, then tick, then poll the previous row's pad.

## What the original does

- The image's helicopter, rotor/aero and body bytes are **row 1204's** byte for byte. The image is
  not the tick before the first row, as C14's docs said: the first ticked row is 1205.
- The level's collision node searches every object before any world steps. The response runs inside
  the force listener each substep. The body wrapper's post (the nearby list) runs first in the post
  listener, then the measure, the crash test and the touchdown.
- The touchdown sound's clip count (`*(heli+0x54c)+0x200`) is 0 in the image, so the touchdown at
  1816 draws no `rand()`. The port's `touchdown_sound` is false.
- The capture's `pilot_flag` (record +0x40) reads 0 on 8 rows where the helicopter did touch: 1838,
  1843, 1862, 1874, 1876, 1877, 1881 and 1893. On each of them the port finds a ground contact, and
  that contact is needed. Dropping the port's 1-contact search at 1838 takes the body off at once:
  vel.y −2.44 against the game's −1.52. So those reads fall after the next frame's search has
  cleared the record. `flight-run2.csv` has the same 8 rows, since PCSX2 lands the read at the same
  point each run. No row shows the game touching where the port does not.

## How it was proved

A bit-exact replay of the save-state capture: `the_ground_at_both_ends_replays_bit_exact`, 853
ticks (1205..=2057), 275 of them touching. Each tick compares every captured word the frame writes,
the same set as C14's flight test (pad words, commands, loop, rotor/aero states, the measure's 23
figures, all body quadwords and flags, the wreck word 0).

It also asserts:
- the crash test (rotor in ground, then the MinWreckSpeed test on the record's touching) runs once a
  tick and asks nothing;
- the record's flag agrees with `pilot_flag` except on the 8 listed rows, and only in that direction;
- the first touchdown is 1816;
- the generator is untouched;
- every pad word is used.

`./kit check -- -p game-sim --test kiowa_capture --test kiowa`: 13 tests passed.

## Not verified / not 1:1

- The rotor's own contact search (rotor collision object +0x458 against the collision node, and the
  struck-mask test) is not ported. `Grounded::strike` says `struck: false`. The capture never brings
  the rotor near a tree or prop, so the replay can't tell. Filed as C17.
- free09's UTILITYSHACK1.col is a hierarchy file, which the port doesn't build, so the test's world
  lacks it. The replay matches without it, but where it stands was not looked up.
- The MinWreckSpeed test runs with `touching` true on 275 ticks. The speed never goes over
  MinWreckSpeed, so a wreck from that test is not exercised. `ignored()` (struck surface vs. the
  crash mask) is always false here because struck stays empty.
- The touchdown's `rand()` draw is proved absent only for this image (0 clips). A Kiowa whose
  touchdown sound has clips would draw, and that path is proved only by `tests/kiowa.rs`'s generated
  cases.
- The 8 cleared-flag rows are explained by the read point, not observed directly.
- The capture holds no record fields beyond `pilot_flag`, so the contacts themselves (points,
  normals, count) are proved only through the body they move.

## Next

Nothing for C14a. C17 is the rotor strike.
