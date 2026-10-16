# 1 — p3 tricks in the shell (FINAL)

Task P3. 2026-10-16.

## What I did

S10 (the airborne step and its wreck test) and S11 (the trick machine) were proved ports that had
never once run on a bike. P2 left a bike that rides but has nobody on it: nothing turned a gesture
into a stunt, nothing ran a stunt's clocks, and nothing ever decided that the rider came off. P3 is
the joining, in one new module and one new field.

**`crates/game-sim/src/rider.rs`** — new. `Rider` holds P2's `Ride`, S11's `Tricks`, S5's
`state::Ride`, S6's `motion::Driver`, the rider's 24-stunt `Set` and the session's `state::Sense`.
`Rider::frame` is the original's ridden-actor order (A5):

1. `state::Ride::frame_start` — the hand-reset latch drops.
2. `Ride::frame`, **only while the rider is not down**, with `respawn` taken out of the controls it
   is handed: the reset is the machine's to order now.
3. the input phase: `Gesture::read` over the rate-limited stick and the three trick buttons, then the
   pad's reset chord through `chord_allowed` / `take_chord` / `request_reset`.
4. `Tricks::request`, and `Tricks::idle` on the frames it answers no — with the original's gate in
   front of the idle only on the riding branch.
5. `Tricks::update` (the clocks), then `motion::Driver::frame` on the *same* `Request` built before
   the clocks ran, because the original works the restart predicate out once at the top and both
   halves read it. The header is what writes `shown_crossup`, which the landing is judged on.
6. `state::Ride::step`, carrying `air::wreck_test` as its `test` closure.

`Rider::frame` answers a `rider::Frame`: the trick that started, the trick that landed, the reason
the rider came off, and whether they were put back. That struct is the port's own — the original
tells the camera, the announcer and the race record instead.

