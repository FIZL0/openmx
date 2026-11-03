# 1 — the blades on the hull (FINAL)

Task C21. 2026-11-03.

## What the capture could not show

C17's capture (`capture/c17/strike.csv`) has the push on body rows 1299..=1302 only:

| row  | last force                    | last moment               |
|------|-------------------------------|---------------------------|
| 1299 | (-19.709, -4112.5, -135.55)   | (186.96, 2065.4, 713.22)  |
| 1300 | (-55.227, -4110.6, 268.38)    | (32.555, 580.64, -1208.1) |
| 1301 | (-283.95, -4044.5, -43.708)   | (926.28, 866.97, -51.479) |
| 1302 | (-133.62, -4078.2, -11.682)   | (189.28, 680.26, -169.66) |

1303..=1309 are gravity alone (0, -4110, 0), and 1310 is the touchdown.

C19's capture (`capture/c19/probe.csv`) cannot show the contacts:

- Its contact columns read `+0x108` as the count and `+0x100` as the array. These are the wrong way
  round: the count is `+0x100`, the capacity `+0x104` and the array's pointer `+0x108`.
  `watch/c19-kiowa-strike.toml` is fixed.
- Its touch flags (`+0x40..+0x4c`) read 0 on every row through 1310. The save state below has them
  set on 1299. Whatever phase PINE read them at, it is not the frame's.

So the evidence is save states.

## The save states

- **How they were taken.** `recordings/c17/strike.p2m2` was played on PCSX2 copy 3 from C14's state,
  using the C10 playback scripts at a slowed `NominalScalar` (0.05). The ini is back to 0.25.
- **Which.** States were saved at frame counters 1297, 1298 and 1299. In the capture's rows those
  are 1298, 1299 and 1300. They are `context/states/c21-{5,6,7}.ee.bin`, in s3 only and not
  committed. Copy 3's save slots 5..7 were its scratch slots.
- **What they hold.** The addresses are in `research/oracle.txt` (C21).
  - The heli's `*(+0x170)` *is* its collision record. Each blade wrapper's `*(+0x120)` is that
    blade's, as C19's spec guessed; only the count and array offsets were wrong.
  - **c21-6 (row 1299, the push).**
    - The heli's record has flags [1, 1, 0, 1] and one entry. The entry is kind 1, a mirror, and
      its other side is blade 0's record.
    - Blade 0's record has one entry. It is kind 0, its other side is the heli's record, it has no
      mover, and the record's `struck` is the heli's collidable (category 0x202058).
    - The normal is (-0.06273, 0.998, -0.009454) on the blade, negated on the mirror. The point is
      (3039, 143.9, 4053).
  - **c21-7 (row 1300).** The heli has two entries, both mirrors of blade 2's two.
  - **In both.**
    - The heli's own rays struck no blade.
    - There is no blade-against-blade entry.
    - The blades' collidables are category 0x18.
  - **Who shares the world.** The heli and the four blades are in one dynamics world, the heli
    first on every list (S220a1a).
- **The hull's mesh.** It is `helicopter\col\kiowaf.col` in `machine.res` (900 bytes, beside
  `kiowam.col`).

## The port

- **`prop_world::Collection::search_among_hulled`.** It is S243's `search_among` with the hull's
  collidable category and collides-with as an argument; the wreck bike's were hard-coded.
  `search_object_wrecked` keeps its signature and passes the wreck bike's.
- **`prop_world::Collection::{listed, lists}`.** They give the collection's list counts, and its
  lists as a `Dynamics` with given others, for a world the collection shares.
- **`kiowa::Blades::search_on`.** The blades search with the hull as a `Wreck`:
  - its plane is `kiowaf.col` at the heli record's node, which `Collision::search` set this frame,
    moving with the body's velocity;
  - its collidable is the Kiowa's (`COLLIDABLE`, `COLLIDES_WITH`);
  - its side is 0.

  Each mirror is renamed to name its blade, and `Collision::file_mirrors` puts them on the heli's
  record after its own entries, up to the capacity. It sets touching, on-object and
  touching-other, as `flight.rs` does for the plane's hull.
