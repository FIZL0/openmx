# 1 — the snapshot the restart takes (FINAL)

Task V16. 2026-10-17.

## What I did

V15 found the port's pose snapshot reading the wrong buffer. `Set::rewind` copied the live
`Set::pose` — the blend buffer, which holds only the joints the playing clip has channels for —
while the original's restart hook reads each joint's *transform node*. On this disc's rider that is
21 joints of 22 where the port held 0 and the original holds the `.psg` node's own local
translation, 0.28 and 0.69 and 0.96 away. V16 makes the port read what the original reads.

1. `crates/game-render/src/skin.rs`: a new `Skeleton::nodes(&Pose) -> Pose` — the node tree as a
   pose. A joint the clip wrote keeps the pose's value; a joint it did not keeps the bone's own
   local transform, with the translation's fourth lane written to 1 the way the original's accessor
   writes it. Every joint of the result counts as written, because every joint of a snapshot was
   read.
2. `crates/game-render/src/anim.rs`: `Set::rewind` now takes `Skeleton::nodes(&self.pose)` and hands
   *that* to both ends — `Set::under` and `Snapshot::take` — so the simulation's record and the
   renderer's copy still cannot drift.
3. `crates/game-render/tests/skin.rs`: the node read proved against the original's own two
   accessors over 10 000 generated nodes, and a port-only check of the other branch.
4. `crates/game-sim/tests/motion.rs`: the stepping test now runs the port's own two rewinds (the
   frame's restart and the played-once group's), the same two the renderer's `Chassis::frame` has
   and in its order, and compares `Set::under` against the original's two snapshot buffers every
   tick instead of only seeding from them.

**It came out on what the task named.** On the 21 joints no clip of this set translates, the port's
snapshot is now the original's **bit for bit** on all 394 ticks of the recording before its first
wreck — 0e0, not a tolerance — where before V16 the same assert fails at the port's first restart
(tick 2865, joint 16 `LwrLegL`, 1.46 away; I reverted `rewind` to check that it does). The node read
itself is bit-exact against the game's own code on both lanes of both accessors, 10 000 nodes.

**What it did not come out on** is the task's second half: the test still seeds `Set::under` from
the capture, because the rest of the snapshot — the root's translation and the rotations — does not
match and the fade proof would collapse with it. That is measured, printed and listed below.

## What the original does

**The restart hook** (`./kit fn 0x001461e0`, boot overlay) allocates `joint_count << 4` bytes twice,
at +0x248 for the rotations and +0x24c for the translations, then walks the joint records at +0x234
on a 0x60 stride and takes each record's **transform node at +0x34** through two accessors:

* `./kit fn 0x001b25f0` — the node's rotation: its matrix at +0x40 through the largest-component
  matrix-to-quaternion conversion (`./kit fn 0x0013f7b0`, ported as `game_sim::math::mat4_to_quat`
  and already oracle-proved in `crates/game-sim/tests/math.rs`).
* `./kit fn 0x001b2840` — the node's translation: the matrix' last row at +0x70, with **1.0
  written** into the fourth lane rather than read from +0x7c.

The node is not the blend buffer. A clip writes the joints it has channels for and leaves the rest
alone; the blend buffer is those joints only, and the node tree the blend is pushed into still has
the `.psg`'s own transform everywhere the clip is silent. So the snapshot of a joint the clip does
not translate is the bone's local translation, which is exactly the 21 joints V15 measured the port
0.96 away on.

## How it was proved

* **The function oracle, 10 000 generated nodes**
  (`crates/game-render/tests/skin.rs::the_pose_snapshot_reads_a_node_the_way_the_original_does`).
  Each node's rotation rows are laid down by the game's own `transform_set_rotation` from a
  normalised decoded key, its translation by hand, and its +0x7c deliberately set to something that
  is not 1. Then the game's two accessors are called and compared against `Skeleton::nodes` of a
  one-bone skeleton with nothing written over it. **Worst rotation lane 0e0, worst translation lane
  0e0** — bit-exact, both asserted at tolerance 0 for the translation (it is a copy on both sides)
  and 1e-5 for the rotation (it is not, and it still came out exact).
* **A PINE recording compared frame by frame**, `context/capture/v12/rider-joints.csv` (V15's
  capture, 1500 rows, game data, not in git), through
  `crates/game-sim/tests/motion.rs::the_ports_machine_steps_to_the_originals_clip_phase_and_pose`.
  22 restarts in the recording. Before its first wreck: **394 ticks, 5 of them a tick the port took
  its own snapshot on, the 21 untranslated joints off by 0e0**, the root's translation by 8.39e-1
  and the rotations by 1.66e0. After it: 908 ticks at 4.38e-1 / 4.09e0 / 1.40e0. The 21-joint number
  is asserted; the other two are printed.