**`crates/game-sim/src/ride.rs`** — additive. `Controls` gained `trick: [bool; 3]` and
`dpad: [bool; 4]` (the trick decoder's own two halves, in the decoder's own order, which is not the
eight-way code `dpad_dir` already carried), filled by `Controls::from_pad`. `Ride` gained one field,
`stunt`, which is the trick clocks' "in progress" word: the original's airborne control and its
brake-gyro branch read that word off the bike, and the clocks run *after* the ground step, so what a
frame's air code sees is the previous frame's verdict. Before this the ride read the Circle button
there, which is not the same thing. `noise` became `pub` (the hold's wobble needs it) and the reset
chord moved off Triangle, which is now a trick button.

**`crates/openmx/src/main.rs`** — the shell holds a `Rider` instead of a `Ride`. It reads the rider's
stunts off the disc at start-up (`appstart.res` → `rider\anim\stunts.ini` → `Stunts::parse` →
`selection(0)` → `Set::new`: 57 records on the disc, 24 this rider can reach), prints each trick,
landing and wreck as it happens, and shows the phase on its once-a-second line. `--no-wrecks` turns
the session's wrecks off — see the last section for why that had to exist.

## What the original does

**The frame order.** `research/oracle.txt`'s A5 block has the riding branch of the player update and
its vtable slots: the ground step runs only while the wreck reason is clear, then the air state, then
the attachment walk, then the stunt idle, the stunt state pack and the stunt clocks, then the
sequence copy, and last the ride state machine — which is where the wreck test is asked. The gesture
is read in the input phase, ahead of all of it, gated on the control source being a local human.

**One wreck test, not two.** `air::wreck_test` (S10) and `Tricks::landing` (S11) are two ports of the
same tail of the same function, made in two tasks from two directions, and they disagree about one
parameter. Both read the bike's `+0x780`: S11 reads it as a *speed* and compares the bike's speed
against it; S10 reads it as the cosine of a *roll angle* and compares the bike's up axis against it.

S10 is the one that is right, and the disassembly settles it rather than the balance of probability:

- **the word the clause reads is not a speed.** `+0x420..+0x42c` is a four-float vector, and the bike
  constructor fills it from a constant quad that reads `0, 1, 0, 1` — the up axis — in the same
  breath that it fills `+0x410..+0x41c` from `0, 0, 1, 1`, the forward axis. `+0x424` is that
  vector's *y*. The clause compares how upright the bike is;
- **the parameter it compares it to is an angle, already cosined.** S18's loader — proved against the
  original — fills `+0x780` from `NoHanderMaxRollAngle` and stores it passed through the game's
  cosine table, "because what reads it has a dot product in its hand". Now we know what the dot
  product is;
- **the speed limit is spent elsewhere.** `+0x77c`, the key before it, is `NoHanderWreckSpeedLimit`,
  and it goes one clause higher up against the vertical g load at `+0x554`, behind the same
  no-hander flag.

The whole of that is written into `research/oracle.txt` as the P3 block, with the addresses, and
`trick.rs`'s `Params::no_hander_speed` and `Tricks::landing` now carry a doc note pointing at it.

So the rider calls `air::wreck_test` only. It is the whole of the test — the fore-aft and sideways g
loads, the three vertical limits, the ground and airborne attitude limits, the backflip-into-
something, the crossup landing, *and* all three trick endings, in the original's own order — and the
trick machine's state reaches it through `air::Loads`. `Tricks::landing` is left where it is, proved
and unused, rather than deleted: it is a correct port of two of the three endings and the evidence
for the third is in this entry.

Two more notes on that hand-off, both of which read oddly and are both the original's:

- `air::Loads::trick_wrecked` is named for the verdict it feeds, but what the original reads there is
  the trick machine's `safe` — "this one can be landed now". With a stunt in progress, the bike on
  the ground and the rider's hands on the bars, a stunt that is *not* yet safe is a wreck.
- `air::Wreck::NoHander` is `0xc` and `trick::Wreck::TRICK` is `12`. They are the same reason code,
  which is why the two ports could disagree about the clause and still agree about its answer.

**The two reads that are the port's own.** `air::Loads::pose` / `pose_wanted` compare the trick *id*
against `Set::throwaway`, not the slot — `trick.rs` documents why, and the rider passes the id
through unchanged. `air::Step` reaches the wreck test with only three of its fields filled (`up`,
`stunt`, `touching`); the rest are the airborne control's and are already spent by then.

## How it was proved

**`crates/game-sim/tests/trick.rs`, proof (9)** — `a_scripted_pad_lands_one_trick_and_wrecks_another`.
Not an oracle proof: every clause it crosses is proved against the original already (the clocks and
the landing in (6) and (7) of the same file, the wreck test in `tests/air.rs`, the stunt table in
(1), the ride in `tests/ride.rs`). What it proves is the wiring, which is the only thing P3 added.

Two drops onto flat ground off one hand-written bike (`crates/game-sim/tests/trick-bike.phy.txt` —
the ride's own test bike with the four sections a rider needs; no number in it is the disc's), with
the same stunt and the same gesture, differing only in the frame the gesture is made on. The stunt
and its gesture are *not* written in the test: it takes the shortest of the rider's 24 that is
neither the no-hander nor the throw-away, straight off the disc's `stunts.ini`, and works back from
its slot to the group-and-direction that names it. On this disc that is `cancan`, 1.33 s long, safe
at 1.13 s, group 1 and left.

- asked for on frame 2 of a 164-frame drop: the clock runs the whole 1.33 s in the air, the wheels
  come down with nothing in progress, `Frame::landed` names the stunt, the phase is still `Riding`
  and the wreck counter is zero.
- asked for on frame 162: the clock has 2 frames on it when the wheels touch, nowhere near the safe
  time, and the wreck test answers `Wreck::TRICK`. The phase is `Down`, the counter is 1, the
  requested trick is back to `NONE` and the queue is empty.

A third flight with nobody touching the pad is the control: no wreck, no landing, and the same
landing frame as the flight that landed a trick, which is what says the stunt did not move the bike.

**`crates/game-sim/tests/ride.rs`** — the walk guard moved from 471 leaves to 472 and its last path
from `touching` to `stunt`, which is the new field being hashed. `size_of::<Ride>()` did not move:
the bool fits in padding.

**The shell, on the disc.** `openmx --seconds 8 --no-wrecks` rides free06 on the 125sx for eight
seconds at 59–60 fps, having read 57 stunts off `appstart.res` and reduced them to the 24 the first
player's gesture map reaches. `./kit check`: 441 tests.

**Not proved live.** Nobody made a gesture on a real pad: this desktop has no GUI automation (no
working focus dispatch, no `ydotool`), so driving the shell's window by hand is a human ask. The
scripted fixture is the stand-in, and it drives the same `Controls` the shell builds from a pad.

## Not verified / not 1:1

- **The ported ride's g loads oscillate, and with wrecks on the shell is unrideable.** This is the
  finding of the task and it is not P3's to fix. P3 is the first thing that has ever read
  `move::Ride::g_peak`, and on the disc's own 125sx on free06 the sideways load alternates sign and
  grows frame over frame while the bike idles forward off the start line — about 0.5, 1.0, 1.5, 6.6,
  11.7, 15.4, 18.3 over the last dozen frames — and crosses the bike's own `SideWreckSpeedLimit`
  (16.87) about 2.5 seconds after the spawn, standing still. That is a divergence in P2's ground
  step, not a wreck: the wreck test is oracle-proved, the limit comes from S18's proved loader, and
  `g_peak` itself is proved in `move.rs`. What has never been compared against the original is the
  acceleration the port's tire and suspension solver feeds it, and three of the things that feed it
  are P2's own stand-ins (the tire patch forces, the wheel's spin integration, the steering hinge).
  `--no-wrecks` exists so the shell can still be ridden; it sets `state::Sense::wrecks_enabled`,
  which is the original's own per-session flag, and it defaults to *on*. **This wants a task of its
  own**: compare the port's per-substep `local_accel` against a live capture of `+0x550` and find
  where the solver is ringing.
