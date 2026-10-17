# 1 — blends that never run (FINAL)

Task V11. 2026-10-17.

## The answer

**Those two writers are not wired to the driver.** Nothing was missing from a select.

- The rider's own dispatch (`game_sim::motion::Driver::select`, S6/S20, oracle-proved) writes modes
  0, 1 and 2 and *has* no arm that writes 4 — not in the port and not in the original. V7 and V8 read
  that correctly.
- The machine that does write 4 and 5 is the sixty-state one S25 ported
  (`game_sim::vehicle_anim::Driver::frame`). In the port it is a leaf: nothing calls it but its own
  test. The function that decides which of its sixty states to run — the writer of `Driver::want` —
  is a method of the animated object itself (`./kit fn 0x00248a70`), and S25 explicitly left it out.
  No `want` writer, no state 30, no pose grid, no mode 4.

So the blend the original uses and the port never reaches was one function away, and this task ports
that function, proves it against the image, and draws what it asks for.

## What the original does

### The sixty states are the *rider's* clips, not the bike's

S25 called the object "the vehicle", from its own field map. The disc says otherwise, and it matters,
because it is what makes the missing caller findable:

- all 36 of the machine's fixed clip names (`vehicle_anim::POSE_CLIPS` — `turn_stop_*`,
  `turn_still_*`, `turn_riding_*`, `reverse_l/r`, `air_*`, `backflip`, `nohandedlander_left_right`,
  `wreck_air_flail`, `hold_*`, `get_ready_*`) are members of every **rider** archive on the disc and
  of **no bike archive**: `250sx.res` holds twelve `.mot` files, all of them `bike_*`;
- the names its first 24 slots build are `<prefix><stunt>` with no `bike_` infix, and the prefixes the
  image hands the body actor are `rider_` and `ff_rider_` (V8, `0x0042ead8` / `0x0042eb10`) — so the
  gesture slots come out `rider_backflip`, which is exactly what V8 found a rider's set holds;
- the state groups are poses a *body* has and a bike does not: an arm going back for the start gate,
  five hold-this-limb poses after a wreck, ten start-line idles and three start poses.

This is the rider's own animation state machine — the one V8's journal said was not ported and could
not say existed ("whether the original runs a second, rider-specific selection on the body actor …
is unknown"). It does, and this is it. I have not renamed `vehicle_anim`: the module's field map and
proofs are S25's and a rename is churn in a file three other tasks touch.

### The missing function: the state chooser (`./kit fn 0x00248a70`, 2068 bytes)

`(dt in f12, object in a0)`. It is the only writer of `want` in the image, it calls the animation
step itself (`./kit fn 0x002492b0`) in its own tail, and after the call it writes the two fields S25
could not find a writer for. In order:

1. **At the start line** — the request's at-start flag (`+0x140`, which is the same word the step
   reads as "cut"):
   - a breathe timer (`+0xa28`) still running: state 46 (`get_ready_breathe`), timer less `dt`;
   - else no start pose picked yet (`+0xa0c`): pick one of the ten start idles (47..56) **that no
     other body on the line has already taken** — a file-static list at `0x004e1fc0` with its count
     at `0x004e1fe8`, redrawn until the draw is not in the list, appended to it, and the whole list
     reset to the one new entry once all ten are gone — then mark it picked;
   - else the clip has not finished: if the state is 46, pick one of the three start poses
     (57..59); otherwise, if the request's commanded number (`+0x160`) is not zero, **clear the
     request's at-start flag** and ask for state 30. That is the start gate opening, and it is the
     one writer of that flag I found;
   - else (the clip finished): state 46 again, and the breathe timer set to `3.5 - request+0x144`
     less the start-line fade rate. So the start line alternates breathe → a random start pose →
     breathe until the throttle is commanded.
