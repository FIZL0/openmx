# 1 — the animation driver (FINAL)

Task MD65c. 2026-11-03.

## Read

- **PS2:** I read `./kit fn 0x004be3c0 -p atv` and the asm (`./kit dis 0x004be3c0 0x004be8cc -p atv`).
  The asm settles the float ops: the fade is `adda(0, fade) msub(rate, dt)`; the off-pose crossup blend
  is `adda(0, 0.5) madd(0.5, lean)`; the riding pose's blend is `adda(0, 0) madd(t, 1 − 0)`. Every
  weight is clamped with `max.s 0` then `min.s 1`. The mode register only ever gets 1 or 2, so the
  controller's own update (mode 0) and the four-argument blend (mode 4) cannot be reached.
- **PC:** I read `./kit fn MXVehicleAtv::vf67 -o pc`, and it has the same clauses. The riding pose's
  crossup blend there is the generic map-clamp (0x0040ab90). The non-pose branch skips the clamp,
  which makes no difference because 0.5 is already in range.
- **Callees:**
  - The stunt lookup 0x00268c90 is `set + 0x18 + 12·id`, and 0 for id ≥ 46.
  - 0x0013e650 sets the controller to a null clip.
  - 0x00266d30 moves the attachment's box in the spatial database.
  - The base 0x004a8350 is the biplane's driver. In vehicle states 3/4 it zeroes the world angular
    velocity to (0,0,0,1) and, if vt +0x3e8 answers, runs the net unpack.

## Ported

`quad::animate(&mut Anim, &mut PutBack, &mut Quad, &mut Body, dt, &mut impl StuntClips, &mut impl Moto)`:

- **The fade (+0x14bc):** run down by its rate and clamped to `0..1`. When the animation is stopped
  it goes back to one. The animation is stopped when no put-back is pending (`PutBack::request`, +0x31c,
  is 0) and either the stunt changed or the unwreck's word +0x1650 is set.
- **The stunt played (+0x1524):** copied from the stunt steps' next one (+0x1528), and
  `Quad::balancing` becomes `stunt == balance_stunt` (+0x152c). That is the stand-upright's guard.
- **The pose blends (+0x14b4 crossup, +0x14b8 fore-aft):** written outside vehicle states 3 and 4.
  For the riding pose (46), `PutBack::settle` (+0x141c) is set to the crossup blend.
- **The clip table:** stunt ids 0..45 are trick clips. The riding pose's three clips (+0x15ec/+0x15f0/
  +0x15f4) are entries 46, 47 and 48 of the same table at +0x1534, so a clip in a `Call` is a place
  in that table (`POSE_CLIPS`).
- **`StuntClips`:** answers whether a stunt is ready (the set's record byte +0x59) and the clip's
  length. The length is asked only when it is needed. The original does not read the length when the
  animation was stopped, and the spawn image has stunts that are ready but have no clip, so reading it
  eagerly faulted.
- **The calls**, in order: `BaseAnimate(dt)`, `AnimStop`, `PlayClip`/`BlendClips`, `PoseNodes`,
  `ReadFrame(0/1)`, `SetFrame` per hung node (the put-back's loop), `AttachmentMoved`.
- **Skipped:** the body's two self-copies (+0x190, +0x80 with lq/sq) change nothing, so they are not
  ported.

Nothing was taken from opentennis: it has no counterpart to this.

## Checks

- `./kit check -- -p game-sim --test atv_quad`: TESTS PASSED, 56 tests (54 before plus the 2 new ones).
- I ran the two new tests with `cargo test … -- --nocapture` after copying `atv-r-spawn.ee.bin` from
  s2's context into this slot's `context/mxvatv-ps2/md37a/`. Before the copy, both tests skipped and
  still showed as passed.
- **`the_quads_animation_driver_is_the_originals`:** 20,000 generated cases. Every callee except the
  stunt lookup is hooked. The test compares:
  - the five words by bits;
  - the call log, printed so −0 and 0 differ, with clips compared as table handles;
  - the balance guard;
  - that the original wrote nothing else, except the body's self-copies and its stack (checked per
    byte).

  Each of these 10 paths ran at least 200 times: stopped, playing on, stunt clip, stunt clip from its
  start, stunt not ready, another id, riding pose, riding pose held, riding pose with no range (±0),
  middle pose held.
- **`the_spawned_quads_animation_driver_is_the_ports`:** runs on the image's own quad (0x015e0690) and
  its real stunt set and clip table. Each next-stunt id 0..=60 and 255 is tried with request 0/1,
  vehicle state 1/3 and dt 1/60 and 1/30. Paths seen: 36 stunt clip, 48 stunt clip from its start,
  272 not ready, 120 another id, 4 riding pose, 4 riding pose held. 12 cases were skipped as "ready
  without a clip".
- **Hand mutations:** each of these turned the tests red:
  - `<` → `<=` on the fore-aft half;
  - no settle write;
  - a stopped stunt keeping its weight;
  - no balance guard;
  - state 4 not held;
  - no restart word.

  Two survived, and both are value-equal rewrites: the fade as a separate `mul` then `sub`, and the
  off-pose blend as `0.5 + 0.5·lean`. The EE's madd/msub chop the product just as `mul.s` does, so
  the oracle cannot tell either from the original. I kept the accumulator forms as the asm has them.
- `./kit audit`: 4 leaks, none of them mine (atv_wheel.rs:62/66, plan/DONE.md:519/533).

## Not verified / not 1:1

- **The base driver is a Call.** `Call::BaseAnimate` stands for the biplane's driver, which is
  simulation in the held states (it zeroes the world angular velocity and runs the net unpack). It is
  not ported. Filed as MD65c1.
- **No caller wiring.** Nothing in the ride calls `quad::animate` yet, and `PutBack::request` and
  `Tail::request` still hold the same word (+0x31c) twice.
- **Negative stunt ids not driven.** The lookup is signed, so a negative id would read the clip table
  before +0x1534, which is the quad's own +0x1528..+0x1530 words. The tests only use ids 0..60 and 255.
  The port would hand the negative place on as it is.
- **Stunts that are ready but have no clip.** On the image these make the original read the length
  through a null handle, which faults the interpreter. Those 12 cases are skipped, not compared.
- **Generated and image cases only.** No capture of a ride's frames was taken, so the driver's
  inputs (stunt switches, put-backs) were never seen in sequence.
- **The render calls' own effects are not ported**, by design: the clip play and blend, the stop, the
  node pose, the frame reads and sets, and the attachment move.
