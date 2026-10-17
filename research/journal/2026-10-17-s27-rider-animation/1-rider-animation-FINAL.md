# 1 — rider animation (FINAL)

Task S27. 2026-10-17.

## What I did

Ported **the rider's own animation state machine** — the function in *front* of the step S25 ported,
which is what decides the state that step is handed. Everything in a rider's motion file that the
bike has no reason to ask for lives behind it: the gate's get-ready sequence, the wreck family, the
reverse pair and the look-around idle.

* `crates/game-sim/src/rider_anim.rs`, new, 462 lines. The selection (`Body::select`) in the
  original's order, the whole body frame around it (`Body::frame` — select, the step, the clip the
  step asked to be put on, the library call, the latch), and the pieces the selection needs that
  nothing else owns: `Pool`, the ten get-ready idles' no-repeat list and the generator it draws
  with; `Clock`, the CD clock's four BCD bytes the generator seeds itself off; `Params`, the body's
  three numbers; `Ask`, the half of the request block only the selection reads.
* `crates/game-sim/tests/rider_anim.rs`, new, 835 lines, four proofs — the selection against the
  retail image arm by arm, and the three behavioural ones the task asked for in words.
* `crates/game-sim/src/rider.rs`, +151 lines, all additive: the body, its numbers, its idle pool and
  the request block as fields, and `Rider::body_frame` — the second entry point that publishes the
  request off the rider's own fields and runs one body frame. `Rider::frame` is untouched, and so is
  `impl State for Rider`'s `walk` (a doc comment aside), so every replay hash in the crate is
  unchanged.
* `crates/game-sim/src/lib.rs` declares the module. `research/oracle.txt` gains an S27 block; its
  four new symbols are `rider_animation`, `rider_idle_pool`, `rider_idle_count` and `rand_seeded`.
  `PLAN.md`'s code map gains the module and the new entry point.

### Two corrections to S25, both proved

- **The actor is the rider's body, not the vehicle.** The spawn that allocates it
  (`./kit fn 0x0020e4f0`) hands it the prefix `"rider_"` or `"ff_rider_"` — the two modes the disc's
  rider archive spells `[Stunts]` and `[ff_Stunts]` — and the loader joins that prefix to the 24
  gesture names only, looking the other 36 up bare. Those 36 bare names are a rider's own clips (V8
  bound all 24 gesture slots and the 3 airborne poses under the rider's rule), so the sixty-entry
  table is the rider's and `vehicle_anim::POSE_CLIPS` is a list of rider clips. Every address, offset
  and arm in the S25 block is still right; only the word "vehicle" in it was wrong. I did not rename
  the module — other tasks are in flight and a rename is the one change that cannot merge cleanly —
  so the correction is in both blocks of `research/oracle.txt`, in `rider_anim.rs`'s header and here.
- **The byte S25 could not find a writer for is written here.** The tail of this function stores
  `+0x8f8` and the byte `+0x8fc` off the request, every frame, after the step returns: `+0x8f8` is
  *last* frame's progress through the gesture (S25 read it as a configured start phase) and `+0x8fc`
  is the request's own count. So the restart test S25 ported as "a count of two or more that differs
  from a byte nothing writes" is in fact "the count moved since last frame", and a repeat of the
  state already running restarts it on that frame alone.

## What the original does

The selection is `./kit fn 0x00248a70` — 2068 bytes, `(dt in f12, body in a0)`, a slot of the body
actor's vtable and the only caller of the step at `0x002492b0`. It is a chain of else-ifs; the first
arm that matches is the only one that runs, and the state it writes is `+0x8f4`.

1. **The gate**, while the request's cut flag is up. Four steps in one chain, in the order they
   happen: a wait counts down and the body breathes (state 46); the wait over, one of ten get-ready
   idles is drawn *without repeating* (47..56); that idle ended, the body stands up into one of three
   start poses (57..59) — or, if the throttle is open, the cut flag is cleared and the body rides;
   and whenever whatever was playing ends, back to the breathe with a new wait of three and a half
   seconds less the gate's own clock less the group's fade rate.
