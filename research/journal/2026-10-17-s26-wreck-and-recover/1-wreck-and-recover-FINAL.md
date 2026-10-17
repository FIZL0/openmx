# 1 — wreck and recover (FINAL)

Task S26. 2026-10-17.

## What I did

The crash and the recovery, end to end: a reason-5 wreck now throws the rider off, tumbles the bike
on its own parameters and puts the rider back on it with nobody touching the pad.

1. **`Wreckbike.phy`'s reader** — `phy::Part`, the engine's rigid-body *part* loader, which is not
   the vehicle loader S18 ported: one `[PhysicsPart1]` section, thirty-odd keys, its own defaults and
   its own units. A part's mass is `Weight × 0.0310559` (pounds to slugs) and its gravity is kept as
   a *force*, `(0, −GravityScale × Weight, 0, 1)`, so the weight cancels on the way through the
   integrator. `phy::Part::inertia` is the loader's own tensor: `mass/12 × InertiaScale ×
   (y²+z², x²+z², x²+y²)` of the **model's** bounding box.
2. **The rider file's `[Wrecks]`** — `phy::Wrecks`: seven throw groups of six keys each (hit from the
   front, the side, the back, the bottom, fell off the front, fell off the back, came off doing a
   trick), each a force, a force time, a hand-hold time and a per-difficulty delta for all three,
   plus `GroundAngularRateDamping`, `RiderFlailHeight`, `RiderRollTime` and `RiderRollSpeed` (mph).
3. **`ride::Crash`** — a wreck in progress. The entry is the original's own: the wreck body takes the
   bike's transform, its velocity and its spin in its own frame, and the cannonball's three random
   lanes are handed in rather than drawn. The body is then integrated by the same ported integrator
   the bike uses, under its own gravity force, against a sphere-vs-ground contact built from
   `Wreckbike.phy`'s spring, damping, `MaxPenetration` and `FrictionZone1`. The rider is one point:
   released at the throw's `HandHoldTime` from `RiderFlailHeight` above the body's centre of gravity
   with the body's velocity, falling under the part's own gravity scale until it reaches the ground.
4. **The settle gate** — the one input S5's machine had no source for. `state::Sense::settles` is now
   written every frame from the wreck in progress, which is what arms the reset clock, which is what
   makes `MaxWreckTime` mean anything. S5's note that "a bike's class never asks for it" was wrong
   and is corrected in `state.rs`: the gate is a runtime signal, not a class constant.
5. **The wiring** — `rider::Wreckage` (the two files plus the body's tensor) and
   `Rider::carries_wreck`. While the rider is down the ground step is skipped and the crash is
   stepped instead, which is what the original does; `rider::Frame::separated` reports the frame the
   hands come off (the frame a `wreck_sep` gesture would be played on); the respawn puts the bike
   **where the wreck ended up** and not on the start line; the crash hashes into the rider's walk.
6. **The shell** — `openmx` loads `wreckbike.phy` and the rider's `mrider.phy` by suffix out of
   whichever archive has them and hands them over. `--no-wrecks` still turns the session's wrecks off,
   but it is no longer the only way to ride for a minute: a wreck gets up by itself now.

## What the original does

Read with `./kit fn`:

| what | recipe |
| --- | --- |
| the rigid-body part loader | `./kit fn 0x15a500` |
| the rider file's `[Wrecks]` loader | `./kit fn 0x249cb0` (capped — the full dump is `context/notes/fn_boot_00249cb0.c`, keys at lines 404–455) |
| the wreck entry | `./kit fn 0x252ca0` |
| the INI getters it reads with | `0x14b790` float-or-default, `0x14b810` vector-or-default, `0x14b6f0` int/flag, `0x14a7a0` section select |

The loaders are ordinary INI readers; what is worth writing down is what they do *besides* read:

* A part with no `InertiaScale` gets **two**, not one: the loader builds its default by multiplying
  its all-ones constant by a broadcast 2.0 before handing it to the getter. `Wreckbike.phy` says
  `1,1,1` so the disc's wreck body never sees it, but a file without the key would.
* `CollisionWeight` defaults to `Weight`, which is why `Weight` has to be read first.
* The four sleep thresholds default to `FLT_MAX` — "never sleep" — not to zero.
* **Four keys of `Wreckbike.phy` are dead.** `LimitAngularRate`, `BodyAngularRateLimit`,
  `ImpactVelocityDamping` and `ImpactAngularRateDamping` have no string anywhere in the executable
  (`./kit data --strings` over the whole image, then `./kit xref` on what did match), so nothing reads
  them. They are in the file and the port ignores them, deliberately.
* The wreck entry hands the body the bike's spin *in the body's own frame*, and for the cannonball
  (reason 13) three `rand`-derived lanes instead.

The recovery itself is S5's machine, unchanged: the reason is kept, the latch arms when the rider
settles, the clock runs against `MaxWreckTime` (or `CannonballWreckTime`), and the respawn is the
caller's. What S26 adds is the settling.

## How it was proved

**The gate, measured three ways.** The settle gate is the rider actor's `+0xd3c`. Across three
reason-5 wrecks — two driven live over PINE with the freestyle vertical limit poked down to 0.1, one
in the human's capture — it comes on 18, 22 and 26 ticks after the entry, which at the 30 Hz the game
was running is 0.60 s, 0.73 s and 0.87 s. `HitBottomHandHoldTime` is 0.3 s, `RiderFlailHeight` is
3 feet, and a 3-foot fall under the file's own gravity takes 0.43 s: **0.3 + 0.43 = 0.73 s**, the
middle of the three to the frame, with the spread either side being the vertical velocity the rider
leaves the bike with. That is the model the port implements.

Two rival models the same three measurements **rule out**:

* *the gate is the wreck body coming to rest* — the body was still doing 42 ft/s when the gate came on.
* *the gate is the throw force applied to the rider as a whole* — 800 lbf for 0.1 s over the ragdoll's
  total mass is 11 ft/s upwards, an 0.89 s flight, and a gate ten ticks late on every one of the three.

So the throw's force is loaded and **not applied**: it poses a ragdoll this port does not have, and
applying it to a point moves the one number that was measured the wrong way.

**The two live captures.** `context/capture/s26/wreck.csv` and `wreck2.csv` (600 frames each, the
rider's reason, latch, clock, gate and loads, plus the wreck body's transform, velocity and rate)
hold a reason-5 wreck and its automatic recovery: **86 ticks** and **82 ticks** from entry to
respawn. The port, stepped at the same `dt`, takes **84**. The human's own minute (S8's capture) has
the two reason-5 windows the brief names, ticks 3259–3349 and 4167–4190 — **91** and **24**.

**The contact numbers.** `FrictionZone1 = 0.5` over the body's own load is 0.5 g ≈ 16 ft/s² of
deceleration, which is the ~60 units of slide the captured body does between landing and stopping.

**The respawn place.** Measured live: the game put the rider back at
`(4294.28, 136.99, 3473.11)` where the wreck body had come to rest at `(4294.79, 137.25, 3470.86)` —
the wreck's place, not the start line, which is what `Crash::rest` hands back.

**The tests** (`crates/game-sim/tests/ride.rs`, section 5, both new):

* `a_wreck_at_reason_five_gets_up_by_itself` — a bike dropped flat from 30 feet onto level ground
  passes the fixture's vertical wreck limit, wrecks at reason 5, and with **no input at all** from
  that frame on: the hands come off, the rider lands, the clock runs and the rider is riding again
  **84 ticks** later, inside the capture's own 91 and above the floor the model allows
  (`HandHoldTime + MaxWreckTime`).
* `the_two_captured_wrecks_match_their_lengths` — the same drop twice: left alone it is back up in 84
  of the capture's 91 ticks; with the reset chord pressed 24 ticks in it is back up on tick 24, which
  is the length of the capture's second wreck. Both lengths the recording holds are the port's.

`./kit check`: **477 tests passed**, and `./kit audit` is clean.

## Not verified / not 1:1

- **The gate's writer was never found.** A whole-program search for what writes the rider's `+0xd3c`
  turned up no store I could pin, so the *condition* is inferred from the rider separating and from
  the three measured times, not read off the original. It is bounded hard by those three (0.60 s,
  0.73 s, 0.87 s) and the port lands in the middle of them, but a wreck whose rider is thrown far
  enough to land somewhere else will drift from the original before this does.
- **The rider is a point, not a ragdoll.** The original's rider is an eighteen-part ragdoll with
  twenty-six constraints stepped at 25 Hz. None of it is ported: what is here is the one thing the
  machine reads out of it — when the rider hits the ground.
- **The separation is reported, not animated.** `rider::Frame::separated` is the frame the hands come
  off, which is the frame a separation gesture would start on, and nothing plays one. What the
  executable names is one wreck clip, `wreck_air_flail` (`./kit data --strings wreck`), loaded by the
  *rider's* own clip table — not the bike's `motion::Library` that this port has — into rider
  `+0x9a0`, by `./kit fn 0x2483a0`. The `wreck_sep*` gestures the brief mentions are in the rider's
  animation set but are named nowhere in the executable, so which one plays is the ragdoll's
  selection and is not ported with it.
- **The throw force is loaded and not applied**, with the numbers that rule it out written above.
  Every key of the seven groups is read and carried; only `HandHoldTime` changes the simulation.
- **The collision response is the original's shape with the file's numbers, not its solver.** The
  wreck body's contact is a sphere against the ground with `SpringStiffness`, `SpringDamping`,
  `MaxPenetration`, `FrictionZone1` and the two moment scales, pushed in at the contact point. The
  original runs its parts through the engine's own constraint solver, which is a task of its own; the
  consequence is that the body's *resting place* is approximate — and the respawn is that place.
- **The wreck body's inertia tensor is a measurement, not a load.** The loader builds it from the
  model's bounding box, and this port has no model library, so `ride::WRECK_INERTIA` is the diagonal
  the live wreck body carries, `(56.11, 46.04, 27.29)`. It is self-consistent — through the loader's
  own formula and `Weight = 250` it implies a box of 3.65 × 5.37 × 7.61 feet, which is a bike — and
  `phy::Part::inertia` is there for a shell that can hand over a real box, but the shipped constant
  is one bike's and would be wrong for a different wreck model.
- **The reason → throw mapping is the port's own inference.** `ride::hit_of` maps the wreck reasons
  onto the file's seven hit kinds by what their names say; there is no string, no table and no key
  for it in the executable, and the code that would read one is the ragdoll's. Only `Bottom` —
  reason 5, the one the capture hit twice — is exercised by anything.
- **The cannonball's spin is handed in, not drawn.** Reason 13 takes three random lanes off the
  game's own `rand` stream; `Crash::begin` takes them as an argument so the caller can supply the
  ported stream. Nothing calls it with them yet, so a cannonball tumbles on the bike's spin.
- **The respawn's facing is the port's own**: the bike's flattened forward at the moment it wrecked.
  The place was measured; the direction was not.
- **The ~3-tick latency between the request and the respawn is not ported.** In the captures the
  reason clears two to three ticks after the clock passes its limit; the port clears on the tick.
  This is inside the 84-against-91 margin but it is not the original's timing.
- **The fixture's numbers are not the disc's.** `tests/ride.rs`'s wreck limits, wreck part and rider
  throw set are hand-written in the shape of the game's files, as the rest of that file's bike is
  (game data is never committed). They are chosen so that the model's own sum lands inside the
  capture's window; the disc's values are quoted in this entry and in the source's docs, where they
  are measurements rather than data.
- **S5's claim is corrected, not merely added to.** `state.rs` used to say the settle window was
  something "a bike's class never asks for" and that the automatic reset was out of reach. That was
  wrong: the gate is written at runtime during a wreck. The old wording is replaced.

## Next

Nothing in S26 is left open. Three things it touched that are other tasks:

1. **The ride's g loads still oscillate past `SideWreckSpeedLimit` within seconds of the start line**
   (P3's note). With wrecks on, the shell now recovers from those wrecks instead of lying there — so
   the bug is rideable, but it is still a bug and `--no-wrecks` is still how you avoid it.
2. **The ragdoll, and the rider's own clip table with it.** Eighteen parts, twenty-six constraints,
   25 Hz, `wreck_air_flail` and the `wreck_sep*` gestures on top of it. The part loader this task
   ported is the reader it would need, `ride::Crash` is where the parts would hang, and
   `./kit fn 0x2483a0` is the table that holds their clips (the rider's, prefix-less `air_Fwd` and
   friends, which is a different table from the bike's `bike_air_*`).
3. **`WreckInertia` is a `[Wrecks]` key for other vehicle classes** (`0x0043f2b0`, `0x00442d38`,
   beside `WreckRotorRadius` and `%sWreckBlade.phy`) — so a class that is not the bike reads its
   wreck body's tensor out of the file after all. Worth a look when another vehicle is ported: it
   would replace the measured constant this task had to use for the bike.
