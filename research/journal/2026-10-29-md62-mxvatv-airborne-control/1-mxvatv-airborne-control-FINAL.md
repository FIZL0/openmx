# 1 — mxvatv airborne control (FINAL)

Task MD62. 2026-10-29.

## What I did

- `handling::airborne_control` (crates/game-sim/src/handling.rs): SLUS-21104's airborne control
  (`./kit fn 0x004e65d0 -o atv`, oracle name atv_bike_airborne_control), bit-exact, with its
  `GasGyroTorque` term and the rider-stunt-id stick scaling. Inputs beyond MX Unleashed's `Step` in
  `handling::AirIn` (stunt id, gas, `Gyro`).
- `handling::Gyro`: the bike's `+0x1290`/`+0x1294`, which are MX vs. ATV's **Brake Gyro / Gas Gyro
  options** (the PC exe's strings "Brake Gyro Value"/"Gas Gyro Value"; on the PS2 copied from the
  player's settings +0x134/+0x138 when the bike is made, `./kit fn 0x004de920 -o atv`; another
  setup `0x004de160` writes 1/0). state04's bike holds 1, 0: brake on, gas off — `Gyro::default`.
- `Phy::gas_gyro_torque` (read from `[Airborne Control] GasGyroTorque`; MX Unleashed's files have
  none, so 0) and `Phy::gyro` (the default; nothing sets it yet — P226 filed for the settings row).
- The ride (ride.rs) calls `handling::ride_airborne_control` instead of `air::airborne_control`
  whenever `Ride::mxvatv` is on (Handling::MxVsAtv); the original's path is untouched.
  `ride_airborne_control` stands a stunt id in by range (see below).
- air.rs: `DEAD`, `SLOW`, `NOTHING`, `UP`, `map_clamp`, `normalize`, `cross` made `pub(crate)` so
  handling.rs uses the same proved helpers; no other change there.
- research/oracle.txt: the step's field map as `#` lines under the MD40 block.
- Nothing taken from opentennis: no air control there.

## What the original does

Read beside the PC's (`./kit fn 0x006024e0 -o pc`, same branches; PC fields +0x20 higher in the
high range) and MX Unleashed's (`./kit fn 0x00256e50`). Differences from MX Unleashed's, all ported:

- The authority ramp is the same inline map-and-clamp, and the ground early return the same.
- The stick moment is left out only when the backflip's torque (+0x1194) **and** the 360's (+0x11d0)
  both went in; MX Unleashed's leaves it out under the backflip's alone. Under the backflip's ramp
  (+0x1198) yaw and roll are zeroed and pitch kept, then all scaled by the ramp (`vmulx.xyz`).
- "Trick held" is the rider's stunt id +0x1524 < 41 (x0.5 pitch, x0.4 yaw/roll, `stick*(k*acm)`),
  not the trick clocks' flag; unscaled is `acm*stick`.
- Damped when |heading y| >= 0x3f708fb2 (cos 20°, a literal; MX Unleashed reads a sine-table entry,
  0.93934375), |lean| >= 0.1, the backflip ramp, the 360 ramp (+0x11d4), or authority > 0 (after the
  decrement). Otherwise auto-level with `adda`/`madd` (z: unroll, y: unyaw) and `adda`/`msub` for
  the two dead-band damping clauses.
- Gyros only with stunt id >= 46: Brake Gyro option on and either brake non-zero → + BrakeGyroTorque;
  else Gas Gyro option on and gas (+0x318) non-zero → − GasGyroTorque. Ids 41..45 (the backflip's
  stunt is 41, +0x13b8 in state04) get neither scaling nor gyro.
- Moment through the body's vt +0x78 (atv_body_add_moment, `vadd.xyz`); the lean force, with stunt id
  >= 46, neither torque in, speed > 10, as MX Unleashed's (`normalize(UP x vel) * force*lean`), through
  vt +0x80.

## How it was proved

- Function oracle, 10,000 generated states, `mxvatv_airborne_control_is_the_originals` in
  crates/game-sim/tests/mxvatv_air.rs, off context/mxvatv-ps2/state04.ee.bin (the MD40 harness:
  bike object laid out at the top of RAM, the original run in the interpreter with its own body
  vtable, the bike/body regions compared word for word, and a check that it wrote nothing else).
  Every branch is tallied and must be hit ≥ 200 times (stick left out / scaled / whole, damped /
  levelled, brake gyro / gas gyro / none, lean force / none, on the ground). Inputs aimed at the
  thresholds: heading y exactly cos 20°, lean at ±0.1, angular velocity at ±0.1, speed at 10, stunt
  ids 40/41/45/46/0x2e/0xff. A one-ulp nudge of the unroll torque fails it at state 3 (checked).
- Wiring: crates/game-sim/tests/mxvatv_ride.rs — on MxVsAtv a pinned throttle in the air with Gas
  Gyro on pitches the bike nose-down against Gas Gyro off; on Original the two options give
  bit-identical rides (not read).

## Not verified / not 1:1

- **The ride's stunt id is stood in by range, not the rider's real one.** The port's rider keeps no
  MX vs. ATV stunt id: a held trick (`Step::stunt`) reads as id 0, a running backflip clip
  (`Flip::clip != 0`) or 360 clip as 41, anything else as 0x2e. Only the range matters to this step,
  so it is exact *if* the ranges line up; not proved against a capture: the original's backflip stunt
  can be refused by the rider (`start_stunt` answer), the port always takes it (as MD40 does), and the
  360's stunt id was not read (assumed in 41..45 like the backflip's 41).
- **The gyro options are always the game's default** (brake on, gas off): nothing in the shell sets
  `Phy::gyro` yet, so GasGyroTorque never acts in a real ride. Filed P226. Where the player's settings
  struct (+0x134/+0x138) gets its values was not traced past the bike setup.
- **No live capture.** The step is proved by the oracle alone; the ride on MxVsAtv as a whole (MD40's
  mx_*.phy remap plus this) is not compared with a PCSX2 ride of MX vs. ATV.
- **`Step::heading` is taken to be +0xbe4.** The oracle pokes `heading.y` there by construction; that
  +0xbe4 is the same forward-y the port's `heading` carries is by analogy with the PC (+0xc04) and MX
  Unleashed (+0x414), not checked on a state.
- The body's moment-listener count is 0 in the harness, so the listener calls in vt +0x78/+0x80 are not
  exercised (as in MD40).

## Next

Nothing for MD62. P226 for the options row.
