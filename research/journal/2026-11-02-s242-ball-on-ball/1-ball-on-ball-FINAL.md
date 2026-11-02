# 1 — ball on ball (FINAL)

Task S242. 2026-11-02.

## What I did

- Capture (PCSX2 copy 3, research/s242 scripts, states in s3's `context/states/s242-drop2-*` and
  `context/s242/drop2-*.p2s`): `slots.py put s197b-hit2-2828 2`, `put_ball.py PID 2 1 0 16 1` puts ball
  1, awake, 16 ft over ball 0 at rest (frame 2850). Then c6's `resume_if_paused.py` and `strike.py
  drop2 1 1 0 --frames 40 --at 2868,...,2873`, which records watch/s197b-wreck.toml
  (`context/capture/s242/drop2.csv`) and saves the states. drop1 was the same run with states taken
  too early (2862..2867); its rows match drop2's.
- Found (oracle S242 block; `lists.py`, `probe.py`):
  - An awake ball's probe searches every category. A ball's mesh collidable is 0x18 both awake and
    at rest, so the rays meet a resting ball too.
  - The probe ignores only its own mesh.
  - At 2869 ball 1's rays strike ball 0 at rest. Ball 0 is woken, filed at the end of the node's list
    and searched in the same pass, and its rays strike ball 1 back.
  - A mirror goes at the head of a struck list that is later in node order (the node cleared every
    list before searching) and after the struck ball's own entries when it is earlier.
- `prop_world::Object::filed_at`: the object's place in the node's list (hashed, on the wire, reset on
  unfile). `PropSet::stamp_filed` gives each newly filed object the next place, in collection order,
  and `PropSet::node_order` lists the filed objects in that order.
- `Collection::search_object`: `Collection::search`'s body for one object, so a ball filed by a strike
  can be searched on its own.
- `prop_props::strike_props` runs after the collections' own searches in `PropSet::search` and
  `search_in`. It walks the filed objects in node order:
  - Each striker's probe searches every other collection's props with a mesh, collision object and
    collision enabled, wherever their node stands, using the striker's own category list.
  - A struck ray gives a kind-0 entry on the striker naming `PROP_OTHER + flat` and sets its flags.
    Its collection is activated, as its listeners would.
  - The struck prop goes through `PropSet::struck`. If it was not filed, it is stamped, searched
    through the `own` closure (in `search_in`, against its collection's set, or a set near it) and
    walked later in the same pass.
  - The mirrors go at the head (after earlier head mirrors) or the tail, as above, and the struck
    prop's flags are set.
- `prop_props::step_named` replaces the plain step in `PropSet::step` and `step_with`. A collection
  whose lists name a prop gets each named prop as a `Pushed` copy after its strikers. The entries are
  renamed for the step and named back after, and the copies are written back (`PropSet::pushed`). The
  step order is still the order the collections were made.
- tests/prop_props.rs (states + disc + kit):
  - `a_ball_strikes_a_ball`: drop2-2868..2872, each tick from its state. The node's ball order, both
    balls' lists (kind, from, on_body, normal, point, surface, the ball named), their flags, and both
    bodies after the step, all bit for bit.
  - `a_ball_on_a_ball_runs_on`: 2868 seeded once and run to 2873, both bodies bit for bit each tick.
  - `flat_places_round_trip`.
- tests/prop_knock.rs `the_knocked_balls_fly_land_sleep_and_go_back` went red at first: the port's
  ball 2 rolled into the port's ball 4, still at rest because the bike that knocked it at 3066 is not
  in that replay. The replay now sets the uncompared balls from the capture's rows each tick
  (`follow`: the body lanes the csv has, and the node).
  - Ball 2 then agrees bit for bit to 3273. The game's 3093 "struck again" was ball 4's rays on ball 2
    and ball 2's on ball 4.
  - It parts at 3274: the game puts the collection to sleep a tick after the body, the port on the
    same tick.
  - The original's node walk removes a finished listener by moving the list's last listener into its
    slot (node_listener_remove), on one list shared by every free09 prop. When ball 1's settle came
    off at 3010, ball 2's settle moved ahead of its object's listener. The port keeps an ordered list
    per collection. The test now asserts the 3273 agreement and the 3274 parting.
- tests/prop_set.rs: `a_woken_prop_ticks_with_the_ride_and_rolls_back` compares a ride's collection to
  one ticked alone, and now copies the set's `filed_at` across.
- `./kit check -- -p game-sim`: TESTS PASSED. `cargo build --release --workspace` builds. `./kit audit`:
  two leaks in plan/DONE.md, on other tasks' lines (0x00256e50, 0x00493210), not from this task.

## Not verified / not 1:1

- A body of the striker's own collection is not a target (each free09 ball is a collection of its
  own; no capture of a multi-body collection's bodies meeting). Filed as a follow-up.
- Filing order: objects filed outside the pass (a bike strike, a wake from the shell) get their place
  at the next search, in collection order, not in the order of their wakes.
- One narrow pass: the original orders a scene mesh and a prop in one search and keeps one contact per
  ray across both. The port puts a prop's entries after its scene's, and one-per-ray only among props.
  In drop2 the balls' lists hold no scene meshes.
- The gather is every prop with a mesh, measured from the rays' centre, as S197's. The original's
  moving and static trees are not modelled.
- The step order is the collections' made order. drop2 and knock agree with it, but the original steps
  each collection's world after the node's in the node's child order, which these captures do not
  tell apart.
- The record's `struck` (the last object the world rays struck) is not written for a prop hit.
- The node's listener list is per collection in the port and shared with swap-removal in the original
  (above). Filed as a follow-up.
- The wreck bike (S197b) and the bikes still go through their own passes. A prop's rays on the wreck
  bike and on another prop are separate searches, so one-per-ray and the order between them are not
  the original's.
