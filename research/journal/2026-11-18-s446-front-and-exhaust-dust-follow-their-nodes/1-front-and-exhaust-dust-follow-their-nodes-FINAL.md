# 1 — front and exhaust dust follow their nodes (FINAL)

Task S446. 2026-11-18.

## What I did

- `crates/game-sim/src/dust.rs`:
  - New `FrontNode` (Bike's, Handlebars', Shocks' and the emitter node's locals, the clip's
    Handlebars `pose`, the steering axis, the fork's rest place and slide scale). `point(root)` runs the
    chain with `atv_moto::frame_mul`. `drive(root, feed, curve)` is the original's step after the
    emitters: Handlebars set to the pose, Shocks slid, then on the ground Handlebars turned by the steer.
  - New `ExhaustNode` (Bike's local, the pipe's local, the emitter node's). `drive(root)` sets the
    pipe's local to Bike's world.
  - `Place::Front` and `Place::Exhaust`. All three points are worked out once a frame, before the
    emitters; the drives run after them, as in the original.
  - `Feed` gains `front_compression`, `steer`, `grounded`; `Sim::dust_frame` fills them from the front
    wheel and `air.on_ground`, and passes the ride's curve table.
- `crates/game-sim/src/kiowa.rs`: `axis_turn` and `node_unpoint` are `pub(crate)` (no other change).
- `tests/ai/race4.rs`: `c33_front_node`, `c33_exhaust_node` read the chains off the image. The seed puts
  the front sprays on `Place::Front` and the exhaust on `Place::Exhaust`. C33_DUST stays green.
- `tests/ai/draws.rs`: the emit hook and the tick loop check both chains per tick (below).
- `research/oracle.txt`: the S446 block. S447 noted with the Handlebars pose.

## What the original does

- Emit: as S421 (`./kit fn 0x001b2870` on the node). The front node sits under Shocks, under
  Handlebars, under Bike. The exhaust node sits under the pipe, which has no parent.
- The drive `./kit fn 0x002593e0` runs after the emitters, so each emit reads the nodes the frame
  before left.
  - First the bike's clip poses Bike and Handlebars (`./kit fn 0x00146870` and its two siblings by
    kind). While riding, Handlebars moves about a degree from the image's.
  - Then the fork: `+0x9b0` + axis `+0x9d0` × (`+0x9e0` × front compression `+0x86c`), plus the nudge
    (-0.03, 0, 0.02, 1). Into the world by Bike, then `./kit fn 0x001b2660` writes it into Shocks
    through the posed Handlebars' world (`./kit fn 0x001b3180`). Shocks' rotation is kept.
  - If `+0x93c` (a wheel down): `./kit fn 0x001b2e50` turns Handlebars by the steer `+0x874` about the
    axis, place kept.
  - Attachments (`+0x950` list): kind 0 takes Bike's world as its local. The exhaust pipe is kind 0.

## How it was proved

- **Ee, the original's whole frame, C33, 60 ticks** (`tests/ai/draws.rs`
  `s328_c33s_other_draws_are_looks_notes_and_dust`). Per rider (4) and tick:
  - Each of the 2 front and 1 exhaust emitters: the port's `point` from the original's locals equals
    `+0x40` after the emit (720 points).
  - The drive, run on the frame before's nodes with the end snapshot, the root, Bike's local and the
    clip's Handlebars (read when Shocks is set): Shocks, Handlebars and the pipe equal the original's,
    all 16 lanes (240 rider-ticks).
  - Exercised: 218 rider-ticks grounded (132 with a non-zero steer), 220 with a front compression.
- **The Sim** (`tests/ai/race4.rs` `s12e_free09s_four_racers_ride_as_the_capture`): seeds and runs;
  C33_DUST matches as before. Both tests pass under `./kit check`.

## Not verified / not 1:1

- **Handlebars' clip pose is held at the image's.** The original's clip moves it on 229 of 240
  rider-ticks. Fed the image's pose, Shocks is off by about 15 ulps once the fork slides, and the
  bars' place differs too. So the Sim's front points part from the original's from the first tick.
  Why: the bike's clips are not in the Sim (S447, noted).
- **Bike's local is held**, as in S421; the exhaust pipe and both chains inherit that. S447.
- **The image's front and exhaust points are not checked at the seed.** The drive moved the nodes
  after the image's last emit, so the image's `+0x40` is a frame older than its locals.
- **The seed takes the image's Handlebars as the pose.** That holds only while no wheel is down (the
  turn replaces it); the seed asserts C33's riders are in the air.
- **The emitter offset** is taken as 0, as in S421.
- **Only C33 seeds these nodes.** No live shell race builds `Sim::dust` yet (S420).

## Next

None.
