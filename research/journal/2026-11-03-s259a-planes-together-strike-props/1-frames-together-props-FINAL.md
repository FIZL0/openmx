# S259a — planes flown together strike the seats' shared props

2026-11-03. Task: `./kit plan S259a`.

**What was missing.** With two planes or more up (N22a1), split screen (`World::fly_held`,
`Garage::ready_plane`) and online (`net/rides.rs` `fly_held`, `vehicle::ready_lane`) ran
`sim.traffic.frame` and `flight::frames_together`. During the tick every seat's traffic holds an
empty set (`Shared::take`), and `frames_together` had no props path, so those planes struck no prop.
S259 had given only the lone flying seat the shared set.

**The change.**
- `flight::frames_together_props` (`frames_together` is it with `None`): `Flight::frame_props`' steps
  for the fleet. Each plane's search (`search_together`) has the knockable props after the other
  planes' hulls (`Scene::knocking`), their records after the other planes' records. Entries on a
  prop name `KNOCK_FROM + k`. After every search and the bikes' mirrors, plane by plane,
  `Flight::knocked` wakes the struck props, files the mirrors (naming the plane by its seat) and
  runs the props' rays on its hull. The struck bodies are one list for the fleet: `knocked` now adds
  to a list it is handed, and a body another plane already struck keeps its place, so two planes
  push one copy. `Together::force` puts the bodies after the other planes among each plane's sides.
  The bodies are written back after the step (`PropSet::pushed`).
- `Shared::set_mut`, and `Shared::traffic`, the ready step's traffic: lend (frame_held, the set's
  collide pass if no seat ran it yet), back, `frame_late`. Same as `traffic.frame` on the empty set,
  plus the collide pass.
- `ready_plane` / `ready_lane` take the `Shared`. `vehicle::fly_together` takes the set and each
  plane's seat. A lone readied plane in the group flies `frame_props`.
- The set's step already took the together planes as strikers (`Shared::finish` from `Sim::plane`
  online, `finish_flying` from the garages in split screen).

**Checked.** New test `crates/openmx/tests/plane_seat_props.rs`
`planes_flown_together_knock_the_shared_ball`. Seat 1 flies S241's hit0-11042 plane at free06's
ball 0. Seat 0 flies the same plane 2000 ft off. The test readies both (`ready_lane`, walk order 1
then 0), flies them in one `fly_together`, settles them and steps the set. Over 12 ticks the ball
(both seats) and seat 1's plane match the lone `fly_lane_met` bit for bit, and the ball wakes on
tick 3. With `None` given to `fly_together` (the old behaviour) it fails at tick 3 because the ball
does not wake. `./kit check`: game-sim plane_meets_plane + prop_plane + prop_strike_paths, 13 passed;
openmx netplay + the bin + plane_seat_props, 307 passed.

## Not verified / not 1:1
- Two planes striking the same prop in one frame: the code shares the body, but no test or capture
  covers it. No capture of the original with two planes among props exists.
- The props' rays on the hulls run once per plane (`prop_wreck::struck_on` per hull), not in one pass
  over every hull. A prop ray that meets both hulls files on both. The original's one search would
  keep the nearer. Untested; it needs two hulls within one prop's reach.
- The mirrors' order on a prop two planes strike: `PropSet::strike` puts each striker's mirrors at
  the head, so the later plane's go first. Bike seats share this behaviour. Not checked against the
  original.
- `frame_late` (HasPhysics cars) still runs before the planes fly together, as it did before this
  change. A lone plane among props runs it after its frame. Left as N22a1 had it.
- Not looked at on screen.
