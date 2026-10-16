# 1 — v5 motion clips (FINAL)

Task V5. 2026-10-16.

## What I did

Created `crates/game-render/src/skin.rs` and `crates/game-render/tests/skin.rs`, and wrote the twenty
functions, tables and offsets the task needed into `research/oracle.txt` under a new "V5" section.
S6 ported the clock and said the look was the renderer's; this is the look.

The module is the chain from a clip's bytes to triangles, in the original's own order:

- **the clip** (`Clip::parse`) — the `.mot` file: version, frame count, name table, duration, the two
  key kinds, and the two channel lists;
- **the channels** (`RotChannel::sample`, `PosChannel::sample`) — a quaternion and a translation at a
  time in seconds, with the original's cached-pair hint, its two walks, its held ends and its
  extrapolation past the end;
- **the quaternion** (`decode`, `decode_narrow`, `slerp`, `rotation`) — the two compressed key kinds,
  the table-driven blend between them, and the matrix the result becomes;
- **the pose on a skeleton** (`Clip::joints`, `pose`, `pose_faded`, `pose_mixed`, `Pose`, `Skeleton`)
  — the clip's name→joint byte map, the three blend entries S6's driver asks for, and the joint tree
  walked to world matrices;
- **the skin** (`skin`) — the rigid one-node-per-vertex skinning the `.psg` models use.

`Pose` is deliberately the shape S6's `Blend` hands over: a clip, a phase and a weight go in, joint
rotations and translations come out, and `turned`/`moved` say which joints the clip actually wrote.

## What the original does

**Two key kinds, two channel classes.** A clip's `+0x2c` names its rotation key kind, and
`motion_channels_new` allocates one of two 0x1c-byte classes from it — same fields, different vtable,
`+0x10` the sampler. Kind 1 is a twelve-byte key (`f32 t`, `u16 turn`, `i16 axis[3]`), kind 0 an
eight-byte one (`f32 t`, `u8 turn`, `i8 axis[3]`). Read them with `./kit fn 0x00140c80` and
`./kit fn 0x00142750`; the key sizes are visible in the reader's own multiplies (`count<<3`,
`count*0xc`, `count<<4`, `count*0x28`).

**A key is a turn index and an axis.** `./kit fn 0x00100a50`: the `turn` field indexes a table of
`(sin(n·π/4096)/32767, cos(n·π/4096))` pairs, the three axis shorts are multiplied by the sine, and
the cosine's **sign bit is XORed into the sine** so that `w = |cos|` is never negative — a half turn
past π is stored as the negated axis of the same rotation, which is the same rotation. The narrow
decoder (`./kit fn 0x001008e0`) is the same code over a 256-entry table scaled by 127, and that table
sits immediately after the wide one in memory.

**The blend is a table of arc cosines.** `./kit fn 0x0013ee10` does not call `acosf`. It scales the
dot product by 14999.5, offsets into a 30002-entry table of `acos(i/14999.5 - 1)`, takes a four-term
Taylor sine of the angle and of its two fractions, and divides. Above a dot product of 0.99997 it
reads the table's last two entries, which are both `0.00048828125` and an arc cosine of nothing; at or
past exactly 1.0 it returns `from` untouched rather than dividing by `sin 0`. Only the key-pair
wrapper (`./kit fn 0x0013f000`) sets the shortest-path flag, so the flip is part of sampling a channel
and not of the blend itself.

**The rotation sampler holds key 1, not key 0.** `./kit fn 0x00136880`: the channel caches the pair it
used last at `+0x18`, guesses that pair and the next one, then walks forward and, failing that,
backward. Past the last key it holds the last. *Before* the first key its fallback index is **1**, so a
time below `keys[0].t` returns key 1 — the port keeps the index and holds `keys[0]` instead, because
the original's own weight there is computed from `keys[-1]`, off the front of the array.

