# 2 — the props in the ride (FINAL)

Second session of S131, after part 1 (the node box from the model). The task is split here: what is below
lands, the contacts are S197 and S198.

## What I did

- **`crates/game-sim/src/prop_set.rs`** (new): `PropSet` — every collection the `.toy` makes, as S130's
  `prop_world::Collection`, in the order the loader makes them. `PropSet::load` is the ride's loader:
  `props::claim` over the scene's models, each member's box off its `.psg` (`NodeBox::from_model`; a
  collection with an unreadable box is left out whole and said), `props::bodies`, then `Collection::new`
  per collection with every body asleep where its model stands; what it cannot make is said one line each.
  The edge object is the terrain's extent from the track's `Area`. `frame(dt)` ticks every collection,
  `wake(c, o)` is what a strike will call (activate + enable), `nodes()` hands each body's scene model and
  node to the picture. The ragdoll tables and the body → model map are shared `Arc` constants.
- **`Traffic::props`**: stepped in `Traffic::frame` right after the animated toys, before the cars' early
  return, so every seat's `Sim` ticks them once a tick.
- **Hash**: `Sim`'s walk lists `traffic.props.collections` when there are any (`State` for `Collection`,
  `Object`, `Joint`: what a tick writes; the file's constants are not walked). An empty set walks nothing,
  so every state without props hashes as before.
- **Snapshot**: a `S131prop` tagged section behind the toys' (`wire!` for Collection, Object, Joint, Edge,
  Listener). Decode refuses a state whose collections' shapes differ from this machine's, any index past a
  collection's bodies, and a state without props on a machine that has them. **Fixed on the way**: decode
  took any tagged section straight after a one-player state for the other players' list (only the markers'
  tag was recognised); the props' tag was read as a list length. It now recognises every section tag
  (cars and toys had the same latent problem in a one-player state without markers).
- **Shell** (`openmx/src/main.rs`): loads the props next to the cars (not in a world's tiles, which have no
  cars either), `props: 62 collections, 62 bodies` on free09; maps each body to the picture's scene model by
  name and place (the two scene readers' numbers may part by a rounding, so within 0.01 ft), and each frame
  draws a body off its rest node at its node (pos = origin, look = row 2, up = row 1), one put back where the
  scene stands it — in the software path through `look.models`, on the GPU through the new
  `Renderer::move_scene_prop`.

## How it was proved

`crates/game-sim/tests/prop_set.rs`:

- `free09s_props_load_into_the_ride_as_the_original_made_them`: the ride's loader on the disc's free09 files
  against the first_recording state's 62 collections: world numbers, active flag, ResetTime and countdown,
  every object's gravity, place, flags, ResetTime, countdown, six thresholds, **the edge object's x/z bounds
  bit for bit** (the terrain's 6912), and every body's sleep flag, inverse mass, damping, inverse inertia,
  angular damping and position bit for bit; the turn by value (S80's ulp drift).
- `a_woken_prop_ticks_with_the_ride_and_rolls_back`: a pinned sign in a `Sim` on `Flat`: at rest nothing
  changes; woken and spun, the ride's collection equals the collection ticked alone every tick for 200
  ticks (S130's tick is the one proved against the original), the hash moves while it swings, it sleeps,
  counts down and is put back; a snapshot at tick 20, restored by copy and decoded from bytes, carries on
  to the same hashes; a machine without props refuses the bytes.
- The shell on free09 (`--headless --frames 30`): all 62 bodies found their picture model.
- `tests/hash.rs`'s pinned `Sim` size moved 11544 → 11584 (the `PropSet`); its leaf count did not.
- Gate: `./kit check --all` green but for load flakes at load ~35 (lobby LAN, and
  `smooth_picture::a_shootoff_and_its_put_back_draw_once`, which passed alone). Finished with
  `--skip-audit`: the audit's 7 leaks are other tasks' addresses in plan/DONE.md, not this task's.

## Not verified / not 1:1

- **Nothing in a ride wakes a prop yet**: the f.col collidables and the struck listener are S197, so the
  bike still passes through every prop, as before this task. The drawing of a moved prop was therefore
  never seen on screen; only the at-rest path (nothing moves) ran in the shell.
- A prop's own collision (m.col rays, sweep) is S198: a woken loose prop would fall through the ground.
- **Tick order**: the props tick after the animated toys and before the path cars. Where the original's
  collection nodes update relative to the cars, the toys and the riders in its frame was not traced.
- The picture draws the last tick's pose; it is not blended between ticks as the cars are.
- The edge from the track's `Area` is checked only on free09 (6912 on both axes); the edge's y lanes and
  the other tracks' edge objects were not compared.
- A mod track's `.pco` falls back to a search by file name; no mod with props was tried.
- The sign in the tick test is a synthetic `.pco`, not the disc's (the disc's sign is S130's test).
- The `Ini` reader is the port's; a `.toy` with keys it reads differently was not looked for.

## Follow-ups

- S197 props struck (f.col collidables, struck listener wake, two-sided push).
- S198 a prop's own collision object.
- S195 (existing) a captured knock to replay against.
