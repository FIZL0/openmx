# N24: two players' planes meet, on a tape

N22a1 proved two planes meeting on S93's split-screen RAM image with a poked velocity. N24 does it on a
two-pad PCSX2 tape: both players fly planes in the split free09 session (dt 1/60, one field a tick), and
P1's plane runs into P2's. The checks are in `crates/game-sim/tests/c16_fly.rs` (the N24 section).

Everything under context/ is git-ignored. It was made in `openmx-slots/s1`, on kit copy 1.

## The tapes

Both are written by `make_tape2.py` (C16's make_tape.py with P2's pad from its own segments; `/` splits
P1's segments from P2's). P2's switch to the plane needs one more field than C16's P1 needed: the menu
showed "Switch To Machine: Bike", so Right cycles it to Biplane first.

    NAV="60 8:start 60 6:down 40 6:right 40 6:cross 180 6:cross 200"

- **meet.p2m2** — on C16's slot 5 state (`fly.p2m2_SaveState.p2s`, frame 137402: P1 already in plane 0,
  P2 on its bike). 1232 fields.

      python3 -B research/n24/make_tape2.py context/recordings/fly_and_collide.p2m2 \
          context/recordings/n24/meet.p2m2 600 90:cross 510 / $NAV 20 20:cross 580

  P2 switches to plane 1 (its bike leaves the world) and creeps forward at about 5.5 ft/s on a tap of
  throttle. P1 holds Cross for 90 fields, taxis plane 0 at 32 ft/s into plane 1's tail, and pushes it.
  md5 8d196c941ef3fa9ef995704d2b6decf5, state 3b531200594f6dd09ae449eaf77c1009.
- **order.p2m2** — on C16's slot 7 state (the cheat flag set, both players on bikes). P2 switches first,
  P1 600 fields later: `... order.p2m2 600 $NAV 300 / $NAV`.

`try2.sh PID EVERY TAPE segs / segs` writes a tape, plays it (research/c14/play.sh) and logs both
planes live (`log2.py`). That is how the segments were tuned.

## The capture and the images

- `make_spec.py` writes `watch/n24-meet.toml`: C16's spec head, plane 0's block, and the same block for
  the actor list's second entry with `plane1_` names. `play_capture.sh` records it (S228's, with the
  first-row check on plane 0): `context/capture/n24/meet.csv`, frames 137403 on.
- `research/c10/play_at.sh` saves a state on chosen ticks; `rams.py` turns copy 1's slots into
  `context/states/n24/n24-<frame>.ram`. Frames: 137410, 138000, 138150, 138200, 138210, 138213..138221
  (odd), 138225, 138230, 138260, 138300, 138400, 138500, 138600. 137410 is before P2 switches: unused.
  `n24-order.ram` is order.p2m2 at 137750, after both switched.
- `events.py [FROM TO]` prints both planes per row: speeds, distance, crash words.

## What happened

- Row 138217: the first touch (plane 0 at 32.5 ft/s, plane 1 at 5.6; centres 16 ft apart). Rows lag
  a tick here, so it is the step of tick 138216.
- Row 138221: the hard hit. Plane 1 jumps from 6.6 to 17.3 ft/s, plane 0 drops from 32 to 21.
- After: the two stay together (centres 12-16 ft apart through 138620), plane 0 pushing and both
  turning; from 138540 they shove each other up and sideways again.
- It is a ground taxi, not a meeting in the air. No crash on either side.

## Results

- **Seat 0's plane is the node's plane 0.** On n24-138200 (P1 switched first) and n24-order (P2 switched
  first), the actor list's first plane is P1's and the level node's array holds its object first. The
  node is not at S88's address in this heap; the test finds it by its array.
- **The original flies seat 0's plane first.** From the images, the port flies both planes together
  (`frames_together`, a Met each). Ticks held of 100, each plane:

  | image  | P1's plane first | P2's plane first |
  |--------|------------------|------------------|
  | 138200 | 100, 100         | 19, 19           |
  | 138213 | 100, 100         | 6, 6             |
  | 138219 | 100, 100         | 0, 0             |
  | 138500 | 100, 100         | 16, 16           |

  138260..138400 hold both ways: there the order makes no difference to the bits. The port's shell
  flies the last flying seat first, the wrong way round: N28.
- **The post's refill against the other plane's hull** holds through the touch and the hit in seat order.
- Rows 138212 and 138213 are read inside frames (between two port states on every axis). The rows lag a
  tick after them. 138000, 138150 and 138225 part on torn rows in both orders alike.
