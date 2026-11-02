# P255 — air gas gyro

2026-11-02. Task: `./kit plan P255`.

The user asked for the gas held in the air to pull the bike back, like the brake does but a little
less. MX Unleashed's air step reads the two brakes and never the throttle, so this is a port-only
switch, `Phy::air_gas_gyro`, off in game-sim (the original to the bit) and on in the app
(`--no-gas-gyro` turns it off). The brake gyro turns the nose *down*; the gas turns it up at 0.7 of
the brake's torque, times the rate-limited throttle. Done; details in `1-air-gas-gyro-FINAL.md`.