2. **The wreck.** Off the bike and not yet at rest, the body flails (40). At rest, and only if it is
   not already holding one, it grabs one of five limbs (41..45) and keeps it.
3. **A gesture**, when the request names a slot other than 24: that slot, or the single
   `nohandedlander` clip (39) when the request says the gesture is a no-hander.
4. Then the backflip (38), the airborne pose (35), and the reverse pair — which side of centre the
   bars are on picks left (33) or right (34).
5. **Riding, or standing about.** Below the standing-still speed with the bars centred, the rider
   centred and the throttle shut, a timer runs; when it expires the body plays one of the ten idles
   (a *plain* draw this time — it may repeat itself and does not touch the no-repeat list) and when
   that clip ends it goes back to riding (30). Otherwise the timer is re-armed from a unit draw
   remapped onto 15..45 seconds. Every frame, whichever arm ran, rolling forward faster than the pose
   grid's first threshold cancels the timer — the test is signed, so rolling *backwards* does not.

**Chance.** Four arms draw, each through the same lazily installed seed: 10 idles from state 47, 3
start poses from 57, 5 limbs from 41, and one bare unit for the look timer. The first draw anywhere
in the program reads the CD drive's clock (`./kit fn 0x0041a068`, a BCD `sceCdCLOCK`) and seeds the
engine's `rand` with `second + minute*60 + hour*0x168 + day*0x15180`. `0x168` is 360, not 3600 — an
hour contributes a sixth of an hour's worth of seconds, which is the original's own bug.

**A structural fact.** The states this function can ask for are *exactly* the states the step's jump
table has an arm for (0..23, 30, 33, 34, 35, 38..59) — never one of the ten (24..29, 31, 32, 36, 37)
that fall to the step's free-running advance. The one exception is that a request naming a gesture id
above 24 is copied into the state with no range test.

## How it was proved

`crates/game-sim/tests/rider_anim.rs`, four tests. `./kit check`: **green**, with the four new tests
in it. I am not quoting a total: three runs of the gate from this worktree reported 469, 476 and 471,
and `./kit check`'s log is one shared file in the main checkout that the other slots' concurrent runs
clobber, so the number it sums is not this worktree's alone. What is this task's is the four.

1. **the selection, one state at a time, against the retail image** — 30,000 generated frames through
   `ps2-oracle`, with the step behind the selection hooked away to a no-op so that what is compared is
   this function alone, steered arm by arm through all twelve arms of the chain. Bit-exact, per frame,
   on: the state asked for, the gate's wait, its drawn flag, the look timer and its flag, the motion
   object's finished flag, the cut flag the gate clears, the ten-word no-repeat list and its count,
   the generator's own 64-bit state — which is what proves the *number and order* of the draws and not
   just their values — and the two fields the tail latches. It also asserts the structural fact above
   on every frame, and that the ten arm-less states are asked for exactly zero times. Covered:
   3,267 frames with a random draw, 3,784 at the gate (518 of them ending it), 1,004 look-arounds
   fired, 9,528 timer writes, every reachable state more than 50 times.
2. **a body dropped on a bike gets ready without being asked** — the ported selection *and* the
   ported step, 600 frames, with the state asserted unchanged between every pair of frames so that
   nothing but the selection can have written it: `[30, 53, 46, 57]` — riding, a get-ready idle, the
   breathe, a start pose — and then opening the throttle clears the cut flag and gives riding back.
   The clip on the player is asserted to be the state's own on every frame.
3. **a wreck puts the body into a wreck clip** — riding, then off the bike: the flail clip for 60
   frames, then, told it has settled, one of the five limb clips held for 120 frames and never
   riding.
4. **the wiring** — the same gate sequence again, driven through a whole `rider::Rider` on the
   hand-written test bike (`tests/freeride-bike.phy.txt`, no disc needed) with the player's six-step
   update running in between every pair of body frames: the sequence comes out the same, the player
   update never writes the body's state, and each published field of the request is asserted to be
   the rider field it is mapped from.

