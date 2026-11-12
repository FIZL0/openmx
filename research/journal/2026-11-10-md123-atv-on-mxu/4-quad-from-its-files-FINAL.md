# 4 — the quad from its files (FINAL)

Task MD123. 2026-11-11.

## What I did

MD129 and MD123a had landed, so the remainder of MD123 could be built: a quad made from its files alone,
with no RAM image.

- **`quad::Ride::spawn(load, model, at, heading, bounds, rest)`** (crates/game-sim/src/quad.rs, new section
  `MD123: the quad made from its files`). It takes `Load::read_model`'s keys and lays them over a quad at rest:
  - **Scene.** Node 0 is `spawn_frame(at, heading)` with no parent, and its box is the model's
    (`Model::bounds`, half_box's tree with its centre). Under it hangs the model's first chain
    (`Model::first_chain`: the first root, then each node's first child) with the bind locals, up to
    `atv_moto::NODES`. Then `scene.update`.
  - **Body.** Placed one foot up the frame's up row from the node (the steps keep the node as the body
    less its up row) and turned to the frame with `math::mat4_to_quat`. It is still, enabled on every axis,
    with top speed 300 and no acceleration limits, and then `Load::tune_body`.
  - **Quad.** Active, pushing, an authority time of 1. Heading, up and node position come from the
    frame, then `Load::tune_quad`. Each wheel's spring is its spring when stopped.
  - **Bike steps' words.** Every field is set explicitly. The engine is the loaded one with the
    constructor's two torque scales at 1: no key writes them, and with them at zero the quad stood still
    on the gas (1.3 ft/s top). The wheels have a ground query, the front pair steered, and spring rates.
    The steer axis is up, the spin brakes are 0, the 360's keys come from `[Three Sixty]` with stunts
    44/45 and direction 1, the stunt words are 0xff, and the backflip is `FIRST_FLIP_STUNT`. `cofg_off`
    is `WreckCofG` and the bounds floor is the part reader's minimum reset height. Then `Load::tune`.
    The parts' flag byte is 5, so the collision sync is off.
- **`quad::Ride::rig(load, riding, wreck)`**: MD123a's record, tail and wreck, from the files.
  - **Record.** `mass::Part::words` (the part reader's words) is laid at scratch places, with the
    record's constructor words over it. Those are: world rays; rays and sweep as
    `CheckForCollisions`/`CheckForHighSpeedCollisions` say; rests and damping of 1; a body; terrain;
    near-field; capacity equal to the riding rays; both probes; actors 0x1000; collides with all; and the
    class word 0x3012. The result is read back with `handling::record::read` and `held`, so the
    record and its resolution's fields come out through the same readers MD123a's image seeding uses.
  - **Riding rays.** These come from the bike's `m.col`. For the R quad that is MXVSATV.PSR's
    `Veh\Col\250R_ATV_ChasisM.col` (1520 bytes, 26 rays, PSR sector 123665). I copied it into
    `context/mxvatv-ps2/md37a/250r_atv_chasism.col`. It is game data and not committed.
  - **Wreck body.** Built on WreckATVm.col with `Ride::build_wreck`. The handles are 1–4: any distinct
    non-zero words, since nothing on an MXU world names them.
  - **Tail and wreck.** The tail has vehicle-state mode 1 and stunt speed 5. The wreck has
    `MaxWreckTime` from the `.phy` and struck class 0x20.
- **Tests** (crates/game-sim/tests/atv_quad/mxu.rs, appended):
  - `the_quad_from_its_files_is_c28s_spawned_quad`
  - `the_quad_from_its_files_rides_free09_headless`
  - `the_quad_rigged_from_its_files_is_md123as`
  - `the_quad_from_its_files_wrecks_on_a_free09_prop_and_is_put_back`

  The helpers are `quad_files`, `quad_cols`, `from_files`, `files_on` and `prop_run`. `prop_run` is a
  copy of the MD123a test's prop search, so that test is left untouched.

## What the original does

The quad's constructor and the init's spawn half write the words no loader touches. I read their
values off the C28 spawn image (`c28-atv-r-spawn.ee.bin`, quad at 0x0150a990) and off the Oasis spawn
image's record (`atv-r-spawn.ee.bin`, the quad MD37a uses). I did not read them from the constructor's
code. The record's facing word is the curve table's cosine of `HighSpeedMinCollisionAngle` read as
radians: cos 45 = 0.5253. `mass::facing` already did this.

## How it was proved

All runs went through `./kit check -- -p game-sim --test atv_quad`: 113 passed, 0 failed. The new tests
were also run with `--nocapture` to rule out silent skips.

- **Against C28's spawned quad, bit for bit:**
  - params, the body's limits and mass, joystick, air keys, gyros, stunt ids, centres of gravity;
  - the clutch keys, the engine's keys (gear, both torque scales) and tables, the 360's keys;
  - the parts' flags, each wheel's tuning and spin, the quad wheels, the steer axis, the quad's keys;
  - the scene chain's links.

  The scene node's box is 0.00015 ft off: MD134's box float.
- **Against MD123a's image-seeded rigs** (the Oasis image), bit for bit:
  - the record's flags, tuning, offset, lead, push-out and facing, capacity, near-field, mask and probes;
  - the resolution's collider (mass, springs, responses, masks, surfaces, actors, the class word);
  - the 26 riding rays and the 23 wreck rays, and which rays are active;
  - the tail's mode, stunt speed and surfaces, and the wreck's time (1.75) and class.

  The collider's inverse inertia is 2e-7 relative off: MD134's float again, through the inertia.
- **free09 headless** (the files quad):
  - It stood 60 frames on all four, moving 1.3 ft.
  - On the gas it reached 106.2 ft/s and covered 513.6 ft, on the ground 276 of 360 frames, never
    lower than 0.55 ft over it.
  - On the stick it turned 1.83 rad at most. It handed on only `GravityRecord`.
  - Off the lip at (1490, 2438) it was 20 frames in the air, then 56 frames on all four.
- **Wreck and put-back** (the files quad, rigged from files): on the gas at free09's prop 270 the record
  touched the prop for 75 frames. The quad wrecked with reason 15 at frame 44, was put back at frame 97
  (1.75 s on), and rode 28.9 ft on with the stick over.
- **MD37b's replays and the Oasis chain are unchanged.** The 113 tests include every replay and the
  `frame::` chain through `quad::Ride`. `./kit audit` is ok.

The files test's turn is the most the velocity heading turned while the quad still had speed (over
20 ft/s). The image test's metric, the last frame's heading, read −0.22 rad for the files quad. A
frame-by-frame probe of both quads showed the same steer command, side loads and heading to within
0.1 rad up to frame 330. After that both leave the track and come down tangled, so the last frame
says nothing about the stick. The image test is unchanged.

## Not verified / not 1:1

- **The constructor's words were read off two images, not out of its code.** These are: top speed 300,
  acceleration limits at float max, stunt words 0xff, the 360's stunts 44/45 and direction 1, flags 5,
  authority time 1, torque scales 1, steer axis up, spin brakes 0, actors 0x1000, class word 0x3012,
  stunt speed 5, vehicle mode 1, struck class 0x20. A value that comes from somewhere I did not see (a
  race setup, a menu) would match in these images and differ elsewhere.
- **The pose is mine.**
  - The body is placed at spawn plus the frame's up row, and its quat comes from MXU's `mat4_to_quat`.
    I didn't trace how the init places the body.
  - The scene chain's locals are the bind locals. The image's are animated mid-ride, so the comparison
    covers the links only.
- **MD134's one float.** The scene box (0.00015 ft) and the record's inverse inertia (2e-7) are what
  MD134 is about. Both are compared within a tolerance, not bit for bit.
- **The collision sync is off and no spatial entry is made.** Every call they make is a move in
  SLUS-21104's spatial database, which an MXU world doesn't have.
- **The rig's scratch layout.** The record is read back from scratch addresses. Its body pointer leads
  to a scratch body, which is replaced by the ride's. The `w` lanes of the world centre of mass and the
  relative velocity are set to 1 as in the image; neither is compared.
- **The riding rays' file name** is the chassis model's name plus `m.col` (quad.rs's `Part::Riding`
  says "a ray probe off the bike's `m.col`"). The test hands the parsed file in. Nothing yet derives the
  name from a vehicle type.
- **The trigger segments' order** follows the `.phy`'s tires (left front first). The image's riding
  rays are a different collidable, so I compared nothing there.
- **No capture can prove any of the free09 runs.** The original has no quad on MXU tracks. The tests show
  that the quad rides, turns, jumps, wrecks and is put back, not that it does so as any original would.
- **Still standing from files 2 and 3, and MD123a:**
  - the MXU surface numbers aren't remapped;
  - `OnWorld`'s contacts aren't SLUS-21104's nearby search;
  - the calls dropped in `rest` (`PartMove`, `SpatialMove`, `PreloadCue`, the stunts, rumble, posts);
  - `Ride::frame`'s `Bike` clone isn't timed;
  - MD123a's own list (every prop given the struck class, the put-back spot rule, and the rest).

## Next

Nothing for MD123. MD134 (the box's float) would make the two tolerances exact. A shell or menu entry
that builds the quad by vehicle type (finding its `.phy`, `.psg` and `m.col`) is not filed. No task asks
for one yet.
