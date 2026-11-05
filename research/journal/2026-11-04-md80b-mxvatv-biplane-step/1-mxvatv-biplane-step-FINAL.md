# 1 — mxvatv biplane step (FINAL)

Task MD80b. 2026-11-04.

## What I did

- **The file.** `phy::Plane::read_mxvatv` / `read_mxvatv_bytes` read MX vs. ATV's `Veh\biplane.phy` as
  model one or model two, into `phy::MxvatvPlane`. That is a `Plane`, plus the three `[Camera]`
  vectors. `phy::mxvatv_model_one(form)` picks the model from the skin's name form (oracle.txt # MD80):
  forms 1, 2, 6 and 7 are model one, and anything else is model two.
  - The model's keys: `ModelOne…`/`ModelTwo…` for the aero geometry, the three max surfaces, each tire's
    radius and centre, and the pelvis box.
  - The wrecks: MXVehicle's `[Wrecks]` keys (`MaxWreckTime` 5, `WreckGravity` 3, `WreckDamping` ones,
    `WreckInertia` (0,0,0,1)) override MX Unleashed's.
- **The step.** In `game_sim::biplane`, SLUS-21104's twins of MX Unleashed's functions:
  - `controls_mxvatv`, `autopilot_mxvatv`, `Engine::step_mxvatv`, `Aero::forces_mxvatv`;
  - MXVehicle's wreck check: `Area::outside_mxvatv`, `throw_mxvatv`, and `Crash::step_mxvatv`, which
    swaps them into `Crash::step_in`.
- **The switch.** `flight::Flight` gains `mxvatv: bool`. `Flight::new_mxvatv(&MxvatvPlane)` sets it, and
  the preforce, forces and crash listeners branch on it.
  - It defaults to false, so every MX Unleashed plane runs exactly the code it ran before.
  - It is kept out of the hash walk and off the snapshot wire (snapshot.rs ignores it on put and sets it
    false on take).
  - `Sim`'s size moved from 12232 to 12240; tests/hash.rs is re-pinned with a line saying why.
- **The proofs.** tests/mxvatv_biplane.rs (7 tests).
- **The oracle.** research/oracle.txt # MD80b: the field map, the rounding, the wreck check in words, and
  six new names: `atv_biplane_read_camera`, `atv_terrain_edge`, `atv_rand`, `atv_smooth_noise`,
  `atv_powf` and `atv_curve_table_ptr`.
- **Follow-ups.** MD99 (re-record and replay MD80a's flights) and MD100 (the switch on the netplay wire).

## What the original does

The steps are MX Unleashed's, with fused multiply-adds in places. The PS2's `adda.s` with a zero
operand turns a `-0` into `+0`, so the fused forms differ from the plain ones only on signed zeros and
on the order of the sums.

- **Controls** (`./kit fn 0x004c30b0 -o atv`).
  - The throttle step is `adda(0, throttle)` then `madd`/`msub(increment, dt)`.
  - The rest is MX Unleashed's: the fade remaps, and each surface moves toward `pad × max` by at most
    `rate × dt`.
- **Autopilot** (`./kit fn 0x004c33b0 -o atv`).
  - The roll is wrapped twice by a fused wrap: `n = cvt.w(a / 2π)`, then `adda(0, a).msub(2π, n)`, then
    the ±π corrections.
  - The pitch error is `mula(ff, turn).madd(elevator, max rate) − rate`.
  - The integral is `adda(0, i).madd(dt, ig·err)` and the command is `adda(0, i).madd(gain, err)`, each
    clamped to ±MaxElevator.
- **Engine** (`./kit fn 0x0049e710 -o atv`). Two changes from MX Unleashed's: the prop's advance is
  `adda(0, angle).madd(k·rpm, dt)` through the fused wrap, and the note's wobble is
  `adda(0, note).madd(0.05 | 0.2, noise)`.
- **Aero** (`./kit dis 0x0049e220 --len 245 -p atv`).
  - Every coefficient is one chain of `mula`/`madda`/`madd`, in its own order.
  - It has three `abs.s`: |α·k| in the round-off, |α| and |e|, two fewer than MX Unleashed's (the
    "two dropped abs.s").
  - The drag's sign is `-(v < 0 ? -1 : 1)`, built from an integer; that is the "extra branch".
  - The PC exe's twin (`./kit fn 0x0040a750 -o pc`) has the same terms and fields, and was read first.
- **MXVehicle's wreck check** (`./kit dis 0x004a4f20 --len 168 -p atv`, the vehicle's vtable slot).
  1. Returns 0 when the vehicle's gate word is clear.
  2. Asks the terrain whether the body's position is within 150 of an edge:
     `x < m || w − m < x || z < m || d − m < z`. A NaN counts as inside, not outside as in MX Unleashed.
     The terrain hands back its extents only on a yes.
  3. On a yes it throws the vehicle toward `(w/2, y, d/2)`:
     - the direction is scaled by `rsqrt(1, |dir|²)`, or replaced by (0,0,0,1) at zero length;
     - the speed is `min(2073.6, w/2) / k`, with `k = 2·sqrt(14.906832)`;
     - the lift is `adda(0, 240/k).madd(16.1, k)`;
     - the gravity scale is halved;
     - the spins are `(0.5, 0.4, 0.7) × 2⁻³¹ × rand()`, and w is 1.
  4. Returns 1.
  MX Unleashed's 2.5 s and 0.3 constants are not in this function. They belong to MXVehicle's
  wreck/recover machine, which is elsewhere and not ported (see below).

## How it was proved

