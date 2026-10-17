# 1 — the pose it fades from (FINAL)

Task V15. 2026-10-17.

## What I did

V12 proved the port's animation machine against a minute of the running original, but 248 of its 1499
ticks could not have their **pose** compared: on those the machine's cross-fade ramp (`+0x68c`) was
still above zero, so the original's joints were a *fade* between the clip sample and a pose snapshot
the capture did not carry, and the port had nothing to fade against. V15 widens the capture to carry
that snapshot and compares those ticks too.

1. `watch/rider-joints.toml` gained the snapshot: three fields (`snap_rot_buf` +0x248,
   `snap_pos_buf` +0x24c, `snap_tagged` +0x254) and **two** `[[array]]` blocks, `ur` (22
   quaternions, stride 0x10) and `ut` (22 translations, stride 0x10), each hung off its own buffer
   pointer with the spec's own `- 0x400` arithmetic. 44 more quadwords a row, 2088 bytes a frame in
   one batched read of up to 48 regions.
2. Re-recorded the human's lap against it: 1500 rows, frame counter 2774..4273, 0 gaps, 0 skipped,
   0 null samples, 0 torn reads, 60.5 s of wall time (24.8 Hz against a 30 Hz game — the reads
   throttle the emulator, which matters below).
3. `crates/game-sim/tests/motion.rs`, section 11's second test: the renderer's own `Set::under` is
   seeded from the capture's two arrays every tick, so the ramp ticks are posed and compared like
   any other instead of being skipped.

**It came out.** 1322 of the 1499 ticks are now posed and compared joint by joint, against V12's
1117, and **104** of them are a genuine fade out of the original's own snapshot — worst lane
**1.4e-6**, the Pelvis translation at tick 3177. The machine and the clocks are unchanged: every
state, ramp, rate, axis and clip time within tolerance on all 1499 pairs, worst 0e0 on the
transition clock. Two arms of the renderer are measured against the original for the first time: the
four-clip blend (112 ticks, worst 7.7e-7) and the free-running advance on the wreck's own clips
(90 ticks, worst 8.5e-5).

## What the original does

**The snapshot.** The restart hook (`./kit fn 0x001461e0`, boot overlay) allocates
`joint_count << 4` bytes twice and fills both out of each joint record's own transform node at
+0x34: `+0x248` gets the node's rotation quaternion, `+0x24c` its translation with the `w` lane
written as 1. `+0x250`/`+0x254` are a tag buffer and a flag, and every one of the five call sites in
this image passes a null tag array, so the buffer is never made and the flag is 0 on all 1500 rows
(the test asserts it). Both buffers are allocated on the first restart and reused for good: their
pointers are constant across the whole recording (0xe1f770 and 0xe1f8e0 on this save state), which
the capture carries as a check that nothing moved underneath it.

**What the ramp does with it.** On a blend entry that fades, the sample is pulled back towards the
snapshot: the rotation by a slerp towards `+0x248`'s quaternion, the translation by a lerp towards
`+0x24c`'s — which is what `game_render::skin::Clip::lay` already did with `Set::under`. Proved on
104 ticks across three entry points (one clip, two clips, four clips) to 1.4e-6.

**The free-running advance does not fade.** 114 of the 218 ramp-up ticks are mode 0, and on those
the original draws its clip alone whatever `+0x68c` says — ramps as high as 0.99, agreeing with the
port's unfaded `draw_free` to 8.5e-5 over 90 drawn ticks. So `+0x68c` being up is not by itself a
fade, and the port's omission there is the original's behaviour and not a gap.

**The snapshot is the node tree, not the pose.** At tick 3260, the frame a snapshot was taken, the
snapshot's *rotations* equal the joints' live `+0x50` to the bit on every joint — but its
*translations* equal the live `+0x40` only on the Pelvis. On the other 21 joints the pose's
translation is zero (no clip of this set translates them) while the snapshot holds the node's own
local translation, 0.28 and 0.69 and 0.96 away. The original reads the node; the port's
`Set::rewind` copies the live pose. For every joint a fade can touch the two agree — a joint is only
faded where the clip has a channel for it, and there the snapshot is the pose — which is why the
fade still matches to 1.4e-6. See the not-verified list for what that leaves open.

**Three more behaviours this capture is the first to see**, all of them in what the port draws and
none of them in the machine:

* **The frame a clip-to-clip transition ends on still poses the transition.** `+0x80` is already 0
  when the row is read, but the joints in the same row are not the free clip's: on all three of this
  recording's endings they are the port's own pose 2.6e-3 away *with every lane's sign flipped*,
  which is the outgoing clip's quaternion still in the mix. The port has no second clip there, so
  those three ticks are skipped with the rest of the transition.
* **The frame a wreck is declared, the animation does not step.** At tick 3259 `+0x748` goes 0 -> 5
  and the request block moves while the machine, both clocks, both lag records and all 22 joints
  hold still to the bit. The next frame then does two latches in one (35 -> 30 -> 40, with `+0x8ec`
  left on the intermediate 30), which is why a one-step comparison cannot use either pair.
* **The pose grid's duplicated back column reaches three bands, not one.** V12 found that this
  disc's `turn_still_Bck` is a byte-identical duplicate of `turn_still_Mid` and that the original
  poses something 0.61 of rotation away; it saw it on 27 ticks of the two-clip band. This recording
  also reaches that row through the four-clip blend of the interpolating bands either side, where
  the same back column comes in with the weight the interpolation gives it — and the port is off by
  exactly that weight times the same distance, from 1.7e-4 where the mix axis is barely past centre
  to 1.17 where it is all the way over. One cause, three bands, 47 ticks.