- **`kiowa::Surroundings::among` / `among_listed`.** Both are new, and their defaults are none / 0,
  so every other `Surroundings` is unchanged.
  - `Kiowa::tick` now steps a `Shared` dynamics: the heli at place 0 of every list, the blades after
    it.
  - The blades' posts are left off it. They still run in `Blades::post` after `Wreckage::act`, as
    before.
  - A blade's resolve gets the heli as its other side 0, through a `Weighed` on `ride.body` with
    `Collision::mass`. So the kind-0 impulse moves both bodies, in the same substep.
- **`Grounded::resolve_among` and `Blades::with_others`.** The heli's resolve takes the blades as
  others, by blade index, for its mirrors' springs.
- **What no longer runs.** `Blades::step` (`step_to_posts`) is no longer called by the test or the
  shell: the blades step in the heli's world. It stays for anyone else.
- **The shell.** `KiowaRide::step` does the same, through `KiowaAround`. `KiowaRide::with_hull`
  loads `kiowaf.col`, and `main.rs` reads it after the blades.

## The torn rows from 1373

With the port above, every row through 1372 is bit for bit. From 1373 the original's body rows
differ in two ways:

- the force and moment accumulators (`+0x90`, `+0x190`) are non-zero, where an after-the-frame read
  has them cleared;
- some rows are one, two or three substeps ahead in position and velocity. For example, 1396's
  `last_position` is the port's `position`.

It is not a divergence. The rows in between (1397, 1399, 1402, …) are bit for bit after their frame.

`compare_torn` checks such rows. It takes the port's body after the row's frame, or the body before
and after each substep response of the *next* frame (recorded in `Struck::resolve`), and accepts the
first that equals the row's every quadword:

| place | rows |
|---|---|
| after the frame | 144 (25 of them inside the frame's post, below) |
| in the next frame's substeps | 60 |

That is 204 rows in all, every row from 1299 to 1502, the capture's last.

On 34 of them only the four accumulators (force, moment, last force, last moment) are off. Those are
reads part-way through a response, between two contacts. The rows' integrated state (position,
velocity, orientation, rates, accelerations, last position and orientation, shock, twist) still
equals a substep bit for bit, so those four are left out on those rows only.

The push rows 1299..=1302 must match whole, after their own frame. They do.

## The put-back on 1449, and after it

C19 compared only where the put-back stood the body (within 0.01 ft), because the original steps
the placed body once more that frame. Compared whole, every row from 1449 to 1501 was off, and the
port's 1450 controller (collective 0.29196334, hold altitude 132.64577) was the game's 1449.

**The port now runs the frame a second time after the put-back** (the test's loop, and the shell's
`KiowaRide::step`):

1. `put_back_controls` and the collision refresh;
2. the pad as that frame's input step read it;
3. the collide pass (`Collision::search`) and `Kiowa::tick` again. The put-back stopped every blade,
   so none is in it.

With that, 1449..=1473 are bit for bit. The pad matters:

- if the second step flies the put-back's zeroed `Pad::default()`, row 1449's last longitudinal is
  `0` in the port and `-0` in the game;
- the polled pad's stick is `-0`.

The second step's crash test asks nothing, and it draws nothing from the generator.

### Rows read inside their frame's post

From 1474 the measure's figures differ while the body equals the port's after its frame.

- **From 1482.** Every figure (speed, alpha, beta, the rates, the body velocity, the rows, the Euler
  angles) is the port's from the frame before.
- **1474..=1477.** Some figures are the frame's own and some are the frame before's.
  - 1474: speed through beta are new; the Euler angles' `y` and `z` are new and `x` old; the forward
    and up rows are old.
  - 1475 and 1476 cut earlier.

