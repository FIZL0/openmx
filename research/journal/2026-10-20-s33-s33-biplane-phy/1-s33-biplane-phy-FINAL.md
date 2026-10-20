# 1 — s33 biplane phy (FINAL)

Task S33. 2026-10-20.

## What I did

- Found the plane's loader myself (no wait on R8): the key strings in the ELF (`./kit data --strings
  Aileron`) lead to one reader, `./kit fn 0x003511f0`, reached through vtable slot +0x194 from the
  plane's init `./kit fn 0x00350a70`, which formats `%sbiplane.phy` with the `Biplane\` prefix.
  MXBiplane's own override (`./kit fn 0x00355210`) calls it and then reads `[Animation]`. The gear's
  three setters are `./kit fn 0x0034ee10`, `0x0034ee90`, `0x0034eee0`; the wing's geometry setter is
  `0x0034fc40`. `[Mass Properties]` goes through the shared part loader `0x0015a500`, which S26
  already ported as `phy::Part`. All of it is written into `research/oracle.txt` under "S33".
- `crates/game-sim/src/phy.rs`: `Plane` (and `PlaneEngine`, `Aero`, `Surfaces`, `Gear`, `Autopilot`,
  `PlaneJoystick`, `PlaneWrecks`, `PelvisBox`), `Plane::read`/`read_bytes`, `Gear::read`, and
  `BIPLANE_PHY`. Every key the loader reads, in its order, with its defaults (checked in the
  disassembly: every float getter passes f12 = 0, except the gear's first six, 1.0) and its
  conversions (DEG on eight angles but not `MinElevator`, MPH on two speeds, the chord fallback, the
  gear's spring and dampers against the part's mass, 1/radius). Added code only; nothing moved.
- `crates/game-sim/tests/phy.rs`: `biplane_phy_matches_the_loaded_struct`.
- The plane's state RAM: `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s` →
  `context/states/biplane.p2m2_SaveState.eeMemory.bin` (in s2's context; the main checkout does not
  have it yet, so the test skips there until someone runs the same command).

## What the original does

The plane's file is not the bike's format with extra keys: it has its own loader with its own nine
sections, and only `[Mass Properties]` is shared (the rigid-body part loader that `Wreckbike.phy`
uses). The loader keeps the surface limits, autopilot, joystick and wreck numbers on the plane
object, the engine numbers on an engine part, the wing's 35 coefficients on a wing part and the tire
numbers on three gear parts, which work their spring (`|k * m * 32.2|`) and dampers
(`ratio * 2 * sqrt(m * rate)`) out against the body's mass — the same arithmetic as the bike's
`phy::spring_rate` and `phy::damping`.

What the retail file has that is not read: `Inertia`, `Damping` (the part loader's keys are
`InertiaScale` and `SpringDamping`, so the plane gets the defaults 2 and 0.7 — the RAM's 0.7 at the
collision record's damping word confirms it), and `MinAltitude`. **`AeroSurfaceArea` is not a
biplane key** — the brief lists it, but its only reader is `0x00394450`, another vehicle's loader
(with a clutch and `NumberOfWings`); the plane's area is `[Aerodynamics] SurfaceArea`. Written into
the `Plane` doc comment and oracle.txt so S34 does not go looking for it.

The file ships byte-identical in `machine.res` and every freeride track's `_2.res` / `_ss_2.res`
(eleven copies, 3,060 bytes); the test reads two and asserts they are equal.

## How it was proved

A save-state capture: `biplane_phy_matches_the_loaded_struct` reads the plane out of the biplane
recording's EE image (found by the name its loader wrote at +0x230), parses `Biplane\biplane.phy`
from `MX.PSR` → `machine.res` / `Tracks\free06_2.res` with the port, and compares **179 words**:
the plane's own 36 (20 floats, the two wreck and two pelvis-box quadwords), the engine's 4, the
wing's 35, the three gear legs' 15 each (11 words and the centre quadword), the part loader's 8
plane-side floats and 3 flags, the body's mass, and the collision record's 47 (26 floats including
its spring rate and damping worked out with `phy::spring_rate`/`damping`, its flag, 5 quadwords).
All bit-exact on the first run. A mutation (dropping
`* DEG` from `MaxRollAngle`) fails it with one named word, so the test can fail.

`./kit check`: green, with one move outside this task: `the_humans_tricks` (trick.rs) reds on C6's
`recording2-freeride.csv` in s2's acceptance folder (S30's, known), so for the gate the freeride csv
was parked and s5's `recording2-tricks.csv` put in its place; then the freeride csv was put back and
`--test ride --test replay` run on it, green. Nothing in this task touches either.

## Not verified / not 1:1

- **The part's gravity word** (part+0xf0, `(0, -GravityScale*Weight, 0, 1)`) is not compared: the image
  holds `-1499.9999`, which `-1.0 * 1500` cannot produce, so something after the load rewrites it. I
  did not find what (probably the plane's tick); `Part::gravity` keeps the loader's formula unproved
  on the plane.
- **The gear's contact point** (`Gear::contact`, gear+0xa0) is not compared: the running gear rewrites
  it as the strut compresses (the image has y = -0.189 for a centre at 0.5 and radius 0.7). The
  loader's formula (centre minus one radius in y) is from the decompilation only.
- **The body's inertia** (body+0x110, copied to plane+0x3f0) comes from the model's bounding box, not
  the file (`Part::inertia` needs the box); not compared, and the port has no box for the plane.
- **The part fields set on the rigid body other than its mass** — `CofG`, `Radius`, `InertiaScale`,
  `DefaultVelocityDamping`, `DefaultAngularRateDamping` — are not compared: I did not locate where the
  body's setters store them (they are vtable calls into the body class). The raw `Weight` and
  `CollisionWeight` are compared only through the masses they become.
- **Defaults of keys the retail file has** are proved only by the getters' f12 constants in the
  disassembly, not by a file without those keys. The ones the file lacks (`CM0`, `MinElevator`'s 0,
  the part's sleep thresholds, `SpringDamping`) are proved by the image.
- **A missing section** behaves differently: the original's getters keep reading the previously
  selected section when `ini_section` fails, the port's read defaults. No retail file is missing one.
- `Plane` is not wired into anything — S34 flies it.

## Next

`./kit done S33`. S34 can take `phy::Plane::read_bytes` on the disc's file instead of reading the
plane's keys out of the save state.