**The position sampler extrapolates.** `./kit fn 0x00137e70` with `./kit fn 0x001380b0`: a binary
search, then a lerp — but past the last key the search returns the last *two* keys and false, and the
weight is left above 1, so the translation runs on along the last interval instead of holding. A mover
that reaches the end of its path keeps going.

**The matrix is the row form.** `./kit fn 0x001b2130` writes the quaternion as three rows at
`+0x40/+0x50/+0x60` — the transpose of the textbook matrix, which is what a row-vector engine needs.

## How it was proved

Fifteen tests in `crates/game-render/tests/skin.rs`; eight call the original through the function
oracle, one reads the game's own memory, two read the disc. `./kit check -- --test skin -- --nocapture`
→ 15 passed (the log is `context/notes/check.log`).

1. `the_quaternion_decode_matches_the_original` — **10,000** generated keys, 5,051 of them past a half
   turn (a negative cosine, the sign folded into the axis). Worst lane off by **5.96e-8**.
2. `the_narrow_quaternion_decode_matches_the_original` — **10,000** four-byte keys, 4,946 past a half
   turn. Worst lane **5.96e-8**.
3. `the_slerp_matches_the_original` — **10,000** blends: 883 flipped the short way round, 746 early-outs
   on a dot product of one, 1,269 at an end of the range. Worst lane **5.36e-7**.
4. `the_rotation_channel_matches_the_original` — **400 channels × 10,000 samples** for each key kind,
   through a channel built in the emulator's own memory and sampled by its own vtable: 2,442 (wide) and
   2,546 (narrow) past the last key and so held. Compares the four lanes **and** the cached pair the
   original left at `+0x18`, so the search's path is proved and not just its answer. Worst lane
   **4.77e-7** both kinds.
5. `the_position_channel_matches_the_original` — **400 channels × 10,000 samples**, 1,204 of them past
   the last key and so extrapolated, 1,374 exactly on the first key. Worst lane **9.77e-4** on lanes of
   magnitude 8.6e3, i.e. one part in 1e7.
6. `the_quaternion_matrix_matches_the_original` — **10,000** quaternions, three rows each, worst element
   **1.79e-7**.
7. `an_empty_rotation_channel_writes_nothing` — the original leaves the output buffer alone for a
   channel with no keys and for one switched off.
8. `the_tables_are_the_games_own` — the three reconstructed tables against `context/states/human-ram.bin`:
   4,096 wide turns off by at most **1.79e-7**, 256 narrow turns by **1.19e-7**, 30,002 arc cosines (at
   `0x62eaa0` in that state) by **1.08e-5**. Skipped, not failed, when the state is absent.
9. `every_clip_on_the_disc_parses` — **17,337 `.mot` members across 523 archives** (394 distinct names),
   channels of 2..3,001 keys, 300,316 twelve-byte rotation channels and 189 eight-byte ones, the highest
   turn index 3,353 of 4,095. Every one version 3 and position kind 0, every channel's times increasing
   from zero, and every sample a unit quaternion.
10. `a_rider_is_posed_and_skinned_by_its_own_clips` — `Rider\wey1.res`: `turn_stop_fwd.mot` and
    `backflip.mot` onto `mrider.psg`. 18 of 18 channels resolve to joints of that model, 242 joints
    moved across eleven poses, 22 poses skinned onto its 22 joints.
11–15. Port-only, because no oracle call can say what they mean: a channel whose name is no joint is
    skipped, the cross-fade's ends are the clip and the pose under it, the two-clip mix's ends are the
    two clips, a joint is placed by its parents, and a file that is not a clip is refused.

The generated inputs are an LCG, so the counts above are reproducible; the three oracle channel tests
build their key arrays in emulator RAM and call the game's own allocator-shaped structs, not a
reimplementation of them.

## Not verified / not 1:1

