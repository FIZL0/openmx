# 1 — ghost through traffic (FINAL)

Task S215. 2026-10-28.

## What I did

- `traffic::Traffic::span` (where the cars' meshes lie in the world's props), `traffic::hide` /
  `traffic::show` and `traffic::Hidden`: for one rider's frame each car's prop keeps its place in the
  list but holds an empty mesh (no tree, no triangles), radius 0 and no motion, and is given back as it
  was at the frame's end. Every search the riding bike makes reads the cars off those props — the body
  rays' scenery search (S138), the car search (S137, also skipped outright for a ghost in
  `Sim::car_contacts`), the wheels, the chassis' sweep, the wreck test's sweeps (S129's hit/throw, the
  nose S170), the wreck bike and the ragdoll's bones (`PathCar::near` skips a prop with no motion) — so
  that one choke point covers all of them.
- `Rider::cars` (not on the wire, not hashed; `snapshot.rs` lists it as skipped; `tests/hash.rs` pins the Sim at 11952), set before every frame
  by `Sim::lend_cars` from `Sim::step`, `step_all` and `step_bumped`. `Rider::frame_bumped` hides the cars
  after the L1+R1 put-back (step 1d) whenever the ghost is on, so the put-back's own tick is a ghost's
  too, and drops the bump list's car entries (the only entries with no other side at that point) the
  rays found before the put-back.
- `ghost::inside_cars`: the ghost's clock-holding overlap against the cars, ORed into `Ghost::step`'s
  `inside` beside the riders' `overlaps`.
- `ray_probe::tree` returns false for a mesh with no nodes (it indexed node 0): the empty car mesh.
- Test: `crates/game-sim/tests/traffic.rs` `a_ghost_on_the_rail_lets_the_train_through`.
- Nothing taken from opentennis: it has no ghost or traffic.

## What the original does

Read from `research/oracle.txt` S63 and S129, no new decompilation:

- The put-back adds the vehicle class word (0x40) to the exclude-mode category lists of the bike's body
  rays (+0x7c4) and sweep (+0x7cc). The pair filter drops a candidate whose category meets a listed
  word; the path cars' mesh collidables are category 0x48 (and so are the two driving toys and the bikes'
  frames), so **the original's ghost already passes through the cars**: its body rays' nearby search
  gathers none, and the wheel probe collides only against that search. Scenery (0x220) is untouched.
- The ghost's clock: `ghost_step` flips the body's list to include mode for one overlap query, which
  then sees vehicles only — 0x48 holds 0x40, so the query sees the cars, and a ghost inside a car holds
  its clock as it does inside a rider.
- So the port follows the original for the riding bike and the clock; what it adds is listed below.

## How it was proved

- `a_ghost_on_the_rail_lets_the_train_through` (s89-along's cars stepped by the port, the disc's free06
  terrain and the disc's `Bike\Col\bike` files): a braked bike stood 40 ft in front of the engine,
  broadside to the rail. Solid, the train shoves it 120 ft (no wreck: the stock bike is pushed, not
  thrown). As a ghost, the whole train (374 ticks) runs through it: no wreck, 0.28 ft of creep, the clock
  held on exactly the 374 ticks a car's box held the bike. Once the train has gone the clock runs out in
  `ceil(left · 30)` ticks; stood in front of the engine again, now solid, it is shoved 114 ft as the
  control was.
- The original-mode train replays still agree as before, run with `GAME_SIM_FIXTURES=<s2>/context`:
  `the_bike_meets_the_dallas_train_as_the_original_does` (braked parts at tick 2 and across at 148, as
  their recorded thresholds allow), `the_parked_bike_never_gathers_the_train`,
  `a_bike_parked_on_the_trains_roof_rides_it_as_the_original_does` (every lane, every tick), and the
  nose tests. In those states the bike is not a ghost, so nothing is hidden.

## Not verified / not 1:1

- **No capture of the original's ghost beside the train.** The pass-through is read off the category
  words (S63/S129), not watched: no state has a respawn beside the rail with the ghost running. A put-back
  next to the train recorded in PCSX2 would prove it.
- **The clock's overlap shape is the port's own**: the bike's `Rival` sphere against each car's root box
  in the mesh's space, where the original's query asks the bike's collidables against the car's mesh.
  It holds a little earlier and later than the original's would at the car's ends and corners; the
  size of the difference is not measured.
- **The original's stale gather is not modelled**: S129 found that a bike which last gathered its nearby
  search as a ghost meets no car until it moves out of the search's bound. The port's cars come back the
  tick the ghost ends. A bike that sits still after its ghost ends is struck by the next train in the
  port, and possibly not in the original.
- **Port additions, where the original differs or is unread**: the wreck bike and the ragdoll's bones
  miss the cars while the ghost runs (the original's lists for them are not the ghost's, so a ghost that
  wrecks during its three seconds would be struck by a train in the original). Asked for by the user.
  The riding ragdoll's tick (0a, off by default) runs before the hiding and still sees the cars.
- **Only the path cars are hidden.** The original's 0x40 filter also drops the animated toys and the
  props in category 0x48 (S129's "two more 0x48 toys"); the port's toys (`toys.rs`) and knocked props
  (`prop_set.rs`) stay solid to a ghost. Their categories were not read for this task.
- **The chord put-back tick**: dropping the car entries the rays found before the put-back is the port's
  reading of the put-back adding the ghost word before the bike's collide; the ordering against the
  original's list was not traced live. The rider-against-rider entries of that tick are left as S63 had
  them.
- **Broad-phase capacity**: a hidden car stays in the props as an empty mesh with reach 0, so a search
  may still list it as a candidate that strikes nothing, where the original's filter drops it. That
  only matters on a search that hits its capacity (0x100), which free06 does not.
- `openmx --trace` steps `Rider::frame` itself and never sets `Rider::cars`: a traced ghost is struck by
  the cars. Debug output only.

## Next

Nothing for this task.
