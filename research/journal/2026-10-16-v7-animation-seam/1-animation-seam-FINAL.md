# 1 — animation seam (FINAL)

Task V7. 2026-10-16.

## What I did

Joined the three halves of the animation into one seam, so that a stunt's clip can reach the bones it
animates. Before this, the port had:

- `Clip`, `Pose` and `Skeleton` declared **twice** — once in `game_sim::motion` (S6/S20, the driving)
  and once in `game_render::skin` (V5, the files) — plus a **third** `Skeleton` in
  `game_render::rider` (V4), which is the one a rider's model was actually read into. A clip bound to
  one of them could not pose the other;
- `trait game_sim::motion::Animations`, declared by the simulation because it may not read a file,
  with **no implementation anywhere** — the clip table was filled with handles that pointed at
  nothing;
- the **joint map** the original builds when it binds a clip to a skeleton in neither half:
  `skin::Clip::joints` could compute one, and nothing held it.

What the join looks like now, in the only direction the crate graph allows (`game-render` depends on
`game-sim`, never the reverse):

- **one clip.** `skin::Clip` (the parsed `.mot`) now carries the simulation's own record as its
  `timing: game_sim::motion::Clip` field rather than a `duration: f32` of its own. The duration the
  renderer samples at and the duration the simulation's clock compares against are now the same word.
  `looping` is filled at bind time, not at parse time, because a `.mot` does not carry one — it is the
  animation set's. `Clip::duration()` returns the host `f32` the samplers want. `game-render` gained a
  `ps2-float` dependency purely so the simulation's type is nameable; nothing here does `F` arithmetic.
- **one pose.** `skin::Pose` is the one buffer of joint poses in the program and now implements
  `game_sim::motion::Joints`, which is how `Snapshot::take` reads it. `Pose::of_snapshot` goes the
  other way, for the pose a cross-fade fades from. `motion::Pose` (the rider's lean, fore-aft and
  crossup) is a different thing entirely and keeps its name; both modules now say so in their docs.
  I did not rename it: it is used by `game-sim`'s `rider.rs`, `tests/motion.rs` and `tests/replay.rs`,
  and a rename would collide with whatever else is in flight in those files.
- **one skeleton.** V4's `rider::Skeleton` and V5's `skin::Skeleton` are now a single
  `skin::Skeleton { bones: Vec<Bone> }`, with `Bone { name, parent, local, rest }` — V4's per-bone
  shape, V5's methods (`of`, `find`, `locals`, `world`, `rebind`) and V4's (`at`, `shared_with`,
  `bolt_onto`) on one type. `rider.rs` re-exports it under the name it landed with, so nothing that
  reads a rider's skeleton changed.
- **one loader,** the new `crates/game-render/src/anim.rs`:
  - `Set` — one animation set: a `.mcf`, the `.psg` it names as its skeleton, the archives it reads
    clips out of, the bound clips, the live pose and the pose a fade comes from. It is the port's
    implementation of `Animations`: `load` finds the member, parses it, takes `looping` from the set's
    own entry, and **binds** it — builds and keeps the joint map (`Bound { name, member, clip,
    joints }`); `bound` is the lookup the simulation's `Library::load` fills its table through. A name
    that finds nothing goes on `Set::missing` with the reason instead of vanishing.
  - `Set::load_listed` loads the `.mcf`'s own motions first. This matters: `Library::load` only
    *loads* the 24 gesture slots and only *asks* for the three airborne poses, which live in the
    `.mcf`. Without it a bike can show a stunt and nothing else.
  - `Set::apply` turns a `Blend` into a sampler call, `Set::rewind` takes both sides' snapshot in one
    call, `Set::world` runs the pose down the tree.
  - `Rig` holds the `Set` with the simulation's `Driver`, `Library`, `Player` and `Snapshot`, so
    `Rig::frame` is one frame in the original's order: selection → rewind on a restart → clock →
    sample → pose → bones.
- **one number for the sample time.** `Player::sample_time(blend)` on the simulation side: a
  phase-driven blend samples at `prev` (where `drive_at` left `duration × phase`), the free-running
  advance at `time`. The renderer needs it and has no business recomputing it.

## What the original does

- **Binding.** The loader the port replaces is `anim_find_or_load` → `anim_bind_clip` (named in
  `research/oracle.txt`; `./kit fn anim_bind_clip`). It searches the animation set's loaded clips by
  **name**, falls back on reading the file off the disc, and binds the result to the skeleton it is
  handed — which is where the per-channel joint byte comes from, `0xff` for a channel this model has
  no joint of. S20's proof 5 already runs the caller of this for real with both halves hooked out, so
  the *names* each slot is looked up by are proved; what happens inside, including which file a name
  becomes, is not reachable from the oracle (it reads the disc).
- **The name rules.** The simulation asks by name: `<prefix>bike_<stunt name>` per gesture slot,
  `bike_air_Fwd` / `_Mid` / `_Bck` for the airborne poses. The port resolves a name to a member in
  three steps: the set's `[MOTION n]` entry with that `NAME=`, taking its `VUE=` with the extension
  replaced by `.mot` (the written extension is the editor's name for the format, not the member's);
  else `<ContentDirectory><name>.mot`; else any member ending `<name>.mot`.
- **The sample time.** Each of the four blend entries works the number out and passes it straight
  into the channel samplers; it is stored nowhere, which is why proof 9 is against the clock's own
  window rather than against a field.

