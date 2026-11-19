# 1 — shared and fleet plane struck prop searches (FINAL)

Task B34a. 2026-11-18.

## What I did

- B34 made a ball the lone plane wakes search in the props' pass. This does the same for the two
  plane lanes B34 left heard only (B28).
- Shared-seat lane (openmx `Plane::step_traffic_shared`): flies `Flight::frame_props_in` with the
  set's pass movers (`Shared::movers_mut`, now public) once a seat has run the pass; before any
  seat has, `frame_props` as before.
- Planes flown together: new `flight::frames_together_props_in(…, movers)`;
  `frames_together_props` calls it with `None`. Its strike step (`knocked`, `shared`) now puts the
  strike on with `PropSet::strike_in` when given the movers. The fleet keeps its MD254a seam rule:
  the strike's seam shifts are held back from `strike_in` (so it does not move the body for the
  plane's frame) and noted afterwards as before (`pushed: true`).
- openmx: `Shared::set_in` gives the set and its movers together; new `vehicle::fly_together_in`
  (the one-plane case flies `frame_props_in` too); `fly_together` calls it with `None`. The two
  shell callers (main.rs split screen, net/rides.rs online) use `fly_together_in`. These two files
  were not on the task's file list; each is a five-line change at the call.
- Nothing from ../opentennis: it has no planes striking props.

## What the original does

As B34's journal (2026-11-17-b34-…): the node walk re-reads its filed count, so a ball the plane
wakes is searched in the same walk, behind the plane's mirror. The original has one plane; two
planes and shared seats are the port's own lanes, so "as the original" here means "as the one plane
B34 proved".

## How it was proved

- B34's images, still in the shared context: `context/states/s241-b34at-11065` (ball 0 asleep) and
  `-11066` (the strike tick). No new capture needed.
- game-sim `tests/prop_plane.rs a_ball_planes_flown_together_wake_searches_in_its_strike_tick`:
  the plane from 11065 and a copy 2000 ft off, flown by `frames_together_props_in` after the props'
  pass. Ball 0's list against 11066's: same length (2), every searched field bit for bit, the mirror
  naming the near plane (striker 0). With movers `None` the same run lacks the ground entry (checked
  in the test).
- openmx `tests/plane_seat_props.rs`:
  - `a_ball_a_plane_seat_wakes_searches_in_its_strike_tick` (seat lane, bike far off) and
  - `a_ball_planes_flown_together_wake_searches_in_their_strike_tick` (`fly_together_in`, far plane);
  - both from 11065, against the lone seat (`fly_lane_met`, whose strike-tick list B34 proved
    against 11066): ball 0's whole list (every field, `{:?}`) and body bit for bit for 6 ticks, in
    both seats. The mirror names seat 1 there, striker 0 alone; the test maps that one name.
  - With the two lanes switched back to hearing, both fail at tick 1.
- `./kit check -- -p game-sim --test prop_plane --test flight --test wrap_moving --test
  plane_meets_plane --test knock_launch`: 35 passed. `-p openmx --test plane_seat_props`: 21 passed.

## Not verified / not 1:1

- No capture of two planes or shared seats: the original has neither. Proved only against B34's
  one-plane image and against the lone lane.
- Only one strike (free06, ball 0, one aim) is checked, as in B34.
- A fleet where two planes strike the same sleeping prop in one tick is not tested: the second
  plane's strike finds it filed and does not search it again. That is my reading of the walk (filed
  once, searched once), not proved.
- A fleet strike across a wrapped world's seam with movers (the seam shifts held back from
  `strike_in`) has no test of its own; wrap_moving's fleet test still runs the heard path
  (`frames_together_props`, no movers) and passes.
- The game-sim fleet test compares ball 0's list right after the planes' frame, not after the props'
  own step (no `Sim` runs that lane). The openmx tests run the full step.
- The ball's body vs the original: still off as in B34 (the plane's full state is not seeded),
  not chased here.

## Next

Done.
