# S247 — car engine sound

2026-11-02. Task: `./kit plan S247`.

A driven car (buggy, monster truck, baja) is now heard through its own `carengine.aud` profile, as
the original's keyed-loop voice plays it. The drivetrain's sound pitch picks the revs, the revs key
each loop's volume and pitch, and the throttle sets the on/off-gas volume ramps. The car's step now
samples the engine's shared smooth noise for that pitch, where it used to get zero. A new test,
`crates/openmx/tests/car_engine_sound.rs`, drives each car on the disc and checks every loop's voice
against the car's revs. Details in the numbered file.
