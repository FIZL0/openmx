# 1 — animated toys (FINAL)

Task V43. 2026-10-26.

## What I did

- **The clips are the simulation's** (`crates/game-sim/src/toys.rs`, commit ab90e97a).
  - `Toys` holds each toy's `Spec` (motion, rig, placement, collision) and its `Clock`, plus every
    clip's channel hints.
  - `Toys::advance` is `motion_advance`: the clock, the narrow/wide rotation samplers with their
    hints, the position bracket, and the two turn tables rebuilt as the start-up builder builds them.
    `Toys::worlds` is `transform_rebuild`'s compose.
  - The toys ride inside `Sim::traffic` (`Traffic::toys`) and are stepped at the top of `Traffic::frame`,
    so every path that steps the cars steps the toys too. Their collision is put among the world's
    props after the cars' (`attach`) and moved each tick:
    - a mesh `.col` stands at the placement;
    - a hierarchy `.col`'s parts each stand at their bone's world matrix.
  - They are hashed (`toys`, `toy_hints`, only on a track with toys) and sent in snapshots (tag
    `V43toys`), so a rollback or a replay moves them.
- **The shell builds them from the disc** (`crates/game-render/src/animated_toy.rs`).
  - It reads `[AnimatedToys]`/`[AnimatedToyN]` through `phy::Ini` (the count decides), with the base
    reader's defaults, and builds the placement with `math::basis(pos, look, up, true, true)`.
  - Each `.mcf` gets one animation set over the archives, and each clip name one sim `Motion`. The set
    is lent the archives and gives them back (new `Set::into_archives`).
  - Each hierarchy part's bone goes through `flatten` (the hierarchy loader's numbering).
  - `crates/openmx/src/main.rs` loads them beside the traffic, except in a world mod. It hands the sim
    to every seat with the cars and draws the first seat's toys:
    - posed at the clock blended between the last two ticks (`shown_time`, `palette`);
    - on the GPU through `gpu_poses`;
    - on the software path skinned into a per-toy copy (`<model>@toy<k>`).
- **Two loader fixes the disc needed.**
  - `MotionSet` read `Looping` only as `yes`. The tracks' `.mcf`s write `1`, `TRUE`, `False` and the
    rest, so it now goes through `ini::switch`.
  - `Set::load`: a track `.mcf` has an empty `ContentDirectory`, so a name whose `VUE=` is another
    name's file (free09's `SETI_windmill_02` → `SETI_windmill.vue`) was looked for by full path and
    missed. With no directory, the VUE's `.mot` is now found by suffix. Riders' and bikes' sets all
    have a directory and are unchanged.
- **Every listed track loads all its toys, none left out:**

  | Track | Toys |
  |---|---|
  | free09 | 5 |
  | free10 | 2 |
  | nat01 | 4 |
  | nat02 | 1 |
  | nat06 | 19 |
  | nat08 | 2 |
  | nat11 | 8 |
  | nat15 | 1 |
  | sx01 | 2 |
  | free11 | 0, as the original: the count is 0 |

- Names `animated_toy_vtable`, `animated_toy_update`, `animated_toy_base_read`,
  `motion_turn_table_build`, `node_flatten` and `node_count` are in `research/oracle.txt` V43, with
  the field offsets as comments.

## What the original does

- **Placement.** `./kit fn 2144d0 -p boot` (animated_toy_read) calls `./kit fn 212270 -p boot`:
  1. Box and Position;
  2. Yaw/Pitch/Roll if there is a Yaw, else LookVector/UpVector (`ini_quadword` with the defaults
     above);
  3. `mat4_basis(toy+0x50, pos, look, up, 1, 1)`, which the toy's root node takes as its local
     matrix.
- **Collision.** `./kit fn 119070 -p boot` loads the model's `.col` (MCFName stem + `.col`) against
  the model's skeleton. A hierarchy is split into its parts, and each part is bound to a rigid body on
  its bone.