All in crates/game-sim/tests/mxvatv_biplane.rs. The six function oracles run on MD80a's Desert Oasis
image `context/mxvatv-ps2/md80a/biplane.ee.bin`, on the live plane at 0x014de810 (model one). Each
writes the case into the plane's own fields, or through the engine's and aero model's own pointers,
which the test asserts first. Each compares every written field bit for bit:

1. the controls: 10,000 generated cases;
2. the autopilot: 10,000, with the curve table taken from the image;
3. the engine: 10,000, with the smooth noise hooked and the thrust recorded at the body's force entry;
4. the aerodynamics: 10,000, with the force and moment recorded at the body's two entries;
5. the edge test: 10,000 points on the image's own terrain (5760 × 5760), 6,418 of them by the edge;
6. the throw: 10,000 cases through the whole wreck check, with the terrain test hooked to say yes with
   generated extents (zero included), and `rand` hooked to hand out `game_sim::rng::Rng`'s numbers. The
   port's generator draws the same count. The gate's 0 path is checked once.

Plus **the reader**: the disc's `biplane.phy` (copied to `context/mxvatv-ps2/md80b/biplane.phy`) is read
as model one against `biplane.ee.bin` and as model two against `biplane3.ee.bin` (plane 0x01b13790).
Equal word for word:

- Chord, Span, SurfaceArea, AspectRatio and DefaultAngleOfAttack;
- the three max surfaces;
- MaxWreckTime, WreckGravity, WreckDamping and WreckInertia;
- the pelvis box;
- the two shoulder offsets and the model's camera offset.

It also checks that `Flight::new_mxvatv` sets the switch and carries the model's MaxElevator.

**The cases can tell the two games apart.** I swapped MX Unleashed's functions into a scratch copy of the
test:
- all four fail: aero at case 31 (CL `+0` against `-0`), autopilot at 4826, engine at case 7 (the
  prop's angle), controls at 304 (the throttle);
- the engine and controls only fail after adding the signed-zero cases (a `-0` throttle or prop angle,
  `dt = 0`) that the test now generates.

**MX Unleashed's planes are unchanged:**

| Test | Result |
|---|---|
| tests/biplane.rs | 19 pass |
| tests/flight.rs | 5 pass |
| tests/replay.rs | 121 pass |
| tests/hash.rs | 16 pass, with the size re-pinned |

## Not verified / not 1:1

- **No flight replay.** The brief's test, "MD80a's biplane flights replay bit-exact", is not met. The
  MD80a capture csvs are gone from this slot's context/capture/md80a. A replay would also need:
  - Desert Oasis's ground under `Flight`;
  - SLUS-21104's input step (`atv_biplane_input`, matched by shape at 0.90, not compared);
  - MXVehicle's wreck/recover machine.
  What is proved is each step function on the original's own code, above. MD99 holds the replay.
- **MXVehicle's wreck/recover state machine is not ported.** It reads the clock at +0xc84 against
  MaxWreckTime +0xc88, including a `MaxWreckTime − 2.0` test. The ATV plane keeps MX Unleashed's
  recover rule (thrown planes wait 2.5 s and then land; at WreckTime they keep 0.3 of their velocity),
  fed with MaxWreckTime. In MD99.
- **WreckGravity, WreckDamping and WreckInertia** are read and proved equal to the loaded words, but
  `Flight`'s wreck uses them as MX Unleashed's does. Whether MXVehicle's machine uses them the same way
  is unread.
- **The part loader's extra keys** (CheckForCollisions, HighSpeedMinCollisionAngle,
  BodySpring{Stiffness, Damping, MaxPenetration, Restitution}) are not read. Nothing ported uses them.
- **The tires' radius and centre** are read with the model's keys, but not compared with the image's
  tire objects; their field offsets there are not decoded. `FCamFileName` is not kept.
- **Not compared:** the preforce, forces, measure, pose and tire twins (matched by shape at 0.80–1.00 in
  # MD80, read as "no float change"), and SLUS-21104's input step.
- **`rand` equal to MX Unleashed's.** The throw test hooks `rand`, so it proves the arithmetic on rand's
  outputs, not that `atv_rand` is the same generator as `Rng`.
- **The gate word** (+0x308) is assumed set for a flying plane: it is 1 in both images. The port has no
  such word.
- **The terrain edge test** is proved on one terrain (Desert Oasis's, square). The port feeds it from
  `Flight::area`, which comes from the ground the ride is built on, not from SLUS-21104's terrain record.
- **The model pick for unknown forms.** Forms outside 1–10 are taken as model two (the word as
  constructed), which is read from the switch, not run.
- **The switch is off the netplay wire.** A snapshot taken back is an MX Unleashed plane. Fine offline,
  where nothing builds the ATV plane yet. MD100.
- **Nothing builds `Flight::new_mxvatv` in the game yet.** That is MD80e's.
- **The gate ran with `OPENMX_WORLDS` empty.** openmx's disc_worlds test compares its world with the
  user's installed `world_mxu_free/mod.json`. That copy was just reinstalled from openmx-mods, and on this
  branch the comparison reds; this task never touched the stitcher. So `./kit done` ran with
  `OPENMX_WORLDS=$PWD/target/no-worlds` (the comparison skipped). world_first_frame's slow first frame
  passed on a lone rerun (load).
- **The PC exe** was read for the aero step only, against the PS2's. The controls, autopilot, engine and
  wreck check were read on the PS2 alone, with no PC oracle run.

## Next

Nothing for MD80b. MD99 is the replay; MD100 is the wire.
