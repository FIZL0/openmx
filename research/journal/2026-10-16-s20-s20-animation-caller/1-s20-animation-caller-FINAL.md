# 1 — s20 animation caller (FINAL)

Task S20. 2026-10-16.

## What I did

S6 ported the rider's animation *selection* — which clip slot each state asks for and with what weights —
and left four things on its own "not verified" list. This is those four, and they turned out to be four
different pieces of the same thing: the step between the selection and the motion library.

In `crates/game-sim/src/motion.rs`, all additions, nothing moved:

- **`Library` and `Animations`** — the rider's clip table, and the two things it needs of the layer that
  owns the animation set and the disc. `Library::load` is the loader: the name every slot is looked up
  by (`"<prefix>bike_<stunt name>"`, cut off at 31 characters), the `UsedForBike` flag's one effect on
  animation, and the three airborne poses as named fields. `Library::clip` turns a selection's
  [`Slot`] into a handle and `Library::present` is the `Clips` the selection already took.
- **`Player::play` and `Put`** — the one function in the game that starts a clip-to-clip transition,
  which is what sets `Fade::on`. Nothing in the rider's own animation section calls it.
- **`Fade::from`** replaces S6's `Fade::plain`. S6 read the field at +0x4c as "which transition
  sampler"; it is the clip handle the transition is coming *out* of, and it is `play` that writes it.
  Its nullness picks the sampler, which is why the flag looked like one.
- **`Snapshot` and `Joints`** — the pose a cross-fade fades from, which S6 stubbed. Two buffers, one
  quaternion and one point per joint, taken off the skeleton the frame the motion restarts.
- **`Blend::Three`**, `Blend::mode`, `Blend::driven`, `Blend::fires_empty`, `Blend::clips` and
  **`Player::run`** — the dispatch the frame ends in, including the fourth entry point and the answer
  to "blend mode 3": there is no mode 3.

And one fix to S6's own clock: `Player::drive` applied the one-clip entry's "a sample time of exactly
zero fires nothing" rule to all three blends. Only the one-clip entry has it. `Player::drive_at` is the
rule as an argument, and `Blend::fires_empty` says which entry wants it.

## What the original does

Read along with `./kit fn`; every name below is in `research/oracle.txt` with its address.

**The clip table.** `rider_load_clips` — the same function S6 named `stunt_specials`, which does rather
more than find the two special slots. Two passes over the 24 stunt records. The first builds
`"<prefix>bike_<Name>"` with `snprintf` into a **32-byte** buffer and, *only if the record's
`UsedForBike` word at +0x4c is set*, calls `anim_find_or_load` (the set, else the disc) and
`anim_bind_clip`. The second pass builds the same name again for **every** record, marked or not, and
writes `anim_clip_handle`'s answer into +0xa88 + id×4 — a plain 0 when the set has no clip of that
name. Then `"bike_air_Fwd"`, `"bike_air_Mid"` and `"bike_air_Bck"` into +0xaec, +0xaf0 and +0xaf4,
which are named fields that happen to sit where ids 25, 26 and 27 would index; the id-24 word at +0xae8
is never written at all. Clip names are matched with `stricmp`, so the lookup is case-blind, and an
unbound clip and a missing clip both come out as the same null — the port cannot tell them apart
either, because the original does not.

**Who sets `Fade::on`.** `motion_play` (length in f12, motion, clip handle, keep-the-phase) is the only
writer of +0x80 in the image. A length of zero is a hard cut — finished off, the clip on, no transition,
the clock at zero. Otherwise it starts the transition: the delay and the clock both take the *old* clip
time, the end is that plus the length, and +0x4c takes the clip that was playing. If there was no old
clip, **or a transition was already running**, +0x4c is nulled and `anim_rewind` takes the pose snapshot
instead — a transition on top of a transition cannot fade from a clip that is itself mid-blend. With the
third argument set, the clip time is carried across as `new.duration × (old time / old.duration)`,
worked out from the old clip *before* any of this and written back after all of it. `motion_play_clip`
is a tail jump into it with the length read from the clip's own +0x40, forwarding its other arguments.

