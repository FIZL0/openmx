# 2 — wheels stand on a woken truck (FINAL)

Task S208. 2026-11-02. Iteration 1 proved the woken truck's `f.col` is in the wheel's reach and blocked on
S186; S186/S186a have landed the woken truck's rigid body, so this wires it.

## What I did

- **New `game_sim::truck_wheel`** (`crates/game-sim/src/truck_wheel.rs`): the riding bike's wheel probe
  against each physical truck's `f.col` at its body's pose (`PhysicsCar::pose`), on the same
  `ray_probe::Record` the scene's and the props' (S210) searches filled, the nearer contact kept a
  segment. A truck contact sets the wheel's owner word to `truck_wheel::handle(k)` (`OWNER | k`, a bit no
  prop handle reaches) and no vehicle; `truck_wheel::under` answers the body's velocity and its velocity
  at the point (`Collection::under`, `contact::other_at`'s body side, S169).
- **The path mesh stands down for the wheel's search** while the car is physical, as the wake does
  (`Trucks::stand_down` → `traffic::hide`, shown again right after), so a wheel never takes both.
- **Lending.** `prop_wheel::Lent` now carries the trucks too (`Lent(PropSet, Trucks)`): `Sim::lend_props`
  takes `Traffic::physics` for the bike's frame when any car is physical, `props_back` returns it. This
  keeps the `Ride` (2472) and `Sim` (12096) size pins and needs no new plumbing at the call sites.
- `ride.rs`: `wheel_hits` splits the lent set, stands the meshes down and calls `wheel_hits_in`, which
  runs the props' search then the trucks' on the record (`on_after` drops a prop entry a truck contact
  replaced in place). `under`, `contacts` and `share_load` take the `Lent`; doc comments updated.
- `traffic.rs`: the module doc's last sentence now says the wheels stand on the f.col (S208) and the body
  rays / sweep do not yet (S251).
- New test `crates/game-sim/tests/truck_wheel.rs` `a_wheel_stands_on_a_woken_truck`.
- Nothing taken from opentennis: it has no counterpart.

## What the original does

From iteration 1 (interpreter on s89-along) and S168/S169: the wake sets the `f.col`'s nearby bit (category
`0x18`), stands the path mesh (`+0x428`) down, and the mesh's collision object names the physics body
(`+0xf4`) and no chassis. So a wheel on a woken truck is an ordinary record contact whose owner word is the
truck's object, read through the contact-velocity step's owner branch on a dynamics body. The `f.col`
has no struck listener of the car's and its object's listener is off: a wheel on it wakes nothing.

## How it was proved

- `a_wheel_stands_on_a_woken_truck` (disc-driven, free06 terrain, free06's ctruck off the disc with
  race125p's bike): strikes the truck, lets it slide to rest (body off, still physical), drops the bike
  over the roof and checks (1) owners are `handle(0)` or 0 and a truck owner names no vehicle; (2) with
  the body off, poking its velocity to 0 or (0,0,8) leaves the truck's pose the same but changes the
  bike's chassis velocity — the wheel reads the body; (3) never struck, a wheel over the truck stands on
  its path mesh (vehicle set, owner 0) and nothing is lent; (4) after the frame the physics objects are
  back in the traffic and the path mesh is stood up again.
- Mutants, each red with the real code green: no stand-down; `truck_wheel::under` → `None`; no truck
  search in `wheel_hits_in`.
- `cargo test --release -p game-sim` for prop_strike_paths, prop_knock, physics_car_struck, traffic,
  hash, snapshot, bike_col, prop_under, physics_cars, moving_vehicles, truck_wheel: all green.
- `tests/replay.rs` `a_path_car_into_a_parked_bike a_bike_meets_a_moving_truck` (S207/S219 captures and
  RAM images copied from s2/s3 into this slot's `context/`): 2 passed, all 8 rides ran, unchanged from
  S186a — ctruck parts on 36, no wreck; rear parts on 42 and wrecks (42,4) as the original; truck (105,2),
  flank (20,6), rails (98,1); slow / beside / trailer none.
- `cargo check` of game-sim, game-render and openmx `--tests --bins`: clean (pre-existing warnings only).

## Not verified / not 1:1

- **No capture of a wheel on a woken truck.** The S207/S219 bikes are thrown off the truck and part on the
  strike tick's push (S251), so the proof is the port's own test plus iteration 1's RAM proof. A PCSX2
  state of a bike landing on a struck, resting truck would make a replay check.
- **When the bit drops.** The port takes the `f.col` in reach for as long as `PhysicsCar::physical`;
  whether the original clears the nearby bit while the body lies off before the put-away was not read.
- **Body rays and sweep** still meet the path mesh, not the `f.col`, and do not push the truck: S251.
- **Order inside the record.** Trucks are searched after the scene and the props on the same record; the
  original's order of the bike's nearby list over these was not read. With the one-a-segment nearer rule
  the order only matters for exact ties.