- **Bone numbering.** The hierarchy reader (`./kit fn 1effe0 -p boot`) numbers bones from the topmost
  node (the toy's root):
  - `./kit fn 1b35b0 -p boot` flattens descendants: first child `+0x104`, next sibling `+0x108`,
    children, then each child's own;
  - a part's bone is clamped to `./kit fn 1b3420 -p boot`'s count − 1.
- **Update.** `./kit fn 2150f0 -p boot` advances the toy's motion by the scene step
  (`./kit fn 144800 -p boot`, motion_advance).
- **Clip sharing.** Clips are the animation set's, one per name. Two toys on one name share its
  channels' hints, which is why the port keeps one `Motion` per name, even where two names are one
  `.mot`.

## How it was proved

- **Function oracle over a save state.** `crates/game-sim/tests/toys.rs` uses first_recording's
  free09 RAM image, with its 4 seti windmills and its rustic windmill.
  - `the_turn_tables_are_the_originals`: both turn tables bit for bit.
  - `the_toys_turn_as_the_originals_do`: `motion_advance` and `transform_rebuild` run in the
    interpreter beside the port for 300 ticks × 5 toys. On every tick these match bit for bit: the
    clock, the previous clock, the finished flag, every rotation channel's hint, and every joint's
    local and world matrix.
- **Disc against the state.** `crates/game-render/tests/animated_toy.rs`
  `the_shells_toys_are_the_states`. Each of the 5 toys the shell builds from free09's disc files is
  one of the state's 5, matched by placement bit for bit. For each one, these are bit for bit:
  - the joint parents;
  - every rest row no channel writes;
  - the clip's duration and looping flag;
  - every rotation and position key (t and raw key words) and every channel name;
  - the joint map.

  I proved it ran with a temporary panic printing `[true, true, true, true, true]`.
- **By eye in the shell.**
  - Command: `openmx --track free09 --shot … --shot-frame 30/40 --shot-pose 2462,180,3220,135,10,85`.
    The seti windmills stand at their places with their blades turning between the frames, on the
    GPU and on `--renderer software`.
  - free10's shovel (`--shot-pose 2930,175,3900,45,0,85`, frames 30 and 90): the cab, the tracks and
    the arm are drawn, and the bucket swings.
  - Not beside a shot of the original.

## Not verified / not 1:1

- **The hierarchy toys' collision is not proven** (free10's shovel_01, nat11's eight gondolas; filed
  as S130). No RAM image of free10 or nat11 exists. Four things are inferred and not checked:
  - Bone 0 is the toy's root, from the decompilation and from vertex counts: the shovel's bone 1 is
    tracks_body (29 verts) and bone 2 is the cabin (78); the gondolas' bones 1 and 2 are
    gondola_top and Nat11_GONDOLA.
  - Siblings are numbered in `.psg` node order. `node_add_child` appends at the tail, so this holds
    only if the model's nodes are added in file order.
  - Each part stands at its bone's world matrix through `Prop::on`.
  - The original binds each part to an 800-byte rigid body. Its contact behaviour is not read; the
    port's part is a moving mesh prop like a path car's.
- **The mesh toys' collision** (free09's and nat01's rustic windmills, free10's windsock, nat15's
  steam saw) stands at the placement, as the reader builds it. The prop list's contents for them are
  not compared against the state's collidables.
- **AnimateWhenNotVisible is not ported** (V71). free09.toy writes False for its five toys (free09_ss.toy True), so the
  original does not step them while they are not drawn. The port steps every toy every tick. The proof
  ran the state's toys unconditionally, as the interpreter call does.
- **Keys not ported:**
  - `UpdateSearch`, which 34 toys write (all False);
  - `CameraCollision`, `DisableWhenIdle`, `InitialFrame` and the Yaw/Pitch/Roll branch of the
    placement, which the reader takes but no animated toy on the disc writes.

  The disc's toy sections write only Name, AnimateWhenNotVisible, UpdateSearch, Position, Box,
  LookVector, UpVector, MCFName, InitialMotion and HasCollision. None of them writes InitialTime,
  which is ported anyway.
- **Update order across toys** is the `.toy`'s order, before the cars. The original's scene order is
  not read. It only matters to toys sharing a clip name's hints, and the 300-tick proof stepped the
  state's toys in the order they sit in memory.
- **Sampler corners the disc does not reach, kept as the original has them but not exercised:**
  - the 2-key channel reading key[-1].t;
  - a wide turn index past 4096 running into the narrow table;
  - a position sample before the first key.
- **Only free09's toys are proven against RAM.** The other tracks' clips are read by the same code and
  load without a miss, but are not compared against a state. The sx gates and blimps, nat02's bell,
  nat08's cook and pipe, and nat06's windmills are checked by load count only, not looked at.
- **The picture.** The drawn pose is the sim's own sample at a blended clock, composed in host f32. A
  finished, non-looping clip is drawn at its duration. Both are by eye.
- **A world mod's tiles have no animated toys**, like their cars' collision (no props list).
- **Each seat steps its own copy**, as with the cars. The original has one scene clock.

## Next

None. The follow-ups are S130 (hierarchy collision against a free10/nat11 state) and V71 (the
visibility gate).