* **The assert was shown to bite.** With `Set::rewind` put back to copying the live pose, the same
  test fails at tick 2865 — the port's first restart, joint 16, 1.46e0 — so the 0e0 above is the fix
  and not an assert that cannot fail.
* **The rest of the suite.** `./kit check` green, 509 tests. It was not when I arrived: this
  worktree's `context/capture/` had the three acceptance captures loose at the top level, and all
  of them diverge at tick 0 on `bike.pos[0]` (4309.5 against the port's 0) because they were
  recorded from a start the acceptance fixture is not. That fails with my three files checked out
  to HEAD too, so it is nothing of V16's. The main checkout keeps those captures in a
  `capture/acceptance/` subfolder, where the reader does not pick them up; I moved this worktree's
  to match. Local game data either way — `context/` is not in git.

## Not verified / not 1:1

- **The stepping test still seeds `Set::under` from the capture, which the task asked it to stop
  doing.** The port's snapshot is now the original's on the joints the clip does not write, but not
  on the ones it does, and the fade is against all of them: dropping the seeding would unprove the
  104 fading ticks V15 added rather than prove the restart. The seeding stays and the port's own
  snapshot is compared against the original's buffers *beside* it, every tick, so `Set::rewind` is
  now measured either way.
- **The written joints' half of the snapshot does not match, and the cause is not isolated.** On the
  five pre-wreck ticks the port takes its own snapshot on, the root's translation is 0.33 to 0.84
  away and the rotations 0.05 to 0.17 once a whole-quaternion sign flip is folded out (one of the
  five is a flip: 1.66 raw). The tick before each of them is posed and agrees with the original to
  1e-6, so this is not the port's pose being wrong — what the original's hook reads for a *written*
  joint is not that joint's blend buffer on the tick before, and this capture does not say what it
  is. The capture carries the blend buffer and the snapshot buffers but not the node tree itself; a
  capture of the joint records' nodes (+0x34, matrix at +0x40) on a restart frame would settle it.
- **After the recording's first wreck the node tree has a writer this object is not.** All 22 node
  translations change from tick 3260 on while the blend buffer still translates only the Pelvis, so
  even the 21 untranslated joints stop agreeing (0.44). Those 908 ticks are measured and printed,
  not asserted. Something in the wreck's own put-the-rider-back code writes the nodes; finding it is
  not this task.
- **The port does not round-trip a written joint's rotation through the matrix.** The original puts
  the blend into the node and reads the node's matrix back out through `mat4_to_quat`;
  `Skeleton::nodes` takes the pose quaternion straight. V15 measured that round trip bit-exact on
  all 22 joints at tick 3260, so this is a shortcut and not a difference — but it is a shortcut, and
  if a clip ever writes a non-unit quaternion the two would part.
- **`Skeleton::nodes` calls `game_sim::math::mat4_to_quat`, which is ps2-float arithmetic, inside a
  crate whose Cargo.toml says it draws in host `f32`.** It is the right call — it is the original's
  conversion, and the oracle proof above is bit-exact because of it — but it is the first place in
  `game-render` that reaches for the simulation's float emulation for a per-frame path. Only
  per-joint per-restart, so the cost is 22 conversions a restart.
- **`Set::rewind` now allocates a `Pose` per restart** where it used to `clone_from` into the one it
  had. 22 restarts in a minute of play; not worth a buffer, but it is a change in shape.
- **One recording, one rider, one track.** Everything in the second bullet list above is V15's
  capture of the human's `first_recording.p2m2` replayed with nobody watching, and nothing here is
  proved for a second rider, the AI's riders, or a clip set that *does* translate the other 21
  joints — which is precisely the case this fix was written for and the case this recording does not
  contain.
- **The capture is game data and is not committed.** Both tests print SKIPPED and return without it,
  so a checkout without `context/capture/v12/rider-joints.csv` still passes `./kit check` and proves
  nothing about the snapshot.

## Next

1. Capture the joint records' transform nodes themselves (+0x34 per record, matrix at +0x40) across
   a restart frame. That is the one measurement that would say what the original's hook reads for a
   joint the clip *does* write, and it is what stands between this test and dropping the seeding —
   the task's own second half.
2. Find what writes the rider's node tree during a wreck. Until then every pose comparison after a
   wreck is against a tree this animation object did not write alone.