Its four callers are that wrapper, the vehicle's animation step, and two other actors. **Not the
rider**: the rider's animation section only ever asks for a phase-driven blend, so the cross-fade the
rider's `ramp` drives is the blend entry's own weight against the pose snapshot and not this at all.

**The pose snapshot.** `anim_rewind` (motion, a tag array, a mask). It allocates one 16-byte rotation
and one 16-byte translation per joint, at +0x248 and +0x24c, the first time it is called and reuses them
after — it checks the pointers for null and nothing else, so a skeleton that grew joints between two
snapshots would be written past. Then, per joint in order, the joint's node (the array at +0x234,
stride 0x60, the node at +0x34) through `anim_joint_rotation` — which pulls a quaternion out of the
node's local matrix — and `anim_joint_translation`. The tag argument is null at all five call sites: it
would set the "tagged" flag at +0x254, make a buffer of ones through `anim_fill_words` if there is not
one yet, and store `mask ^ tags[joint]` alongside each joint. Nothing in the image passes it.

**Blend mode 3, and the fourth arm.** The dispatch at the tail of `stunt_clocks` (`./kit dis 0x259b00
0x259c00`) is a chain of equality tests on one register — 2, then 1, then 0 — with **no arm for 3**: a 3
falls out of the bottom and the frame makes no call. In the rider's copy that register is written
exactly four times (0 in the prologue, 2 in the airborne arm, 1 in the two arms that end on the neutral
pose), so the rider reaches modes 0, 1 and 2 and nothing else.

The blocks Ghidra drops are not the rider's. The second copy of the same dispatch is in
`vehicle_animation` (`./kit dis 0x249b20 0x249c40`), which has a sixth arm at mode 5, and it is there
that mode **4** is written, at 0x002497b4 and 0x002498d4 — two arms of a 60-entry jump table on the
vehicle's own animation state. `motion_blend_three` takes three weights and **four** clips, not three:
a/b mixed by f13, c/d mixed by the same f13, and the two results mixed by f14, everything sampled at
f12 × the first clip's duration and the action frames taken from the first clip. That is what
`Blend::Three` is, and it is why the variant exists at all for a rider that never asks for it.

Two more things fell out of reading the entries. The field S6 called the sampler selector is the
from-clip: a non-null +0x4c picks the arm that mixes the new clip at +0x84 with the old one at the fade
clock +0x88, and a null one picks the arm that mixes the new clip with the pose snapshot — so the two
S6 oracle names (`motion_transition_weighted`, `motion_transition_plain`) are the wrong way round, and
`fade_from_pose`/`fade_from_clip` are the same two addresses under names that say what they do. And the
one-clip entry skips its action frames when the sample time is exactly zero where the two-clip and
four-clip entries do not, which is a visible difference, not a cosmetic one: the window test is
inclusive, so an empty window still fires a key sitting on a clip's first frame.

## How it was proved

Four new oracle proofs in `crates/game-sim/tests/motion.rs` (5 to 8), all bit-exact, all run by
`./kit check` — 410 tests pass.

- **5, the clip table** — 4,000 generated riders × 27 slots through `rider_load_clips` itself. The disc
  read (`anim_find_or_load`) and the allocating bind (`anim_bind_clip`) are hooked onto a Rust-side
  shelf; `snprintf`, `stricmp`, `anim_clip_by_name` and `anim_clip_handle` all run for real, so the
  name format, the truncation, the `UsedForBike` gate, the case-blind lookup and both ways of coming
  out null are the original's own code. Generated with mixed-case names, duplicates, names long enough
  to be cut off, clips in the set that nothing bound, and clips bound before the loader runs.