- **A freeride session's own `Sense` is guessed.** `Rider::new` sets a local human, no AI route,
  wrecks on, and no settle window. The human and the route are what a one-player shell is; the
  settle window is S5's finding that a bike's class never asks for one. Whether the *freeride* mode
  turns wrecks off at the session level was not read out of the original — the flag is ported, the
  value the front end gives it is not.
- **A down rider's air state does not run.** The original skips only its ground step while a wreck
  reason is set and still runs the air state; the port skips `Ride::frame` whole, because that
  function fuses the two and splitting it would duplicate about 25 lines of step construction into
  this module. The practical effect is nil — once down, `state::Allowed::landing` is false and
  nothing the air state would write is read again before the respawn — but it is not 1:1.
- **The gesture is read after the physics, not before it.** The original reads the pad in its input
  phase, ahead of the update, off the stick its rate limiter has just written. The port's rate
  limiter lives inside `Ride::frame`, so the read had to be on one side or the other; after it is
  the side that sees the same frame's stick the original would have seen. Nothing between the two
  points reads a gesture, so the only visible difference would be a frame where the chord fires: the
  bike has already stepped at the old place that frame.
- **The wreck entry is only half ported.** `state::Ride::enter` does the machine's half and the
  rider does one line of the caller's — the running trick stops and the queue empties. The rest of
  the original's entry is not ported and is not this task's: the rider is placed at the bike's rider
  point with the bike's drop velocity (three draws from the shared random sequence for a cannonball),
  the wheel loads are zeroed, and the camera, the rumble and the announcer are told.
- **`Frame::landed` is bookkeeping, not a port.** "The trick that was running when the wheels came
  back down, on a touchdown that was not a wreck" is this module's own definition, kept so the shell
  can print something and the fixture can assert on it. The original counts a landed trick in its
  scoring, which is a different task and a different rule.
- **`motion::Driver` runs for one field.** The rider has no animation library and draws nothing, so
  `Driver::frame` is run only because it writes `shown_crossup`, which `air::Loads::landing_crossup`
  reads. Everything else it answers — the blend, the clip selection — is dropped. With no stunt
  playing it is the airborne idle arm that writes that field, and the port has not checked that the
  shown crossup a real rider's animation set produces matches the one the blend computes here.
- **`trick::Tricks::landing` is left live and wrong in one clause.** Its third way — the no-hander —
  compares the wrong word against the wrong parameter, and that is now proved, not suspected. The
  function is not called from anywhere, its other two ways are right, and its tests pass, so fixing
  it means changing a module another task proved and re-cutting its proofs; that is a task, not a
  footnote to this one. It carries a doc note saying so.
- **`rider::Tables` builds the arc-cosine table twice.** The air code's `AcosTable` and the trick
  decoder's `AngleTable` are the same function on different grids with different lookups, so both
  are built. Whether the original really has two is not known — it has at least the trick one, which
  (2) in `tests/trick.rs` proves entry for entry.
- **Which host button is which is the shell's choice.** The disc's binding table is in its front end
  and is not ported. What *is* the disc's, and is kept, is the *order* of the three trick buttons,
  because the order is what numbers the six groups and so which of the 24 slots a gesture names.
  Circle, Triangle and R1 are the shipped pad's three; the reset chord moved to L3-and-R3 together
  (the original's is two buttons too, but not these two) because Triangle is now a trick button and
  Select is unreachable on this shell's joystick path; the camera presets lost R1 and cycle one way
  on L1.
- **The fixture bike is not the disc's.** `trick-bike.phy.txt` is hand-written, like `ride.rs`'s, and
  its wreck limits are deliberately wide so that the clause the test is about is the one that fires.
  It also runs at a third of the test bike's gravity, so that a drop long enough to hold a 1.33 s
  stunt does not land hard enough to reach the vertical-g clause. No value the fixture asserts comes
  from those numbers.
- **No gesture has been made on a real pad.** See the last paragraph of the previous section.

## Next

This file is -PART only in the sense that the shell has never been ridden by a human with a pad in
their hands. Everything in the task line is done and proved as far as this machine can prove it.

The one thing that should happen next, and is not P3's: **a task for the ride's g loads.** With
wrecks on, the disc's own bike wrecks itself on the start line in two and a half seconds, which means
P2's ground step is ringing. The way in is a live capture of the bike's `+0x550` over the first few
seconds of a freeride spawn, compared against the port's `move::Ride::g_peak` frame by frame; the
replay harness already walks that field.

After that, a human with a pad should ride it and try the 24 gestures, because nothing here has
proved that the group-and-direction a player's thumbs make is the stunt they expect.
