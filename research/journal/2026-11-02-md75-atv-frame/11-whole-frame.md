MD75e: the quad's whole physics frame off the image

2026-11-04. Task: `./kit plan MD75e`.

Up to now, every MD75 proof ran one listener of the quad with the others hooked. This task runs the whole
frame. SLUS-21104's world step (`atv_world_step`) runs once over the game's own physics world in MD37a's
`atv-r-spawn.ee.bin`, at 1/30 s, with nothing hooked but the ground query. That leaves unhooked the
collision sweep through MD86's search, the world's other five preforce listeners, and the whole scene. On
the other side, the port's `quad::frame` runs on `image::read_ride`'s read of the same image. The test is
`tests/atv_quad/frame.rs` `the_spawned_quads_whole_frame_is_the_originals`. Field notes are in
`research/oracle.txt` under "MD75e".

## The setup

Both sides answer the ground query with `atv_ground::Terrain::ground`, over Desert Oasis's
`MC02.ps2.trn` out of `mc02.res`. MD76a proves that query against the original's. The port runs
`quad::Steps` with `atv_search::Oasis` as its sweep. That is MD86's Desert Oasis world, built by
`oasis::desert_world` (MD37a's race rows are not needed here). The image's nearby list is empty, as
`Oasis` with `gathered` set takes it.

At 115 iterations a second, one frame of 1/30 is four substeps. The original runs 63,073 instructions
and asks the ground 16 times, four wheels by four substeps. The port asks 16 times too. The port hands on
two kinds of call, and the test pins both:

- four `GravityRecord`s. The record's vt `+0x88` walks its contacts, and the image's record has none.
- `PartMove` for the frame mesh and the riding probe. That is the image's own database, which MD75c1
  and MD89 already move.

## What the first run found

At first, 20 of the 2,923 words the poke compares differed. The rigid body was equal throughout. The 20
words came from three places, and all three were the port's:

- **The body's last position (`+0x60`).** `atv_moto::Parts` keeps a copy of it, and `head_tail` moves
  that copy. `quad::frame`'s listeners only see the `quad::Body` view, though. The body's `advance`
  had already overwritten the real word, so the copy and the body disagreed. Now `Moto` has two
  default no-op methods, `body_in` and `body_out`. `Frame::listen` calls them around each listener.
  `quad::Steps` takes the body's last position and enabled flag in, and puts the last position back out.
- **The node's right and up heights (`+0x84`, `+0x94`).** The backflip reads `Bike::right_y` and `up_y`,
  which were copies taken once. The original reads the node's world rows where they live. So
  `Steps::call` is now a trampoline over the old match (`answer`). When a call changes node 0's world
  rows, it refreshes the two copies from them.
- **The trigger probe.** In the original, the search's vt `+0xc8` stands the trigger at the node's frame
  before it sweeps. The port swept from that frame but left the ride's trigger collidable unstood.
  `Call::Contacts` now stands it with `atv_sync::stand` once the sweep answers.

After these fixes, all 2,923 words are equal, and so is every word of the rigid body (`seed::body_words`).

## The proof

- Every word the poke lays of the quad, its body, wheels, nodes, collidables and their blocks equals the
  original's after its step. That includes the image's trigger at `+0x1430`. 386 of those words changed
  during the step.
- Every word that changed in those blocks is either one the comparison reads or one of the full body's
  words. There are two exceptions, and the test names them exactly:
  - bit `0x10` (searched) of `+0x126` on the riding probe and on the sweep probe. The trigger's search
    marks them as candidates in its reach, and then its object list leaves them out. The port's world
    does not hold the quad's own collidables as candidates, and `atv_sync::Collidable` has no flag byte.
    Nothing ported reads the bit.
  - the scene past the ride's four nodes. 44 nodes hang off the quad's node. On 32 of the 40 the ride
    does not hold, the step clears bit 0 of `+0x100` (world frame current). That is the mark placing
    the quad's node leaves down its subtree. No other word of those 40 nodes moves.
- One flipped bit in the port's result goes red in exactly one word.

**The partner borrow on objects and movers** is `tests/atv_quad/borrow.rs`
`the_quads_partner_borrow_on_moving_ground_is_the_originals`. It runs the Force body with the wheel
integrate, surface drag and spin unhooked, as the wheel-spin test does. It covers 4,000 generated rides,
seed `0x0075_e001`. In each one, a wheel is off the ground with its rear-pair flag set, and its partner is
down on an object on a rigid body, an object on a chassis, or a mover. The harness lays that ground
whole. The borrower's own ground is still, the partner's, or the other kind (pointer only). Its
`ground_known` is set a sixth of the time.

The original copies the partner's two pointers, so both wheels share one object. The port copies the
`Ground` value. The harness therefore lays the one object and points every wheel the port has on it
there. Three things must match:

- every region;
- both pointer words of every wheel;
- the object's world centre of mass. Every port copy that wrote it must equal the original's one word.

Each path runs at least 100 times. Two mutants laid on the original's side both went red: the mover's
velocity answered with its spin, and the body holder's velocity laid as its spin.

## Also changed

- `tests/atv_quad/oasis.rs`: the original stands the image's trigger, and the port stands the ride's.
  Before each original step, the contact runs now lay the ride's stand (centre, reach, frame) into the
  image's trigger, then carry what the stand left back to the harness's. The Force-body run gives its
  generated ride the spawn's trigger, because the original's trigger is the image's.
- `desert` split into `desert_world` plus the race rows. `Desert` and `image.rs`'s comparison helpers are
  `pub(super)` for `frame.rs`.

## How it was checked

`cargo test -p game-sim --test atv_quad` (63 tests), `atv_wheel`, `atv_search`, `atv_spatial` and
`mxvatv_bike_ride`: all pass. Then `./kit check` over game-sim.

The three oasis tests skip here, because MD37a's race-row csvs (`capture/md37a/race-*.csv`) are in no
slot. For one local run I let them go on with the 480 aimed frames alone, with the paths check off, and
all three passed. Before the trigger fix to the Force-body ride, that run went red on the trigger's centre.
The temporary edit was reverted.

## Not verified / not 1:1

- The search's searched bit on the quad's own collidables is not carried; nothing ported reads it.
- The 40 scene nodes past `atv_moto::NODES` are not carried. Their world-current bit goes down in the
  original and not in the port. A reader of those nodes (the rider's skeleton) rebuilds their frames
  first.
- The gravity record with contacts (its vt `+0x88` walking them) is not ported. The image's record has
  none, so the port hands the call on.
- The world's other five preforce listeners (the second ATV and four more) run only in the original.
  The proof shows they leave the quad's words alone in this frame. They are not ported.
- The `PartMove` boxes go to the image's own database in the original. The port hands them on, and
  MD75c1/MD89 prove that move separately.
- One frame off one image, at spawn: the quad on the ground, with nothing struck and the nearby list
  empty. A frame with contacts, a struck thing or a gathered list is not run whole.
- The oasis contact tests' trigger hand-over has run only on the aimed frames, without MD37a's race rows.
- The borrow test does not check that the original writes nothing outside the regions and the laid ground.
  The wheel-spin test's `only_touched_all` would flag the object's centre-of-mass word, so it is not run.
