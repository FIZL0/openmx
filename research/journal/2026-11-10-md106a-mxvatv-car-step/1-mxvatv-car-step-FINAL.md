# 1 — mxvatv car step (FINAL)

Task MD106a. 2026-11-10.

## What I did

- `atv_car::Pieces<R>` (crates/game-sim/src/atv_car.rs): a `Steps` that answers five of the car's
  calls with MX Unleashed's pieces as SLUS-21104 builds them and hands every other call to `rest`:
  the centre of mass's travel, the anti-slide, the preload (and its new `Call::PreloadCue`), the launch
  and the aero. They run on `Keys` (the loader's words: CofG/CofGAI and travel, `[Preload]`, the
  clutch's launch keys, `[Aerodynamics]` and its wings) and `State` (the pad's, vehicle's and world's
  words they read, and the preload's and launch's own state).
- tests/mxvatv_car_frame.rs: the frame test is now `run(seed, cases, unhooked)`. Each case pokes
  generated keys and state into the image on both sides (`Mx`, `read_mx`, `mx_words`, `generate_mx`)
  and compares them after the frame too. The calls at `unhooked` get their hooks removed, so
  SLUS-21104's own code runs them. On the port side, `Unhooked` routes those calls to `Pieces::answer`
  and tags the branch each one took. The preload's cue is hooked (`hook_cue`) and logged. There is one
  test per piece and one with all five unhooked.
- research/oracle.txt `# MD106a`: the pieces' field map, constants and the cue.
- Filed MD122 (controls), MD123 (contact sort and probes), MD124 (tire step) and MD125 (engine step
  and shift). MD106c now waits on them and MD106b.

## What the original does

The five pieces are MX Unleashed's math (`crate::car`), changed in four ways:

- Where MXU multiplies and adds, they use fused `adda.s 0`/`madd.s`. That covers the travel's
  `cofg.z - travel·lean`, the wings' drag `basic + cl·(cl·induced)`, and the unroll and unyaw.
- The inline remap answers a degenerate input range with `out_lo - in_lo`, not `out_lo`: the branch
  past the lerp carries `sub.s` in its delay slot. The preload's push from a `[Preload]` with
  `MinPreloadTime == MaxPreloadTime` therefore has `y = -min`. The launch's start, with a zero
  `MaxClutchTorqueScaleSpeed`, is `max scale - 0`.
- The body's virtuals do the writes. Set-cofg (`./kit dis -p atv` at vt +0x5c) moves the position and
  the last position by the change turned to the world. The wings' add-force-at (`./kit fn -p atv
  0x0049bfa0`) adds `cross(p, f)·ones` to the moment and the force turned to the world to the force.
- The preload ends with a cue: `./kit dis -p atv 0x004cb7f0 0x004cba30`, the call at 0x004cb994. It
  plays once per push, while the timer is not run out (+0x700 marks it played), and is forgotten once
  the timer runs out.

They read their own words: the vehicle's gas +0x318 and brake +0x310, gravity from `*(car+0xe8)+0x40`,
the record's touching word, and the left front tire's ground normal. A person drives when the
vehicle state's mode is 1. All offsets are in research/oracle.txt `# MD106a`.

## How it was proved

Function oracle in the frame: SLUS-21104's world step over MD39a's buggy image
(`tests/mxvatv_car_frame.rs`). Six runs of 3,000 generated frames each, one per piece unhooked and one
with all five unhooked, plus the old all-hooked run (3,000). In every run, every word of the car,
tires, body, the pieces' keys and state and the engine's torque scale equals the port's. The call logs
are equal (cues included), and the original wrote nothing in those blocks that isn't compared.

Each run sees every branch of its pieces at least 20 times:

- travel: a person's or the AI's, forward or back;
- anti-slide: idle, holds, brakes;
- preload: not ready, ready idle, armed, wound, fired, fired on the degenerate range, pushing, ran
  out, cue, cue forgotten;
- launch: clutch in, idle, starts, starts on the degenerate range, eases;
- aero: wings, air control, unroll, gyro.

Mutation check: with the remap's quirk removed (`out_lo`), preload_is_mx_unleasheds goes red at case
37, with 13 words different.

## Not verified / not 1:1

- **Six of the brief's eleven pieces are not ported**: the controls, contact sort, tire probe, tire
  step, engine step and shift. They stay hooked and are filed as MD122–MD125. The brief's test ("each
  MXU piece unhooked") holds only for the five.
- **The preload's and aero's own wrecked check is not reached.** The Force body calls the preload only
  while riding. The aero's wrecked return is reached through the frame (states 13/14/5 are generated),
  but the preload's is not.
- **Only zero or one wing.** NumberOfWings 2 or more was not run: the image has one wing, and I
  didn't place a second where RAM may be in use. Counts 0 and -1 (no wings) are run.
- **The aero's null-record branch** (no collision record) was not run: the image has a record.
- **Keys are not built from `car::Car`/the .ini loader.** `Keys` is read off the image. Building them
  from MX vs. ATV's files is left to whoever wires `Pieces` into a ride.
- **Gravity was not varied**: only the image's (0, −2400, 0, 1).
- **Only the buggy** (MD39a's image) was run, not the other four car classes.

## Next

MD122–MD125, then MD106c.
