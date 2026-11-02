# S220a — the Kiowa's measure listener and crash test

2026-11-02. Task: `./kit plan S220a`.

I ported the Kiowa's once-a-frame measure listener into `game_sim::kiowa`. Each part is bit-exact by
oracle on S148's live helicopter (S148's Kiowa state in `context/states/`):
- `Measure::flight`: the speed, body-frame velocity, world rates, alpha and beta, the Euler angles and
  the matrix with its forward and up rows. It is the biplane's measure, plus a clamp and a sign-flip
  hysteresis on beta. `Measure::readings` hands S220's `Readings` over.
- `Measure::touchdown`: the touchdown latch that plays the landing sound.
- `Crash::step` and `rotor_in_ground`: the measure's wreck slot (vt+0x1b4). It throws the helicopter
  back from the map's edge, crashes it on a rotor in the ground, a struck rotor or a fast contact, and
  runs the wrecked clock to the blade throw and the put-back.
- `spin_blades`: the one piece of the MX override's four extra calls that is simulation (the rotor's
  angle). The other three only copy figures into presentation records.

One of S148's labels was wrong: the controller's "pilot flag" is the collision record's touching flag.

The wreck and unwreck slots' effects on the body, the blade throw and the put-back are S220a1.
Details in `1-kiowa-measure-FINAL.md`.
