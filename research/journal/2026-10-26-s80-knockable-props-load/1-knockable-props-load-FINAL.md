# 1 — knockable props load (FINAL)

Task S80. 2026-10-26.

## What I did

S80 as briefed (the props' .col files, the rigid-body step, and their state in the rollback snapshot)
is three tasks. I split it. This entry does the first part: the loader, proved against RAM. The rest
is filed as follow-ups:
- S126: the props' tick (the collection world, the constraints, the lifecycle, the node write-back).
- S127: props against the bike and rider, Sim's hash and the rollback snapshot, and drawing them.
- S128: the part-name/RefName fallback.

- `crates/game-sim/src/props.rs` (new):
  - `entries` reads the `.toy`'s `[PhysicsCollections]`.
  - `Pco::read` reads a `.pco`: the `[PhysicsParts]` header, every part's `Type`/`Name`/`Create` and
    its `phy::Part` (mass properties through the shared reader), and the constraints.
  - `claim` runs the original's create/attach rules over the scene models: which collections exist,
    in what order, which model each member stands on, and the one-body flag.
  - `rays_file`/`shape_file` give the `m.col`/`f.col` names.
  - `frame`/`pose`/`turn` give the node frame and the start quaternion.
  - `box_size` is the node bounds query's box.
  - `bodies` makes each prop's `Body` and gravity through `WreckBike::new`, as the original's
    physics object init does.
- `crates/game-sim/tests/props.rs` (new): free09 from the disc against the first_recording state.
- `research/oracle.txt`: an S80 block with the functions and the layouts.

## What the original does

- **The `.toy` reader** (`./kit fn 0x00289480`):
  - Reads `NumberOfPhysicsCollections`, then each `[CollectionN]`'s `Name` and `Type`.
  - A name with no `*` defaults to Type `single`, and its pco is `<track dir><name>.pco`.
  - A `*` cuts the name: the pco is `<prefix>.pco` and the default is `group`. The create then runs
    on every unclaimed scene model whose name strnicmp-matches the prefix, passing the model's full
    name. Every retail starred entry says `Type = single`.
- **The create** (`./kit fn 0x00289050`):
  - `single`: every unclaimed model whose instance name stricmp-equals the name gets its own
    collection. The model is claimed only if attach returns nonzero; otherwise the collection is
    destroyed. If no model matched, it falls back to the part-name search (`./kit fn 0x003b1c60`),
    which is not ported.
  - `group`: one collection, every match attached, kept if any attached.
- **Attach** (`./kit fn 0x003b1f40`):
  - Reads the pco's `[PhysicsParts]`: `NumberOfPhysicsParts`, `NumberOfConstraints`,
    `NumberOfIterations` (default 1), `SimulationHz` (default 55), `ResetTime` (default -1).
  - A collection that already has bodies takes another one only if it is one-body and the instance
    has the collection's name. One-body means no bodies yet, an instance, one part and no
    constraints.
  - Each part's node is the instance's child named `Name`, or else the instance.
  - Only `RigidBody` parts load.
- **The body** (`./kit fn 0x001595d0`, `./kit fn 0x001597b0`):
  - The place and the look/up come from the node's *world* frame.
  - quat = axes_to_quat(look, up); the body gets its start position and turn, then a reset.
- **Mass properties** (`./kit fn 0x0015a500`): the inertia box comes from node_bounds
  (`./kit fn 0x001b3f50`). That spans the corners centre ∓ half, answers (max−min)/2, and the reader
  doubles it.
  - So a box whose centre is off zero is not exactly 2×half. The egg's centre y is 2.1e-6, and that
    put its inverse inertia z an ulp off until the corner rebuild was ported.
- **The scene reader** (`./kit fn 0x001a8cd0`):
  - Sets the node frame with mat4_basis(orth 1, keep 0) on `LookVector`/`UpVector`, defaults
    (0,0,1,1) and (0,1,0,1).
  - A three-number line keeps the default w of 1. I checked that w makes no difference here.

## How it was proved

`tests/props.rs::free09s_props_are_the_collections_the_original_built` is a save-state capture: the
first_recording state, with free09 loaded and no prop knocked yet. It checks the port's loader on the
disc's `free09.toy`, `free09.scn` and the pcos against all 62 collections the `.toy` actor's list holds.

**Bit for bit, all 62:**
- the order, the name, the pco path, the kind, the iterations, the Hz, the one-body flag and the
  member count;
- the node is the model, by name;
- the place, gravity and body position;
- the start turn from the node's world rows through `props::turn`;
- the inverse mass, mass, damping, inverse inertia and angular damping.

**By value:**
- The body's turn equals its start turn.
- The port's start turn, built from the scene's frame, equals the original's for 61 of the 62 (by
  value, because of signed zeros). SH_CrateC model 529 is 1 ulp off.

The 28 signs each carry a single Pin.

The free09 counts: 5 BeachBall, 1 egg, 2 recycling_can, 2 SH_Barrel, 20 SH_CrateC, 1 trash_can,
2 gas_can, 1 tool_box, and 28 signs.

I checked once by hand that the original's mat4_basis on crate 529's scene line gives the port's
rows (oracle call, scratch test, removed).

All retail `.toy`s were surveyed by a scratch script:
- No `Create = TRUE` anywhere.
- Only RigidBody parts.
- Only Pin and BodyTorsionSpring constraints.
- The only entries that need the part-name fallback are nat07 SnowmanA/B and nat15 Nat02_up_bale_hay.
- The sx `*_bale_single*` prefixes match no model, so they load nothing in the original either.

## Not verified / not 1:1

- **The node box is handed in from RAM.** The port has no scene graph to compute node_bounds from a
  model (`NodeBox` comes from the caller). Children's boxes are not merged, and no free09 prop node
  has a child (the test asserts it). Follow-up: S127.
- **The start turn is matched by value, and one crate is 1 ulp off.** In this state every prop node
  carries negative zeros, and twelve rotated nodes sit an ulp or two off the scene's frame. So the
  original made the bodies off nodes its world had already posed (a second load, presumably), and the
  port, which uses the scene's frame, cannot see that drift. SH_CrateC model 529's start quaternion
  is 1 ulp off in some lane. A props state taken straight after a first load would settle this. Not
  done.
- **The node rows of a body at rest (the post listener) are not ported.** They are neither
  quat_to_mat4 of the body's turn (the egg's rows differ) nor the scene frame. Follow-up: S126.
- **A part's node is always the instance.** The child-by-Name lookup is not ported. For free09 the
  instance is right: every node is the model, by name.
- **Not ported:**
  - the part-name/RefName fallback, which `claim` reports as `Unported::PartNameSearch` (S128);
  - `Create = TRUE` placement, reported as `Unported::Create`, though no retail pco uses it.
- **Constraints are read but not built or solved.**
  - The angles are kept in degrees, and the radian conversion and curve-sine limits are not applied.
  - BodyTorsionSpring's own reader (keys `Spring`/`Damping`, default 0) is assumed, not read from
    the decompilation.
  - None of this is proved: free09 only has Pins, and only the Pin's presence is checked. (S126)
- **Group mode is ported but never exercised.** No retail entry uses it, and only the unit test
  covers its pco name.
- **Not ported:** the props' step, lifecycle (ResetTime, activation), contacts, hash and rollback
  snapshot, and drawing (S126, S127).

## Next

S126 first: capture a knocked free09 prop (a BeachBall by the start), then port the collection world
it steps in.
