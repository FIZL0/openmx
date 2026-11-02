# 1 — props struck (FINAL)

Task S197. 2026-11-02.

## What I did

- `PropSet` loads each model's `<model>f.col` (`props::shape_file`, parsed once per file) into
  `shapes`. `frame_among` is split into `search` (the props' own rays, returning the movers) and
  `step(dt, movers, strikers)`; the collection's force listener now resolves its entries against the
  strikers (`Other`s) instead of nothing. `PropSet::struck` wakes as the struck listener does.
- New `prop_contact.rs`: `strike` poses every filed object's shape at `PropSet::matrix(node)`
  (`Prop::posed`, no mesh clone), searches the bike's body rays against them as collidables of the
  props' category, and runs `contact_search::collide` with the bike's record plus one record per prop,
  so the bike gets the kind-0 entry and the prop the kind-1 mirror. `PropSet::strike` wakes each struck
  prop, puts the mirrors on its record and hands its body out as a `Pushed`; `Bump::resolve` chains
  those bodies as `Side::Body` others after the players, and `PropSet::pushed` writes them back.
- `Sim::step`: the props' search runs with the traffic (`Traffic::frame_held`), the strike after the
  car contacts, the bike's frame with the merged contacts, then the props' step with every player's
  vehicle as a striker (`Sim::props_step`). The plane branch steps the props with no strikers.
- tests/prop_knock.rs, on S195's knock states (s3's captures; skips elsewhere):
  - `the_bike_strikes_the_ball` — from knock-2882 the port's strike gives the bike's one entry and
    ball 2's mirror as knock-2883's RAM holds them, and after the push and the props' step ball 2's
    body (positions, velocity, quats, rates, shock, twist), its record flags and the mirror's arms.
  - `the_bike_and_the_ball_push_each_other_as_the_original` — the original's resolve run (hooked
    inside node update) on the same contacts; the port's `Bump::resolve` agrees on the chassis'
    velocity, rates, shock, twist, force and torque and the ball's velocity, rates, shock and twist.
  - `a_strike_files_the_objects_listener_first` — `PropSet::struck` leaves the object's listener ahead
    of the collection's settle on the world node (S195's RAM order test covers the original's side).

## What the original does

The props node's update clears and collides every filed record — the human bike's is filed there ahead
of the balls — then resolves each, so the bike's first-substep resolve runs inside it. The bike's body
rays hitting a ball's f.col file a kind-0 entry on the bike and a kind-1 mirror on the ball; the bike's
resolve pushes both; the ball's force listener resolves the mirror with no impulse. The struck listener
enables the object (its listener goes on) and then activates the collection. Addresses in
research/oracle.txt under S197.

## Not verified / not 1:1

- Only the single-player `Sim::step` bike strikes. `step_all`, `step_among`, `step_bumped` and openmx's
  vehicle loop still call `Traffic::frame` with no strike.
  (Done in S197a: research/journal/2026-11-02-s197a-props-struck-every-path.)
- The rider and ragdoll, the wreck bike, the plane and other props do not strike props.
- A prop's own rays against the bike's f.col (the prop as the searcher) are not ported; the props' step
  sees the bike only through the mirror and the strikers.
- The broad phase is a stand-in: every filed prop is tested, not the original's tree gather.
- The prop search is its own search, separate from the scenery's, so the one-hit-per-ray rule holds per
  search and a ray could hit both a prop and the ground where the original keeps one.
- The wake order is proved by a unit test plus S195's RAM order; no state before the first wake exists.
- The wreck test does not read prop contacts.
- A prop woken by the strike this tick does not search its own rays until the next tick.
- Proved on one strike (one frame, one ball, the bike's first substep); later substeps of a tick and
  several props at once are not compared.