- **6, putting a clip on** — 10,000 generated calls of `motion_play`, a fifth of them through
  `motion_play_clip`, against all nine fields it writes and whether it asked for the snapshot. Hard
  cuts, a transition on top of a transition, nothing playing, and the carried phase (including the
  divide by a zero-duration clip, which both sides answer with the PS2 divider's maximum float).
- **7, the pose snapshot** — 4,970 snapshots over 2,000 generated skeletons of 0 to 40 joints, two or
  three snapshots per skeleton. The engine's two allocators are hooked onto a bump allocator so that
  *when* a buffer is made is part of the comparison, and the two joint reads are replaced on both sides
  by the same recognisable function of the node pointer. Both buffers, the tagged flag, and the tag
  path no caller in the game uses.
- **8, the call itself** — 10,000 frames over all four entry points through `Player::run`, against the
  free-running clip, the last sampled clip, its time and the action-frame window the entry asked for.
  This is the proof that found the one-clip entry's zero rule.

Proof 2 is also a real proof now rather than half of one: it set the sampler field to a flag and then
copied the oracle's answer for which sampler was called. It now puts a real from-clip handle there and
compares the port's own choice.

## Not verified / not 1:1

- **The channel sampling, the quaternion blend and the skinning are still the renderer's.** Nothing in
  proofs 5 to 8 samples a clip: every blend entry is driven with a clip of no channels, which is what
  leaves only the clock and the action frames behind. Unchanged from S6.
- **The two joint reads are not ported.** `Joints::rotation` is documented as the node's local matrix
  pulled back out as a quaternion and `Joints::translation` as its translation with 1 in the fourth
  lane; proof 7 replaces both with the same stand-in on both sides, so what is proved is which node
  lands in which slot of which buffer in which order, not the matrix arithmetic. The quaternion
  extraction is `mat4_to_quat` in `research/oracle.txt` when somebody needs it.
- **`Animations::load` is a stub on both sides.** The asset layer is an input to the port, so proof 5's
  hook *defines* what loading means (look in the set, else the disc, then bind) and the port's mirror
  agrees with it by construction. The hook was written from the original's own first lines, which do
  look in the set first, but the disc read, the clip allocation and the joint-map build inside
  `anim_bind_clip` are not proved by anything here.
- **A blend whose first clip has no handle diverges on purpose.** The original hands the entry point a
  null handle and the entry dereferences it; the port does nothing and keeps its clock. Asserted as a
  divergence at the end of proof 8. It needs a rider whose animation set is missing a clip the
  selection still asks for, which the fallback to the neutral pose is meant to prevent.
- **`Snapshot::take` grows its buffers where the original would run past them.** The original allocates
  only when the pointer is null, so a joint count that *rose* between two snapshots would write past
  both buffers; the port reallocates. Proof 7 holds the joint count still within a skeleton, so the
  difference is unproven in both directions. Same for the tag buffer.
- **The snapshot's tag path has no caller in the image.** All five call sites of `anim_rewind` pass a
  null tag array. Proof 7 proves the path against the original anyway, but nothing says what it is for.
- **`Blend::Three`'s own selection is not ported.** Which four clips and which three weights the
  four-clip entry is called with lives in the vehicle's animation step, behind a 60-entry jump table on
  a state machine this task did not touch. What is proved is the call: the clock, the action frames and
  the argument order. Mode 5, the vehicle's sixth arm, is not modelled at all.
- **`Put::clip` carries a length its caller supplies.** The engine's one-argument play reads the fade
  length out of the clip's own +0x40, a field the port's `Clip` does not have because nothing on the
  ported path reads it. Proof 6 drives the original's wrapper and hands the port the same number, so
  the convention is proved; the field is not modelled.
- **Two records of the same name resolve differently from the port's table reader.** The original's
  walk writes the no-hander and throw-away slot every time it matches, so it keeps the *last* match
  where `Set::new` (S6, `trick.rs`) keeps the first. `stunts.ini` has no duplicate names, and fixing it
  would mean editing another task's code, so proof 5 generates at most one of each special name and the
  divergence stands recorded here.
- **The vehicle's own transitions are not ported.** `motion_play`'s live caller is the vehicle's
  animation step, with the fade length from its own field and the clip out of its own table. The port
  has the function it calls, not the caller.
- **No emulator was used.** Everything here is the function oracle; nothing was checked against a
  running frame, so the claim "the rider never starts a transition" rests on the four call sites of
  `motion_play` in the image rather than on a capture.

## Next

Nothing is owed for S20. Two things it leaves lying around for whoever wants them: the vehicle's
animation step is a whole unported state machine (a 60-entry jump table, its own clip table at +0x900,
and the only live caller of `motion_play`), and `trick.rs`'s `Set::new` keeps the first record of a
duplicated special name where the original keeps the last.
