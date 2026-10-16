# 1 — g loads read the impact accumulator (FINAL)

Task S22. 2026-10-16.

## What I did

P3 left the shell unrideable with wrecks on: the sideways g load alternated sign and grew frame over
frame while the bike idled off the start line — about 0.5, 1.0, 1.5, 6.6, 11.7, 15.4, 18.3 — crossing
the disc's own `SideWreckSpeedLimit` of 16.87 about 2.5 s after the spawn, standing still. `g_peak`,
the limit and the wreck test were each already proved, so the brief was to compare the port's
per-substep local acceleration against a live capture of the chassis' `+0x550` and find the substep
where the two part company.

They part company at the **first** substep, and not because the solver rings: **the port was feeding
the wreck test the wrong field**. Four things came out of it, all four now fixed:

1. **The g loads are an impact measurement, not an acceleration.** The ground step clears
   `chassis+0x170` beside the bike's own `+0x550`, once a frame, and the only thing in the whole frame
   that writes `+0x170` is the bottom-out branch of the tire step, which takes the into-the-ground
   part out of the body's velocity and records the closing speed it cancelled. A ride that never
   bottoms out has g loads of **exactly zero**. The port's shell was copying the body's own acceleration
   (`+0x70` — force times inverse mass, which is roughly gravity plus the springs) into that
   accumulator around every integrate, and unrotating it once a frame instead of once a substep. So
   the port's wreck test was reading, every frame, a number that is never small — and grew without
   bound as the bike toppled, for reasons 2 and 3.
2. **A wheel's mount is the bike model's own tire node, not a `.phy` key.** The shell passed
   `BodyTireForcePosition` (±0.75 ft in `z`), giving a 1.5 ft wheelbase under a centre of gravity 2 ft
   up — a toppling pendulum, which is what was really ringing. The original's wheels sit at the
   model's `TireFront`/`TireRear` nodes: a 5.11 ft wheelbase.
3. **The suspension probe is one tire radius long** (1.2 ft), not `StrutCompression` (1.0).
   `StrutCompression` is the bottom-out limit and nothing else.
4. **The damping ratios are of critical damping**, `ratio * 2 * sqrt(mass * rate)` against the rate
   the loader set last — not ratios of the spring rate. The old reading was 3.5x too stiff on a
   cr125_faisst's front and 4.5x on its rear.

After the four, with nobody on the pad (`openmx --track free06 --bike 125sx --trace 600`, 20 s at the
30 fps clamp):

- Brakes held, the bike stands. `g_peak` reads `0.000 / 0.000 / 0.000` on every one of the 600 frames,
  the suspension sits at 0.453 and 0.441 of its travel, and the two wheels carry 201.4 and 196.4 lbf —
  397.8 of the bike's 400. The original, idling on its own track, sits at 0.462 and 0.447 carrying
  204.2 and 196.4: the same equilibrium within 2%.
- Brakes off, it rolls down the Quadzilla start hill for the whole 600 frames, jump and all, and does
  not wreck. The vertical g load now does what it is for — zero all the way down, 37.7 on the landing
  — and the sideways lane the wreck test judges peaks at 0.014 against a limit of 16.87. Before this
  task the same run wrecked on the sideways lane inside 16 frames, standing still.

## What the original does

`./kit fn --program boot 0x00257a00` (the ground frame). Once a frame, before the substep loop, it
clears the bike's `+0x550..+0x55c` and the chassis' `+0x170..+0x17c` and `+0x180..`. Then, in every
substep, after the two wheels have had their suspension, tire forces and integration and just before
the body's own integrate:

```
transform_unrotate(&local, bike+0x18, chassis+0x170);
if (ABS(bike+0x550) < ABS(local.x)) bike+0x550 = local.x;   // and +0x554 from y, +0x558 from z
```

`bike+0x18` is the same transform the chassis holds at `+0x254` — `./kit fn --program boot 0x00253760`
hands it over through the body's vtable `+0x74` — so this is the body's own frame.

`chassis+0x170` is written in exactly one place: the bottom-out branch of
`./kit fn --program boot 0x0025bc70`, reached when the compression passes `wheel+0x48`. It subtracts
the into-the-ground part of the chassis' velocity from `chassis+0x260`, and subtracts the same scalar
— along the body's own up turned into the world, the constant `(0,1,0,1)` at `0x00426db0` — from
`chassis+0x170`. There is no `dt` in it: what the accumulator carries is the closing speed the shove
cancelled. `./kit fn --program boot 0x0015c540` (the integrate) writes
`+0x70` and `+0x80` and never touches `+0x170`.