- **The proofs are to a tolerance, not bit-exact.** The port is host `f32`; the PS2's multiply-add,
  divide and square root are not IEEE. Every number above is the worst deviation actually measured over
  that run, and `1e-6` relative is the gate. A bit-exact port would need the VU/FPU emulated, which is
  not what the renderer needs to look right.
- **The three tables are reconstructed, not ported.** Nothing in the image's call graph writes
  `motion_turn_table`, its narrow twin or the arc table — they are built at start-up by code Ghidra
  does not reach from the clip path — so the port rebuilds them from `sin`/`cos`/`acos`. They agree with
  a save state's copies to 1.79e-7 / 1.19e-7 / 1.08e-5, which is proof that the formula is right and
  not proof that the bytes are identical. The arc table's last two live entries (0.00048828125) are not
  arc cosines of anything and are reproduced as literals.
- **Channels shorter than the original's own search are deliberately different.** Both rotation
  samplers have a two-key path that weights the pair from the float *in front of* the key array — the
  allocator's bytes. It is reachable: two rotation channels on the disc have two keys
  (`tracks\free10_ss\anim\sh_windsock.mot`). The port weights those from the first key, which is a
  defined value instead of a heap byte. No position channel on the disc is short enough to reach the
  equivalent path.
- **Times before the first key are not proved.** The original brackets `keys[-1]` there; the port holds
  the first key. The generated times in the channel proofs exclude that range for that reason. No clip
  on the disc has a channel whose first key is after zero, so the game never asks.
- **Duplicate key times are not proved.** Both sides divide by zero and disagree about the sign
  (±FLT_MAX against NaN). The generator makes times strictly increasing; the disc-wide test asserts
  that every channel on the disc does too, so this is unreachable in play.
- **The degenerate position path is ported as written but not proved**: with a channel off or fewer than
  two keys the original writes the key's **z into x** and never writes z at all. No channel on this disc
  has fewer than six keys, so there was nothing to call it with.
- **Versions 1 and 2, and the 40-byte position key, are not ported.** `motion_clip_read` reads all
  three; no member of any archive on the disc uses them. `Clip::parse` refuses them with a message
  rather than guessing.
- **Mirroring and the channel stride in the blend entries are not ported.** `motion_blend_one`'s
  `param_5` (run the pose through `transform_rotate`, the mirrored rider) and `param_9` (take every
  other channel) are dropped; nothing in S6's driver sets either.
- **Blend mode 3** (three clips) is unported here as it was in S6 — no arm of the rider's switch sets it.
- **The motion object itself is not ported.** The joint records at `motion+0x234`, the two blend buffers
  a restart snapshots into, the channel on/off masks and the transform nodes the poses are pushed into
  are the plumbing around this task's arithmetic. The port's `Pose` is a flat pair of arrays instead,
  and the only proof that the plumbing matches is that the offsets are written down in `oracle.txt`.
- **V4's skeleton does not exist yet.** `Skeleton::of` is built from the `.psg` node tree, which is what
  the rigid skinning actually uses (one node per vertex, no weights), so the posed rider above is a real
  rider — but when V4 lands its own skeleton this is the seam to check.
- **Nothing was compared against a frame.** No screenshot, no PINE recording, no save-state capture of a
  posed rider's joint matrices. The arithmetic is proved function by function and the data is proved
  against the whole disc, but "the backflip looks like a backflip" is not proved, and an ordering
  mistake between the sampler and the transform chain would survive every test here.
- **The emulator was not driven.** `./kit pcsx2` was never started for this task; every oracle proof is
  the function oracle on the boot image.

## Next

For whoever takes the renderer's draw side: `Skeleton::world(&Pose)` hands back one row-major matrix per
joint and `skin(model, &world, group)` the triangles, so the missing piece is a frame to compare them
against — a save-state capture of the joint matrices at `motion+0x234 +0x34` while a clip plays would
turn the last bullet above into a proof. For V4: reconcile `Skeleton::of` with the real skeleton when it
exists; the names and parents are the only two things this module asks of it.
