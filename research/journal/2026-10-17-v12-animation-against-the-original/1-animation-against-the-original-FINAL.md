# 1 — animation against the original (FINAL)

Task V12. 2026-10-17.

## What I did

Nothing in the port's animation had ever been compared against a frame of the running original. V8
proved the rider's machine against generated inputs, V9 wired the bike's up and admitted in writing
that *"every number in `params()` is this port's own"*. So the port could have had the right clips
and played them at the wrong phase, or the right phase on the wrong clip, and no test would have
said so.

This task recorded the original's own joint array over PINE while the human's recording played back,
and compared it against the port's posed skeleton on the same ticks, joint by joint — the way
`replay.rs` compares the physics.

Three things came out of it, in rising order of how much they matter:

**1 — the port's two animation machines are paired with the wrong models.** The object the capture
watches is the type-4 actor's animation block (the actor pointer − 0x400). It has **22 joints**, it
reads its clips out of `Rider\Anim\`, and all **sixty** of its clip-table slots bind: the 24
`rider_<Stunt>` clips *and* the 36 bare pose names (`turn_stop_*`, `turn_still_*`, `turn_riding_*`,
`air_*`, `get_ready_*`, `hold_*`, `reverse_*`, `backflip`, `nohandedlander_left_right`,
`wreck_*`). That object is the one S25 ported as `vehicle_anim`, and it drives the **rider's body**.
The bike's own object is the one that holds the 12 `bike_*` clips V9 found. The port has them
swapped: `game-render`'s `Chassis` runs `vehicle_anim` against the bike's `.mcf`, which is why V9
could only bind 12 of 60 slots and why its grounded pose grid drew nothing. Re-pairing the renderer
is a change to `game-render` that V12 did not come for; it is on the plan.

**2 — the numbers V9 had to guess are now read off the original.** Measured on all 1500 frames, all
constant for the whole recording:

| `Params` field | original | V9's guess |
| --- | --- | --- |
| `rate_ready` (+0x694) | 0.25 | 3.0 |
| `rate_hold` (+0x698) | 0.25 | 3.0 |
| `rate_default` (+0x69c) | 4.0 | 3.0 |
| `rate_pose` (+0x6a0) | 3.25 | 3.0 |
| `rate_stunt` (+0x6a4) | 4.0 | 3.0 |
| `cross_cap` (+0x8e8) | 1.0 | 0.25 |
| `band` (+0xa14..+0xa1c) | 7.3333, 14.6666, 21.9999 | 0, 0, 0 |

and the two `Lag` filters' own constants, which `Params` does not carry because they live on the
`Lag` itself: the mix axis (+0x630) has tau 0.125 and ratio 0.26664, the phase axis (+0x634) tau
0.15 and ratio 0.22219.

**3 — the port's machine reproduces the original exactly, and its pose to 2.2e-5.** Below.

The capture spec is new (`watch/rider-joints.toml`): 72 fields, 227 columns, 1368 bytes a frame,
every frame, 1500 frames. It walks `@game_manager -> 0x34 -> 0x34` and then eight `0x8` hops down
the actor list to `-> 0x0`, which is the player's vehicle; the rider-animation fields hang off that
as `<that> -> <offset> - 0x400`. The joint array is an `[[array]]` at `-> 0x234 - 0x400 -> 0x0`,
count 22, stride 0x60, with the translation quadword at `+0x40` and the quaternion at `+0x50`.

Finding the chain in the first place is `research/poke.py`, also new: it chases a pointer chain
through a save state's EE RAM with no emulator running, which is the only way to try eighty guesses
at an offset without a PINE client fighting the recorder for the socket. It is kept because every
later pointer hunt wants one.

Playing the recording back needed no new work: S8's third session found that this desktop delivers
no keystrokes to PCSX2 but *does* reach its Qt menus over the accessibility bus.
`research/play_recording.py` is that driver, with the movie path typed into the file dialog the same
way; it refuses a read-only movie, because PCSX2 records over a `.p2m2` when playback ends.

## What the original does

The motion object's clock fields, read out of `research/oracle.txt:611-614` and then confirmed
against the capture — the important correction this task made:

| offset | what it is | port |
| --- | --- | --- |
| +0x3c | the clip finished | `Player::finished` |
| +0x48 | the clip playing | `Player::clip` |
| +0x4c | the clip a transition comes out of | `Fade::from` |
| +0x50 | the clip the last sample came from | `Player::last` |
| **+0x54** | **that sample's time** | **`Player::prev`** |
| +0x80 | a transition is running | `Fade::on` |
| **+0x84** | **the clip time** | **`Player::time`** |
| +0x88 / +0x8c / +0x90 / +0x94 | the transition's clock, end, delay, length | `Fade` |

I had +0x54 down as the clip time, which is wrong and cost most of a session: the port came out a
tick ahead on every phase-driven frame. The capture settles it — at frame 4168 +0x54 is 0 and +0x84
is 0.03333, at 4169 they are 0.03333 and 0.06666, and +0x84 is *frozen* while the blend is
phase-driven while +0x54 is where the drive left it. The sample time a pose comes out at is
therefore +0x54 for a driven blend and +0x84 for a free-running one, which is exactly what
`Set::apply_vehicle` and `Player::sample_time` already do.

The pose grid's interpolating arm is confirmed from the data rather than re-read:
`./kit fn 0x002492b0` is the ported `Driver::frame`, and its `TURN_RIDING` arm below the first band
mixes two pairs. At frame 2776 the request's `+0x178` is 0.609 and the mix axis is 0.5, and the
original's pelvis is `0.9169 · turn_stop_Mid + 0.0831 · turn_still_Mid` on all three lanes to five
digits. `0.9169 = remap(0.609, 0, 7.3333, 1, 0)`, so `+0x178` *is* the band number the grid cuts up
and `band[0]` is 7.3333 — the port's arm and its weight are right, and mode 4 is only unported in
the renderer.

The three fore/aft columns of a grid row are `_Fwd`, `_Mid`, `_Bck` in table order, and the mix axis
runs front to back: `pair()` takes `(Mid, Fwd)` below 0.5 and `(Bck, Mid)` above it. The port had
that right too; the capture is what names the axis.

## How it was proved

`crates/game-sim/tests/motion.rs`, a tenth section of two tests against
`context/capture/v12/rider-joints.csv` — 1500 consecutive frames (counter 2776..4275, 50 s) of the
human's recording played back on PCSX2 copy 5. Both print `SKIPPED: …` and pass with no capture,
the idiom the rest of the file uses. Every row is checked for shape before anything is compared:
actor type 4, controller source 1, rider type 2, 22 joints, `anim_dir` `Rider\Anim\`, `dt` the
simulation's own `CAPPED_DT`, a byte-identical clip table on every row, and every frame the next
counter of the one before.

**Test 1 — `the_originals_posed_rider_and_the_ports_agree_joint_by_joint`.** The simplest possible
question, asked without the machine in the way: when the original is playing one clip and nothing
else, is the port's skeleton the same skeleton? 146 of the 1500 frames are that (`rider_Taunt` ×57,
`rider_lazyboy` ×89); the rest are a blend of more than one clip (1226), a clip-to-clip transition
(21), or a cross-fade against the pose snapshot that had not run out (107). On those 146, all 22
joints × 8 lanes agree, worst 2.16e-5 on joint 3 (`UprLegR`)'s quaternion. The test also proves the
capture's joint array is in skeleton node order, by name: the joints the original never rotates over
the whole recording are exactly the `Dummy_*` bones.

**Test 2 — `the_ports_machine_steps_to_the_originals_clip_phase_and_pose`.** The real proof. For
each of the 1499 consecutive pairs of frames it seeds a `Driver`, a `Params`, a `Request` and a
`Player` from the *earlier* frame, steps them one tick with the original's own `dt`, and compares
every number against the *later* frame. Seeding from the capture each tick rather than running free
is deliberate: it makes each tick an independent test of one step, so a failure names the step that
went wrong instead of the first one that did.

Eighteen numbers, on all 1499 ticks, **bit-identical** — every one came out off by exactly zero, and
the tolerances in the table are headroom, not measured error:

- the latched state (+0x8f0), the previous state (+0x8ec), and the three flags from_table /
  holding / started (+0x680 / +0x684 / +0x688) — asserted with `assert_eq!`
- the cross-fade ramp (+0x68c) and the fade rate (+0x690)
- both `Lag` axes and both their ratios (+0x630, +0x634)
- the stunt cross-fade's phase (+0x8dc), the one it comes out of (+0x8e0), the reverse clock (+0xa3c)
- the clip the blend drove, against +0x50, on every tick
- the last sample's time (+0x54), the clip clock (+0x84), the transition's clock (+0x88)

That last group is the task's "tell a wrong clip from a right clip played at the wrong phase", and
it holds on every tick of the recording including the ones whose *pose* the port cannot draw. Blend
modes over the minute: free/one/two/-/four/pair = 114 / 160 / 1113 / 0 / 112 / 0.

Then the pose. 1117 of the 1499 ticks are drawn and compared joint by joint, worst disagreement
anywhere **2.16e-5**, on joint 3 (`UprLegR`)'s quaternion at tick 4107 during `rider_lazyboy`.
Broken out by which arm produced it (the test prints this table every run):

| state | band region | mode | ticks | worst rotation | worst translation |
| --- | --- | --- | --- | --- | --- |
| 0 (`rider_Taunt`) | 3 | 1 | 57 | 4.2e-7 (LwrTorso) | 2.4e-7 (Pelvis) |
| 1 (`rider_lazyboy`) | 0 | 1 | 42 | 2.16e-5 (UprLegR) | 4.8e-7 (Pelvis) |
| 1 | 1 | 1 | 6 | 2.16e-5 (UprLegR) | 2.4e-7 |
| 1 | 2 | 1 | 6 | 4.2e-7 | 2.4e-7 |
| 1 | 3 | 1 | 35 | 2.07e-5 (UprLegR) | 1.2e-7 |
| 30 (`TURN_RIDING`) | 1 | 2 | 31 | 6.0e-7 (Pelvis) | 4.8e-7 |
| 30 | 3 | 2 | 657 | 7.2e-7 (LwrArmL) | 9.5e-7 |
| 35 (`AIR`) | 0 | 2 | 47 | 6.6e-7 (UprTorso) | 9.5e-7 |
| 35 | 1 | 2 | 26 | 7.2e-7 (UprTorso) | 9.5e-7 |
| 35 | 2 | 2 | 32 | 6.6e-7 (Head) | 9.5e-7 |
| 35 | 3 | 2 | 178 | 6.6e-7 (UprArmL) | 9.5e-7 |

**The tolerance is 1e-4 and it is measured from both sides.** Above: the worst lane anywhere is
2.16e-5, and it only appears on the single-clip rows where the port and the original decode the same
packed channels in a different multiply order — every two-clip row lands at 7e-7 or better. Below:
over the same recording the fastest-moving joint of a tick moves a **median 0.0378** in one tick, so
a clip sampled one frame out shows up two to three orders of magnitude above the bar; and the
quietest quaternion lane that moves at all still swings **0.0443** over the recording, so a lane
held at the wrong constant cannot hide under it either. 1e-4 sits in that gap: ~5× the measured
arithmetic noise, ~400× under the quietest real motion.

### The one pose the port gets wrong, and why

26 of the 1499 ticks are skipped with a named reason, and the test asserts their *shape* (state 30,
band between `band[0]` and `band[1]`, mix axis past 0.5) and bounds their count, so a port that gets
a pose wrong anywhere else — including the front half of the very same row — still fails.

The cause is in this disc's data. `turn_still_Bck` and `turn_still_Mid` are **the same animation**:
sampled twenty times over their length they agree to the bit in every lane of every joint. The other
two grid rows do not do this — `turn_stop_Bck` is 0.49 of rotation from its own middle column and
`turn_riding_Bck` is 0.80. So on the back half of that one row the port's two-clip blend collapses
to the middle clip alone, while the original plainly poses something else: 0.61 of rotation and 1.17
of translation away.

It is not the weight and not the pairing. Solving for the weight by least squares on the *front*
half of the same row recovers the port's own `half(mix)` to 1e-7 on every tick, and the full
22-joint comparison there is 6e-7. And nothing in the set the port loads can be the missing clip:
over all 63 clips of the rider's `.mcf`, at 41 sample times each, mixed against `turn_still_Mid` at
the least-squares best weight, the closest any of them comes to the original's 22 quaternions is
**0.19**. Whatever the original's third column points at is not in the set this port loads, which
makes it a loader question — the same shape of bug V9 hit when `bike_air_Fwd/Mid/Bck` all resolved
to one `.MOT`. It is on the plan.

## Not verified / not 1:1

- **The pose grid's middle row past centre, 26 ticks of 1499.** Characterised above and skipped with
  its shape asserted, not ignored. The port draws `turn_still_Mid` alone because this disc's
  `turn_still_Bck` is a byte-identical duplicate of it; the original draws something 0.61 of rotation
  away that is not any clip of the 63 the port loads. Unresolved, and it needs the clip loader
  looked at, not the machine.
- **The 108 mode-4 ticks are not posed at all.** `Blend::Four` returns false from
  `Set::apply_vehicle` — V9's deliberate omission, now measured: 108 of 1499 ticks of real riding
  reach it, so it is not the rarity V9 took it for. The *machine's* numbers are proved on those
  ticks; only the pose is missing.
- **248 ticks where the cross-fade against the pose snapshot had not run out are not posed.** What
  the ramp fades *from* is a snapshot buffer the capture does not carry, so the port has nothing to
  fade against. Same for the clip-to-clip transition's second clip (counted inside that figure).
  Capturing the snapshot buffer would need a second array in the spec and is the obvious next
  widening of this proof.
- **The two machines are still paired with the wrong models in the renderer.** The finding is
  proved (22 joints, `Rider\Anim\`, all 60 slots bound), the fix is not made: `game-render`'s
  `Chassis` still runs `vehicle_anim` against the bike's `.mcf`. Out of V12's scope — this task
  touches one test, one spec and one dev-dependency.
- **`+0x8f8` is not the constant `research/oracle.txt` calls `start_phase`.** On the rider's object
  it takes 135 distinct values over the recording (0.0 on 847 frames, 1.0153 on 353, 1.0016 on 142).
  The test passes `start_phase = 0`, which is safe only because the stunt cross-fade never runs in
  this recording: +0x8dc and +0x8e0 are exactly zero on all 1500 frames and +0x8e4 never exceeds
  1.4e-43, a denormal left over from whatever last wrote it. Nothing here reaches the arm that reads
  +0x8f8, so whatever that field is, it is unproved.
- **`Blend::Pair` (mode 5) is never exercised**, for the same reason: it needs a stunt cross-fade,
  and there is none in this recording. Nor is `Blend::Free` reached with a finished clip.
- **`crates/ps2-oracle/src/` is on the task's file line and was not touched.** Nothing the proof
  needed belonged there: the capture reader is twenty lines of CSV decoding specific to one spec, and
  it sits in the test beside the eight readers `motion.rs` already has.
- **The watch spec's pointer chain is true of one save state.** Eight `0x8` hops down the actor list
  reaches the player's vehicle in the state the recording starts from. Nothing proves the player is
  always the eighth actor; a different track or a different rider count would need the hop count
  re-found. The per-row shape asserts (actor type 4, rider type 2, 22 joints, `Rider\Anim\`) are
  there to turn a wrong chain into a failed test rather than silent nonsense.
- **Nobody watched the playback.** The movie was driven over the accessibility bus and the capture
  came back the right shape and length, but no human confirmed the bike on screen was doing what the
  recording says. The rows' own self-consistency (a monotonic frame counter, a constant clip table, a
  constant `dt`) is the only evidence the playback was clean.
- **The whole proof is one recording, one rider, one track, 50 seconds.** Six of the sixty states
  appear (30, 35, 1, 40, 0, 45); the other fifty-four are proved only by V8's generated-input
  oracles. No wreck, no reverse, no start gate.
- **`d_extra` is -1 and `holding` is 0 on every frame**, so `Driver::extra` and the holding branch
  are carried through the comparison but never take a non-trivial value.
- **The capture is game data and is not committed** (AGENT.md). `context/capture/v12/` and
  `context/recordings/v12/` are git-ignored, so both tests print `SKIPPED: …` and pass on a checkout
  without them. Re-recording is `./kit record` against `watch/rider-joints.toml` with
  `research/play_recording.py` driving the movie.

## Next

Nothing outstanding for this task. Three follow-ups, all out of V12's scope and all added to the
plan:

1. **V13** — re-pair the renderer's two animation machines: `vehicle_anim` drives the rider's 22-joint
   skeleton off `Rider\Anim\`, and the bike's own object is the one with the 12 `bike_*` clips.
   V9's `params()` can take the seven measured numbers above at the same time.
2. **V14** — find the clip the original's `turn_still` back column points at, which this disc's `.mcf` resolves
   to a duplicate of the middle one for the port. 26 ticks of this capture are waiting for it.
3. **V15** — widen the capture to the pose snapshot buffer, which would let the other 248 ticks be compared.