2. **Riding**, in this priority:
   - wrecked (`+0x104`): state 40 (`wreck_air_flail`) while the body is still moving, and once
     another method of the same object has set `+0x6b4` (it does so when the body's velocity drops
     below its own threshold, `./kit fn 0x0024c7e0`), one of the five hold poses 41..45, drawn once
     — `want < 0x29` is what keeps it from being redrawn every frame;
   - a gesture asked for (`+0x1a0` is not 24, which is the "none" id): that gesture's slot, 0..23 —
     unless the no-hands flag (`+0x100`) is set, which takes state 39
     (`nohandedlander_left_right`) instead;
   - flipping (`+0xfc`): state 38 (`backflip`);
   - airborne (`+0xf8`): state 35 (`air_Fwd`, the airborne row);
   - reversing (`+0x16c` not zero): state 34 when the phase axis is at or above a half, 33 below it;
   - otherwise the **idle wait**, and then state 30 — the pose grid, where both mode-4 writers live.
3. **The idle wait.** When the band value is under the still band (`+0xa10`, 1.46667), both axes are
   centred at exactly a half and nothing is commanded, a countdown (`+0xa30`) runs: when it goes
   negative the chooser asks for one of the ten start idles (47..56) as a *fidget*, raises a
   "playing" flag (`+0xa2c`) and clears the motion object's finished flag; when it is not running it
   is re-armed to a random 15..45 seconds (`+0xa34`, `+0xa38`, through the engine's own remap, whose
   degenerate-range guard Ghidra drops as unreachable because the range is 0..1). While the playing
   flag is up the chooser leaves `want` alone, so the fidget runs to its end and the flag is dropped
   on the frame the motion finishes. Any frame whose band value is above the grid's first threshold
   (`+0xa14`, 7.33333) disarms the countdown and drops the flag.
4. **The tail**, after the animation step has run: the start phase (`+0x8f8`) takes the request's
   progress (`+0x1a4`) and the serviced count (`+0x8fc`) takes the low byte of the request's
   sequence number (`+0x1a8`).

That tail is the answer to S25's last open question. It read `+0x8fc` as a byte nothing writes and
reported "any sequence of two or more restarts the state every frame"; in fact the chooser stores
last frame's sequence number there after every step, so the restart test fires on exactly the frame
the number changes. The start phase the stunt cross-fade is built from is likewise last frame's
progress, one frame of lag by construction.

### The two blend entries the port could not draw

Both are in the motion library and both were ported on the simulation side (S20, S25) and left
undrawn (V7: "inventing the arithmetic would be guessing"). The arithmetic, read off the image:

- **four clips, three weights** (`./kit fn 0x00147e40`, mode 4). The sample time is the *first*
  clip's duration times the phase, and all four clips are sampled at that one time. Per rotation
  channel, paired by position in the channel list: `q1 = slerp(mix, b, a)`, `q2 = slerp(mix, d, c)`,
  `q = slerp(cross, q2, q1)` — so `cross` 1 is the `a`/`b` pair and 0 is the `c`/`d` pair, and `mix`
  1 is the first of each pair. Per position channel, in the original's grouping:
  `p1 = b - mix·(b - a)`, `p2 = d - mix·(d - c)`, `p = p2 - cross·(p2 - p1)`. Then the cross-fade
  against the snapshot, the same `slerp(fade, q, was)` and `p + fade·(was - p)` the one- and
  two-clip entries end with. Both loops take the joint from the **first** clip's own map.
- **two clips at two phases** (`./kit fn 0x00147640`, mode 5, the entry nothing else in the image
  calls). Each clip is sampled at *its own* `duration x phase`; then `q = slerp(mix, b, a)` and
  `p = b - mix·(b - a)`, and the same cross-fade. The rotation loop takes its joint from the
  **second** clip's map and the position loop from the first's — an asymmetry in the original, not a
  port decision; it only shows up on two clips whose channel lists disagree.

## What I did

**The chooser, ported** (`crates/game-sim/src/vehicle_anim.rs`, additive — `Update`, `Asked`,
`Shared`, `Update::serviced`). `Update::want` is the function above: the start-line arm with its
breathe timer, its draw-without-repeats from the ten idles and its gate; the riding priority; the
idle fidget's countdown; and the two tail fields the step itself cannot write. It is the only writer
of `Driver::want`, so with it in place the port can ask for state 30 — and state 30 is where the
four-clip blend lives.

The constructor and the build gave the numbers it needs (`research/oracle.txt`, new entries):

- the still band and the grid's three thresholds, which in feet a second are 1.46666, 7.3333,
  14.6666 and 21.9999 — and `1/1.46666 = 0.681818` is the same factor the shell's speedometer uses,
  so **the grid's bands are 1, 5, 10 and 15 mph exactly**, with two more thresholds at 50 and 55 mph
  that the step never reads;