The same `0x00253760` is where a wheel's geometry comes from: the node named `TireFront` is looked up
in the bike's own frame and its place written straight into the front wheel's `+0xa0`, `Shocks` and
`Handlebars` become the normalised steering axis at `+0x8c0`, and `TireRear` lands at the rear
wheel's `+0xa0`. `BodyTireForcePosition` goes to the wheel's `+0xb0`
(`./kit dis 0x00253d3c --len 32`), which neither the suspension nor the tire step reads.

`./kit fn --program boot 0x0025ac50` is the suspension setter, five lines long:
`+0x48 = StrutCompression`, `+0x4c`/`+0x50` the two rates, `+0x54 = |spring * mass * 32.2|`, and both
damping rates `= ratio * 2 * sqrt(mass * (+0x54))`.

All of it is written up in `research/oracle.txt`'s S22 block, which also records the one 1:1
difference this task found and did not fix (the contact velocity's arm — see below).

## How it was proved

**A live capture of the original**, `watch/s22-standing.toml` → `context/capture/s22-standing.csv`
(300 frames, one row per tick, 0 torn reads and 0 counter gaps, from the human recording's own save
state in slot 8 — free09 on a cr125_faisst, nobody on the pad). It carries, beside the bike's
`+0x550` and `+0x748`, both of the chassis' acceleration pairs and everything behind the suspension on
both wheels. On all 300 rows:

- `g_peak` = `(0, 0, 0, 1)` and `ch_impact` (`+0x170`) = `(0, 0, 0, 1)` and `ch_ang_impact`
  (`+0x180`) = `(0, 0, 0, 1)` — **exactly** zero on all 900 lanes, while `ch_accel` (`+0x70`) is
  nonzero on every one of the 300 rows (0.07 to 0.24 in its largest lane). That is the proof that the
  g loads do not come from `+0x70`.
- `rider_type` 2 and `rider_ctl_src` 1 (the human's rider, not an AI), `wreck_reason` 0, `dt` pinned
  at 0.033330001.
- `f_mount` = `(0, 1.2332027, 2.5224998)` and `r_mount` = `(0, 1.1946994, -2.5897298)`, constant —
  the bike model's `TireFront`/`TireRear` node positions to the bit (the port prints the same two out
  of the model: `openmx --track free09 --bike cr125_faisst --trace 1`).
- `f_probe_len` = `r_probe_len` = 1.19999993 = `TireRadius`.
- `f_damp_in` = `f_damp_out` = 51.7998924 and `r_damp_in` = `r_damp_out` = 89.164505, which is
  `2*0.3*sqrt(12.42236*600)` and `2*0.4*sqrt(12.42236*1000)` — the setter's own formula over
  `Spring`, and what the port now computes to the last digit. Both wheels' `+0x54` reads
  439.99993896484375, which is `|1.1 * 12.42236 * 32.2|`: the *stopped* rate, re-blended into `+0x54`
  by the ground step every frame, and the port's own number for a standing bike to the bit.
- `f_comp`/`r_comp` 0.460..0.465 and 0.434..0.449, `f_spring_force + r_spring_force` 398.7..400.6 —
  the bike's 400 lbf of weight, split near evenly. That is the equilibrium a standing bike has to
  reach, and the port now reaches it on the capture's own track and bike: 0.4622 / 0.4474 of travel
  carrying 203.3 and 196.7 lbf, against the capture's means of 0.4631 / 0.4447 and 203.9 / 195.9.
  Under 0.6% on the travel and under 0.5% on the load, with nothing in between tuned to make it so.

**The decompilation** for the three structural facts a capture cannot show: that `+0x170` is cleared
beside `+0x550` and unrotated into it once per substep, that the bottom-out branch is its only
writer, and that the mounts come from the model's nodes by name. Addresses above.

`./kit check` green, 442 tests (the one gate; the suite includes S3's whole-frame comparison of the
ground step against the original through the real `transform_unrotate`, whose hook already knew
`CHASSIS + 0x170` is `mv::Chassis::accel` — the port's own types had the field right all along).

**A standing-bike regression test**, `crates/game-sim/tests/stand.rs`: 240 frames of the hand-written
bike dropped a foot onto a flat floor with nobody on the pad. It asserts what the capture says a
standing bike is — both wheels in contact, the two spring forces adding up to the weight inside 5%,
the compressions still to a thousandth of a foot over the second half, and **every g lane under 1.0**.
Against the code as it was it stops on the *first* assertion — a wheel has left the floor — and with
the geometry and the damping fixed but the g loads still read off the body's `+0x70` it stops on the
last one, reading 8.05 sideways g on a bike that is demonstrably not moving sideways. Either way it
would have caught this task's bug on its own.

One committed test had to be repointed. `the_freeride_fixture_is_a_ride` asserted that `engine.rpm`
moved across the fixture's 30-frame samples, and it had been passing for the wrong reason: the old
solver threw that bike into the air for half the session, and a wheel with no ground under it
free-revs the engine, so the samples read 7086, 4432, 9247, 10600 rpm. (The -51000 rad/s the rear
wheel reaches on the fifth frame is a separate oddity and not the old solver's: the wheel is airborne,
and what knocks it there is the fixture bike's own `EngineBraking = 0.3` through the engine's
`rpm * (EngineBraking * rpm)` — about a million lbf-ft of crank braking at idle revs, on a part
throttle. It reads the same before this task's changes and after them.) With the solver fixed the fixture bike's revs are honest, and honest means pinned:
the clutch locks, the engine turns at the road's revs (`9.549 * spin * ratio`), the fixture's first
gear is 2.1 against a 3.6 final drive, and a flat floor leaves it cruising at about 17 rad/s of wheel
— some 1700 road rpm, under the file's own `MinRPM = 1800`. So `engine.rpm` sits on its floor all
session and steps off it for the odd single frame that no sample lands on. The assertion now reads the
driven wheel the engine is locked to (`rear.mv.spin`), which is the same information upstream of the
clamp; `engine.rpm` stays in the watch list, where the replay still compares it tick for tick.

## Not verified / not 1:1

- **The hand-written test bike's `EngineBraking` is out of range, and nothing here fixed it.** S15
  proved `engine_update`'s own arithmetic, braking term and all (`rpm * (EngineBraking * rpm)`), so
  the formula is not in question — but `tests/freeride-bike.phy.txt` carries `EngineBraking = 0.3`,
  which at idle revs is about a million lbf-ft on the crank and, on a part throttle, hurls the
  airborne rear wheel to -51000 rad/s inside the first ten frames of the freeride fixture. It reads
  the same before this task and after it. Left alone on purpose: the number is one of this crate's
  own fixtures, the disc's value was not read for it, and changing a fixture bike moves every hash in
  every test that rides one. A task that reads `EngineBraking` off the disc and re-scales the
  fixture's would be worth having.

- **The contact velocity is measured at the wrong point.** The original
  (`./kit fn --program boot 0x0025b710`) takes the arm as `wheel+0x140` (the load point, where the
  suspension line crosses the contact plane) minus `chassis+0x60`; `ride.rs`'s stand-in measures it at
  `wh.w.contact_point` (`+0x120`, the ground query's hit). Not fixed here: `wheel::tire_forces` asks
  for the contact velocity through a `Call` whose answer it *discards* and reads
  `w.contact_velocity` instead, so the shell has to pre-seed it before the step and cannot see the
  load point the step is about to compute. Fixing it means making `Call::ContactVelocity` return its
  answer, which changes a module S16 proved and would land in another agent's way. On flat ground the
  two points differ by the offset of the ray's hit from the strut line, which is small; it is written
  up in the oracle's S22 block so the next reader does not have to find it again.
- **The standing attitude agrees, but only on a standing bike.** The capture is 300 frames of a bike
  doing nothing, so what it proves is the equilibrium: the loads, the travel, the geometry, the
  setters' arithmetic and the g loads' own field. It says nothing about the *transient* — how the port
  gets to that equilibrium, or what it does on a landing — because the original never left it. The
  columns for that comparison are already in `watch/s22-standing.toml`; what it needs is the same spec
  with a pad script that rides.
- **`chassis+0x180`, the angular half of the impact accumulator, is never written by this port.** It
  is cleared once a frame like its linear partner, and `wheel::Chassis` has no field for it, so
  nothing in the ported bottom-out writes it. The capture reads it zero on all 300 rows, so there is
  nothing to compare against yet; a bottom-out capture would say whether the original writes it.
- **`+0x170` does not survive the frame in the port.** In the original it sits on the chassis until
  the next frame's clear; here it lives on the ground step's own view of the chassis, which is built
  and cleared per frame. Nothing outside the ground step reads it in either, so no behaviour hangs on
  it — but a future reader of `+0x170` from outside the frame would find zero where the original has
  the last substep's impact.
- **The g loads were compared against an idling original, not a landing one.** Zero against zero is a
  real comparison and it is what the task's symptom was about, but it does not prove the *scale* of
  the port's impact g against the original's. Proving that needs a capture of a bottom-out: the same
  spec with a pad script that rides off something. The port's own landing reads 37.7 vertical, which
  is the right order for `NoHanderWreckSpeedLimit`-type tests but is not checked against the disc.

## Next

Nothing waiting on this task. What would carry it further is a capture of the original *bottoming out*
— the same spec with a pad script that rides off something — because that is the only way to put a
number on the impact g the port now reads rather than on the zero it has been checked against.