## How it was proved

* **A PINE recording compared frame by frame.** `context/capture/v12/rider-joints.csv` (game data,
  not in git), 1500 rows of the human's `first_recording.p2m2` replayed in PCSX2 over AT-SPI, taken
  with the spec above. `./kit check` runs the two tests in `crates/game-sim/tests/motion.rs`
  section 11 against it: the sampler alone on the single-clip ticks, and the whole machine stepped
  one tick ahead of the capture on all 1499 pairs. Full suite green, 506 tests.
* **The snapshot's layout before spending emulator time**, off a save state with
  `./kit state extract`: both pointers non-null, 22 unit quaternions, translations with `w` = 1, the
  tag buffer never allocated.
* **What the test now says of itself** (one run, `--nocapture`): 1322 ticks posed, worst lane 8.5e-5
  (joint 6 `ShoulderL`, the wreck's clip a millisecond from its end, in the unfaded free-running
  arm); 104 fading ticks, worst 1.4e-6; 112 four-clip ticks, worst 7.7e-7; blend modes
  free/one/two/-/four/pair = 113/96/1071/0/112/0; not posed 177, every one of them counted under a
  named reason and bounded by an assert.

## Not verified / not 1:1

- **The port's own pose snapshot is never exercised by this test.** `Set::under` is overwritten from
  the capture on every tick on purpose — the question asked here is whether the port's *blend* is
  the original's, not whether its snapshot is — so `Set::rewind` is still unproved. It is also not
  1:1: it copies the live pose, and the original reads each joint's transform node, so on the 21
  joints no clip of this set translates the port would hold 0 where the original holds the node's
  local translation (up to 0.96 away). No clip in this recording translates those joints, so the
  difference never reaches a pose here; a clip that does would diverge. Fixing it needs the node
  tree the port keeps elsewhere, which is a renderer task and not this one.
- **105 pairs of 1499 are not compared because the recorder's sample phase slipped.** The reads hold
  the emulator to 24.8 Hz against a 30 Hz game, and in the hundred frames after the first wreck the
  frames ran long enough that nine reads landed *before* the frame's animation step and took the
  previous frame's work a second time. A slip that is not repaid on the very next frame leaves pairs
  with two steps in them and no repeated row to find them by, so the whole stretch between the first
  and last reread (ticks 3603..3706) is skipped. The test bounds it at a fifteenth of the recording
  and says to re-record slower if it grows. Not re-recorded here: the capture's phase is consistent
  on the other 1394 pairs, and a re-record at a lower emulator speed risks landing the sample on the
  other side of the animation step for the whole run, which would invalidate the seeding every
  comparison in this section depends on.
- **Two pairs are not compared because the game did not step the animation** (the wreck's first
  frame, above). The port steps every frame; nothing here models a frame the vehicle skips.
- **47 ticks of the pose grid's middle row past centre are not posed** — the duplicated back column,
  characterised by V12 and widened by three bands here. Still a loader question: whatever the
  original's third column points at is not in the set this port loads. On the plan.
- **23 ticks of clip-to-clip transition are not posed**, now including the three frames a transition
  *ends* on. The port draws no second clip for a transition at all, so neither the body of one nor
  its last frame is compared; what the original does on that last frame is characterised above but
  not ported.
- **The pose tolerance has less headroom than V12 measured.** 1e-4 was five times the worst
  disagreement over 1117 ticks; over the 1322 this capture draws the worst is 8.5e-5, a factor of
  1.2. It is one arm — the wreck's own clips in the free-running advance, at 0.9999s of a
  one-second clip, where the two decoders reach the last key from opposite sides — and every other
  arm is still at 2.2e-5 or better. The bar has not moved and the test's doc comment says this.
- **`+0x8f8` is still unread**, as in V12: nothing in this minute reaches the arm that reads it (all
  three cross-fade phases are zero on all 1500 ticks), so `Params::start_phase` is fed zero and the
  comparison does not depend on it.
- **One recording, one rider, one track, nobody watching.** The capture is the same minute of
  `first_recording.p2m2` V12 used, replayed over the accessibility bus with no human looking at the
  screen; it is a different *window* of it (counter 2774..4273), which is why it contains two wrecks
  V12's run never had. Nothing here is proved for a second rider, a second track, or the AI's
  riders, and no screenshot was compared.
- **The capture is game data and is not committed.** `context/capture/v12/rider-joints.csv` and its
  `.json` stay out of git; both tests print a SKIPPED line and return when it is missing, so a
  checkout without it still passes `./kit check` and proves nothing about the pose.

## Next

Nothing waiting on this task; the pose snapshot is captured, compared and bounded. What it turned up
for someone else, in the order I would take it:

1. `Set::rewind` to snapshot the node tree rather than the live pose, so the port's own snapshot is
   the original's on all 22 joints (first not-verified item above). With that done, the same test
   can stop seeding `Set::under` and prove the restart as well as the fade.
2. The frame a clip-to-clip transition ends on — the original still poses the transition there, with
   the outgoing clip's sign. 3 ticks of 1499, and the port has no arm for it.
3. The frame a wreck is declared: the vehicle skips the rider's animation step, and the next frame
   latches twice. Nothing in the port skips a frame.