- the fidget's 15-to-45-second range, and that the body is constructed already asking for state 30;
- the two axes' rest value of a half and their time constants, 0.125 and 0.15 seconds.

**The two blend entries, drawn** (`crates/game-render/src/skin.rs`, `anim.rs`). The private channel
walk `Clip::lay` already did one clip, two clips and the cross-fade against the snapshot; it now
takes a small `Mixed` enum instead of an optional second clip, and the two new entries
`Clip::pose_four_faded` and `Clip::pose_pair_faded` are the arithmetic in the section above. The
four-clip one is a nested slerp per rotation channel and a bilinear lerp per position channel; the
pair samples each clip at its own `duration x phase` and keeps the original's joint-map asymmetry
(the rotation loop reads the *second* clip's channel list, the position loop the first's).

`Set::apply` now draws mode 4 instead of refusing it, and the new `Set::apply_vehicle` is the
renderer's entry point for the *other* machine: it takes a `vehicle_anim::Blend` and covers all five
of its modes. Two additive helpers on that blend answer the two times it needs
(`Blend::sample_time`, `Blend::pair_time`). `Set::apply`'s five arms are now five private helpers so
both entry points share them.

## How it was proved

**The chooser, against the original** — `./kit check -p game-sim --test vehicle_anim`, proof 4:
twelve thousand generated frames of `0x00248a70` in the oracle against the port, field for field, with
the floats compared as *bits* (a countdown that is `-0.0` where the original has `0.0` is a different
word in a save state and a different draw next frame). The step it calls in its own tail is hooked to
nothing, so what is left running is the chooser: every arm of its priority, the fidget's countdown,
the start line's draws against the list the whole start line shares, and the two fields it files away
after the step. The generator aims at the branches rather than sampling uniformly, and the test
asserts the counts as well as the agreement: every state the chooser can ask for is asked for, the
gate is opened, the fidget is started, re-armed and ended, and a start pose is drawn both from a list
the line had not filled and from one it threw away.

**The drawing** — `./kit check -p game-render --test anim`, nine tests. Both new entries are proved
on fixture clips through the public entry points, not by calling the samplers:

- the four-clip blend: nine clips, the four named rows reached through the real machine, and the
  pose checked against the nested slerp and the bilinear lerp written out by hand, plus strict
  betweenness of the result and the two weights strictly inside `0..1`;
- the pair: two clips that name *different* joints, which is the only way the joint-map asymmetry
  shows. The rotation it writes is the joint named by the clip being come out of and the position
  the joint named by the new one; the two sample times differ by more than a tenth of a second.

**The wiring** — `./kit check -p game-sim --test motion`, proof 10, a ride and a pad script with no
blend value written by hand anywhere in it. A bike on a hill of grade 1, the ported ride, the trick
machine, the chooser and the step:

- the start line ran 60 frames, every one of them in the start-line group and one of them an idle;
  the gate opened on frame 60, the first frame with the throttle commanded, which is the chooser
  clearing the request's at-start flag;
- the grid's bands then came in order — five frames of the four-clip blend of the stopped and
  standing rows (clips 25/24/28/27), 116 frames of the standing row alone, then 419 frames of the
  four-clip blend of the standing and riding rows (28/27/31/30) — each run of frames inside the
  thresholds of its own band, every weight strictly inside `0..1`, and the cross weight falling as
  the band was crossed, because it is the distance left to the top of it;
- a second ride, a 30-foot drop with two gestures five frames apart: the first plays alone, the
  trick machine hands it over half a second in, and frame 37 is the pair at two phases — 0.033 of
  the new clip against 0.550 of the one it is coming out of, mixed 0.167, on the trick machine's
  second request.

`./kit check`: 475 tests pass.

## Not verified / not 1:1

- **The request block's writer is not located.** The chooser reads nine fields of it (`+0x100`,
  `+0x104`, `+0xf8`, `+0xfc`, `+0x140`, `+0x160`, `+0x16c`, `+0x170`, `+0x174`, `+0x178`, `+0x1a0`,
  `+0x1a4`, `+0x1a8`) and writes one. What each *means* is read off how the chooser branches on it,
  and the mapping from a ride to that block — which is `asked_of` in the sim test — is the port's
  reading and not a ported function. Anything downstream of a misreading would show as the wrong
  state, not as wrong arithmetic.