## How it was proved

`./kit check`: **457 tests pass** (449 before this task — 8 new).

- `crates/game-render/tests/anim.rs`, **7 tests**, two of them against the user's disc:
  1. *the seam, end to end* — `Bike\250sx.res`'s own `bike.mcf`, its `bike.psg` (22 joints), its 12
     clips bound, the real `stunts.ini`, and the simulation driving a stunt through eleven frames of
     `Rig::frame`: every gesture slot marked for the bike found a clip, every clip's channels are all
     joints of *that* model, the pose is sampled at exactly the clock's `duration × progress`, every
     world matrix is finite, the clip moved joints off the rest pose, and it wrote **only** the joints
     it names (which is what the joint map is for). Then the airborne idle (mode 2, two clips mixed)
     and a `Snapshot::take` read back joint for joint through `Joints`.
  2. *which clips a bike has* — **1296 slot checks over all 54 bike archives**: `stunts.ini`'s
     `UsedForBike` is true for exactly the stunts whose `.mot` the archive ships, slot for slot, in
     both families (49 race bikes with `bike.mcf` and `bike_<stunt>`, 5 freestyle minibikes with
     `bike50.mcf` and `ff_bike_<stunt>`, which is the rider prefix `Library::stunt_name` exists for).
     378 shipped and marked; **none of the 378 is listed in any `.mcf`**, which is the proof that the
     second name rule — found on the disc by name alone — is real and not a guess.
  3. port-only fixtures: the three name rules in order, binding (case-insensitive, `0xff` for a joint
     the model has not, and a rider's clip on a bike's skeleton reaching nothing), each blend mode's
     sampler including the two that draw nothing, the rewind taking the same pose both sides read, and
     a `.mcf` whose geometry is missing erroring rather than panicking.
- `crates/game-sim/tests/motion.rs`, **proof 9** (new, port only): `sample_time` is the clip time the
  clock itself just reported — the far end of the window `run` returned — over three driven modes at
  eleven phases and three durations with and without a preceding sample (so wraps and the empty window
  are both covered), and over both a looping and a non-looping free-running clip.
- S6's, S20's, V4's and V5's existing proofs are untouched and green; `tests/skin.rs` needed only the
  mechanical changes the joined types imply (`clip.duration()`, `skel.len()`, `skel.bones[j].local`).

## Not verified / not 1:1

- **No oracle for the join.** The original's bind lives inside an allocating loader that reads the
  disc, which `ps2-oracle` cannot run, so nothing in `anim.rs` is compared against the original's
  instructions. Everything above the join (S6, S20) and below it (V5) is; the wiring is proved against
  the disc only.
- **The name → member rules are inferred from the disc,** not read out of `anim_find_or_load`. All
  three are consistent with every bike archive on the disc and the second is forced by 378 clips that
  are in no `.mcf`, but the original may try them in another order, or try something else as well.
- **`Blend::Three` is not drawn.** Four clips and three weights: no arm of the rider's own selection
  reaches it (S6, S20), the nesting of two mixes into a third is not ported, and inventing the
  arithmetic would be guessing. `Set::apply` leaves the pose alone and returns `false`.
- **The clip-to-clip `Fade` is not applied.** `Player::fade` is stepped by the simulation and ignored
  by the renderer, because the rider never starts one (S20) — only the cross-fade `ramp` from the
  snapshotted pose is drawn. A caller that does start one gets no blend out of it.
- **A finished free-running clip holds its pose** instead of being resampled, because `advance` resets
  its clock to zero on the frame it ends (proof 9 asserts this) and sampling there would snap the
  model back to the clip's first frame. Which of the two the original shows is not proved: its mode-0
  path ends in VU0 skinning no oracle call reaches.
- **A joint no clip wrote answers identity/origin through `Joints`,** not the bone's rest transform.
  The original snapshots the engine's own joint array, which holds the rest pose for an unwritten
  joint; the port's `Pose` holds `turned`/`moved` flags and resolves the rest transform later, in
  `Skeleton::locals`. The *drawn* pose is therefore right and the *snapshotted* one differs for any
  joint the clip does not name. It only shows if a cross-fade starts from a pose whose clip named
  fewer joints than the next one — not reachable with the bike's clips, which all name the same six.
- **The two-clip mix pairs channels by index,** not by name — V5's `lay`, inherited unchanged. It is
  right for every pair the game actually mixes (the three airborne poses are literally the same file)
  but would mix the wrong joints for two clips with different channel orders.
- **Nothing here was compared against a frame of the running game.** No emulator was started for this
  task: the seam has no picture of its own, and the pose it produces lands in V5's already-proved
  samplers. A side-by-side belongs with whatever first draws a rider.
- **Only bikes were loaded end to end.** A rider's `.mcf` (`mrider.mcf`) goes through the same code
  and V5 proved its clips parse and pose, but no test in this task binds a *rider* set through `Set`,
  because the rider's prefix and set selection are `rider::Rider`'s question and not this seam's.

## Next

Nothing is left half-finished. The obvious next steps, for whoever comes to the rider: bind a rider's own `.mcf` through `Set` and bolt the posed
rider onto the posed bike (`bolt_onto` already takes the joined `Skeleton`), and take the first
side-by-side of a posed rider against the original — the seam is what was in the way of both.
