# 1 — air gas gyro (FINAL)

Task P255. 2026-11-02.

## What I did

- `air::gas_gyro` (crates/game-sim/src/air.rs) and `air::GAS_GYRO_SHARE = 0.7`: in the air, no trick
  held, no brake held, the pitch lane of the chassis' moment takes `0.7 × BrakeGyroTorque × gas` off
  (`-x`), `gas` the rate-limited throttle (`Held::gas`, what the engine reads). Run in
  `Ride::frame_searched` right after `air::airborne_control`, only on the original's handling and only
  with `Phy::air_gas_gyro` up (MX vs. ATV's handling has its own gas gyro, MD62, left alone).
- `Phy::air_gas_gyro`, false from the reader. The app sets it on every bike file it rides
  (`ports_own_bike` in main.rs: the local stock bike and the other players', the online drop-in's
  and the lobby seats'), so the modifiers' rebuild from the stock files keeps it; `--no-gas-gyro`
  turns it off. It is on `Phy`, not on `Ride`, so no snapshot/hash/wire pin moves; both machines of an
  online ride run the same app, so they agree (a `--no-gas-gyro` on one machine only would desync —
  the flag is for tests).
- The brief named `crates/openmx/src/settings.rs`; there is no settings screen row for this (the ask
  was a behaviour, the way S234's flips were), so no such file.

## What the original does

`./kit fn 0x00256e50` (the air control the port's `air::airborne_control` is proved against): the
gyro term is `if (+0xa0c == 0 && (+0x650 != 0 || +0x654 != 0)) m.x += +0x6ac` — the two brakes and
`BrakeGyroTorque`; no throttle field is read anywhere in the step. So the original's gas does nothing
to the pitch in the air beyond what the drivetrain does, and nothing here is a port of the disc's.
No PCSX2 capture was taken: the decompile already shows the step has no throttle input.

**Which way the brake gyro turns:** `+x`, and every disc bike's `BrakeGyroTorque` is positive (400 on
the 250s, 900 on the cars' files) — that pitches the nose **down** (the backflip's lane is `-x`).
Measured: a 30-frame drop on the brake ends with the forward axis' y at −0.63, coasting 0. So "pulls
back" is the other way from the brake: the gas goes `-x`, nose up, as MX vs. ATV's gas gyro
(`- GasGyroTorque`) and a real bike's do. The brief's "(like the brake does)" reads as "the same kind of
push, a little weaker", not the same direction.

## How it was proved

- crates/game-sim/tests/air_gas_gyro.rs: the same 30-frame drop coasting / full gas / brake with the
  switch on: the brake's pitch rate is +, the gas's −, |gas − coast| between 0.5 and 1 × |brake −
  coast| (the gas ramps in through the rate limit); the nose ends higher than coasting on the gas,
  lower on the brake. With the switch off the gas drop is bit-equal to the coasting one, and a braked
  drop is bit-equal with the switch on or off. The step alone: 0.5 throttle gives −140 on 400, and a
  brake, a trick, a shut throttle or the ground give nothing.
- crates/openmx/tests/gas_gyro.rs: the shell on free09's big jump flat out (`--trace-air`): with the
  switch on the pitch a second into the air is more than 15° above `--no-gas-gyro`'s.
- `./kit check` (the gate), and game-sim's replay tests untouched (the switch is off there).

Measured on free09's jump (`--trace 140 --trace-gas --trace-air`, race125p): off, the nose drifts
from 31° to 21° over two seconds of air; on, it climbs about 12° every 10 frames to 85° and the
rider is thrown by the air pitch limit about 1.3 s into the air. That is the ask (full gas for a whole
big jump stands the bike up, as holding the brake noses it over), but it is a big change from the
original's feel: a rider who holds the gas off every lip now has to let off in the air.

## Not verified / not 1:1

- The whole feature is the port's: the original reads no throttle in the air.
- 0.7 is the brief's "~0.7" of the user's "a little less"; not tuned by anyone riding it.
- Three shell tests held full gas through the air for other reasons and changed with it; they now
  pass `--no-gas-gyro`: train.rs (all four rides, laid out on the original's air control and on
  S170/S191's captures) and shadows.rs' high jump (it was air-wrecked before tick 130). One of
  them, full gas into the train's side, came out 71.7 ft through the far side of the train with the
  gyro on — a collision hole, filed as B3.

## Next

None.