- **The five fade rates and the stunt cross-fade cap have no located writer.** They are zero in the
  constructor (`+0x694..+0x6a4`, `+0x8e8`) and something not found fills them in; the test's are the
  fixture's, so the *speed* of every cross-fade in this machine is unproved. The ramp arithmetic they
  feed is S25's and is oracle-proved.
- **`+0x6b4` is written by a method this task did not port** (`0x0024c7e0`, the one that decides a
  wrecked body has stopped moving). The chooser's hold-pose arm reads it, so the five hold poses are
  reachable in the port only by setting it by hand, which is what `tests/vehicle_anim.rs` does.
- **The chooser's two draws are not seeded like the original's.** The original seeds the engine RNG
  off the time of day on first use; the port takes a seed, because a test that cannot repeat is not a
  proof. Which of the ten idles or three start poses comes out is therefore the port's, and the
  *draw-without-repeats* rule — the file-static list, its count and its reset — is ported and proved.
- **The group mask and the mirroring path of the original's samplers are not ported.** Both new
  entries ignore the per-group mask the image's channel walk tests and the mirrored-channel branch,
  exactly as the one- and two-clip entries that shipped before them do (V5, V7). Nothing in the port
  sets either yet.
- **Neither new drawing entry is oracle-compared.** They end in VU0 skinning, which is not ported, so
  the arithmetic is read off the image and asserted against a hand-written expectation rather than
  run against the original — the same standing limit as every other renderer-side entry. Rendering
  is not held to bit-exactness by the project's own rule.
- **The ride proof asserts no original.** It is a wiring proof: every piece of it is proved against
  the image elsewhere (the chooser and the step in `tests/vehicle_anim.rs`, the library entries in
  `tests/motion.rs` 1–9, the ride and the trick machine in their own tasks), and what it adds is that
  a real ride reaches the two entry points.
- **The grid's fourth band is not reached by that ride.** Over 15 mph the grid plays the riding row
  alone, and the hill tops out at 12.4 mph — because the port cannot *drive* this bike: the clutch
  command `mv::engine_update` reads (`ride.clutch_in`) is only ever written to one, by the
  idle-clutch rule in `ride::Ride::frame`, and the writer that lets it out again is not in the port.
  A clutch that is always fully in passes no torque to the rear wheel whatever the throttle does. In
  the image those are not two numbers: `bike+0x65c` *is* the rate-limited clutch lever — the engine's
  own `+0x34` is pointed straight at it (see `engine_ctor` in `research/oracle.txt`) — and the idle
  rule in the ground step stomps it to 1.0. The port keeps the lever in `mv::Step::clutch` and the
  engine's copy in `ride.clutch_in`, and only the stomp is ported. I have not touched it: it is a
  one-line change in another task's file that moves every ride the replay fixtures compare against.
  The fourth band is covered by the entry-point proof in `tests/vehicle_anim.rs` instead.
- **Nothing calls the body machine outside the tests yet.** The shell still draws the rider from the
  rider's own dispatch (V8), so what this task reached is reached by tests and not on screen.
- **`vehicle_anim` is still called that.** The module is the *rider's* second animation machine and
  the evidence for it is in this entry; the rename is churn in a file three other tasks touch, so the
  name stays and the field map stays S25's.

## Next

- Hook the body machine into the shell: `Update::want` → `Driver::frame` → `Set::apply_vehicle` on
  the rider actor, with the request block filled from the ride. That is what puts these poses on
  screen, and it is also the only thing that can show the fade rates are wrong.
- `ride.clutch_in` needs to *be* the rate-limited clutch lever, as `bike+0x65c` is in the image,
  with the idle rule stomping it rather than owning it. Until it is, no scripted ride in the port can
  accelerate under its own throttle, which costs every later task a band of its own proofs — and it
  wants its own task, because it changes what every ride fixture does.
- `0x0024c7e0` is the small method that writes `+0x6b4`, and it is all that stands between the port
  and the five hold poses after a wreck.