(2), (3) and (4) need no disc and run everywhere; (1) skips with its reason without one.

## Not verified / not 1:1

- **Who publishes the request block is still unidentified**, as S25 left it. This function and the
  step both read it through the pointer at `+0x488`; nothing I followed writes it. So the *inputs* of
  the selection are proved frame by frame against the original, but what the original would put in
  them on a given frame of a race is not — including the cut flag that is the whole of the gate's
  condition, and the gate's own clock at `+0x144`.
- **The `rider.rs` mapping is the port's own**, for that reason: which rider field each input is
  published from (the table on `Rider::body_frame`) is my choice, not a port of anything. It is
  consistent — every trick-shaped input comes off the same latched id the frame's own animation
  header plays — but a capture of the original's request block would be the thing that settles it.
- **The body's mode machine is not ported.** `boot 0x0024c7e0` (3592 bytes) is what writes the
  "wrecked body has come to rest" flag at `+0x6b4` that the wreck arm reads, and it also drives the
  head and look dummies the task's brief mentions. So a body wrecked by this port flails until a
  shell says it has settled, and nothing here turns a head.
- **The step's fade rates and the pose grid's thresholds have no located loader** — the five rate
  fields at `+0x694..+0x6a4` and the three thresholds. `Rider::new` therefore leaves the whole block
  at zero (every fade a hard cut, every row of the grid meeting at zero) and says so where the field
  is declared; the test fills it with plausible numbers of its own. Finding the loader is a task of
  its own and the numbers in it are not claimed to be the disc's anywhere.
- **The wall-clock seeding is not reproducible and is not ported as a global.** The original's first
  draw reads the CD clock and installs a seed shared by the whole program; the port takes the clock
  as an input (`Clock`) and gives the body its own `Pool`. The seeding *path* is proved (the test
  seeds the original by hand and tracks it), but two things follow: the port does not reproduce the
  draw order of a program where other subsystems draw from the same generator, and nothing here can
  reproduce a particular machine's clock. The 360-instead-of-3600 hour multiplier and the BCD bytes
  are recorded in `research/oracle.txt` rather than coded around.
- **The clip durations and loop flags in the behavioural tests are the tests' own.** One second each,
  looping for the clips a body is meant to sit in. Which clips of a rider's set really loop is a
  question for the disc, and the gate's shape depends on it: a start pose that *ends* sends the body
  back to breathing, which is the machine working as written and not what a start line looks like.
- **A request naming a gesture id above 24** is passed through into the state with no range test, so
  the step gets a state with no arm. Ported as the original does it, excluded from the structural
  assertion, and noted here because it is the one hole in "the selection only ever asks for a state
  the step can serve".
- **Nothing here was compared against a frame of the running original.** No PINE recording, no
  capture: the proof is a function oracle plus three behavioural fixtures. What a real gate looks
  like on the disc — which idle, how long, what the start pose settles into — is unproven.
- **The body's state is deliberately outside the replay hash.** `impl State for Rider` does not walk
  it (V8 made the same choice for the animation header), so a replay fixture would not catch a
  divergence in it. That is on purpose — adding it would move every hash in the crate — and it means
  the only guard on this module is its own tests.
- **Housekeeping, not the port:** this worktree's `context/capture/acceptance-freeride.csv` was a
  different run of the human's minute than `tests/replay.rs` expects, which reddened that one test
  before I started. I set it aside in `context/capture/other-start-19-44/` and copied the main
  checkout's capture of the same recording in. Nothing under `context/` is committed.

## Next

Nothing outstanding for S27. Three things a later task could take up, in the order they would pay:
the publisher of the request block (which would close S25's and this task's largest hole at once),
the body's mode machine at `boot 0x0024c7e0` with the head and look dummies, and a loader for the
step's eight numbers.