It cannot be a measure that really lags. The controller reads the measure the last post left, so a
lag would change the forces and the body, and the body matches.

**Reading.** PINE read these rows after the frame's world step and before, or part-way through, its
post's `Measure::flight`. `flight` writes:

1. the speed;
2. the body velocity;
3. the world rates;
4. alpha;
5. beta;
6. the Euler angles one lane at a time, in `quat_to_euler`'s order (`y`, `z`, `x`);
7. the matrix, with the forward and up rows.

**The test's check.** `compare_torn` takes the most of these nine groups, in that order, that are the
frame's own with the rest from the measure the frame started with, and requires the result to equal
the row's figures bit for bit. Such a row's body must be the one after its frame, whole.

25 rows are read this way (1474..=1477, and 1482 on).

## How it was checked

- `./kit check -- -p game-sim --test kiowa_capture a_captured_rotor_strike`: TESTS PASSED, every
  row 1299..=1502. It prints the place counts above, and "34 part-way through a response, 25 in
  their frame's post".
- Without the second step after the put-back (a soft-compare run, since removed), every row from
  1449 to 1501 differs. With the second step on the zeroed pad, 1449 differs on the last
  longitudinal's sign.
- The negative control (no `with_hull`) fails on 1299: the speed is 4.1156425 in the game and
  4.1125774 in the port.
- `./kit check -- -p game-sim --test kiowa_capture --test kiowa --test kiowa_phy --test prop_wreck
  --test ragdoll_bike`: 33 passed.
- `--test ragdoll_wrecks --test atv_search`: 19 passed. All seven together after the put-back
  change: 52 passed.
- `-p game-sim --lib`: 38 passed.
- `-p openmx --test kiowa_switch --test kiowa_particles --test kiowa_pilot_seat --test
  kiowa_picture --test bays`: 53 passed.

## Not verified / not 1:1

- **The heli's rays on the blades.** The blades' collidables (`Blades::knocks`) are still not in the
  heli's own search. The two states show none, but other frames, and other crashes, may have them.
- **Blade against blade.** No blade's search offers the other blades' collidables. There are none
  in the two states.
- **The capacity cut-off.** Mirrors are cut at the heli record's capacity (`kiowam.col`'s ray count).
  This is assumed from the plane's hull (`flight.rs`) and never reached here.
- **The order on the record.** Mirrors after the heli's own entries is inferred: the heli searches
  first, and its clear would otherwise wipe them. On 1299..=1302 the heli had no entries of its
  own, so the order is unexercised.
- **The 34 part-way rows.** Their four accumulators are not compared. The other 116 rows are whole.
- **The read phase of the touch flags** in C19's capture is not explained.
- **The hull's place and velocity.** The hull is placed at the record node `Collision::search` set,
  which is `node_of(body)` at the frame's start, and moves with the body's velocity. Only the rows'
  agreement backs this; the collidable's own fields were not read.
- **Other liveries.** Only `kiowaf.col` was checked; `kiowa2f`, `kiowa_ssf` and `kiowa2_ssf` were not.
- **The shell has no capture test.** Its change mirrors the test's wiring, and the shell's Kiowa
  tests pass. `kiowa_switch` wrecks the shell's helicopter and puts it back through the new second
  step.
- **A blade's `enabled` in the shared step.** `Shared::enabled` answers true for a blade and checks
  the flag inside `integrate`/`end_substep`, because the blades are reached through `&mut`. The
  outcome is the same as the world's loop checking it.
- **The second step after the put-back.** It is a whole frame run again: the collide pass, the
  world step and the post. The rows show the controller and the body stepped once more on the polled
  pad. Whether the original's second step also runs its collide pass, or only its world step, is not
  told apart: they agree here.
- **The 25 rows read in their post.** These are accepted on a cut of the measure's writes, which can
  hold the measure wholly as the frame started (20 rows). Their body is checked whole. That PINE
  reads in the post is the reading above, not seen.
