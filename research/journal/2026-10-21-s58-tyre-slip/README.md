# S58 — tyre slip

2026-10-21. Task: `./kit plan S58`.

The bike never slipped because the port's spin step was a stand-in. It read `WheelFriction` as grip
and never flagged a tire spinning, locked or sliding. S58 ports the original's spin step, surface drag
and spin inertia whole, and proves them bit-exact with the function oracle over 10 000 cases each
(`tests/spin.rs`). S58 also separates the wheel's hub from its force arm: until now
`BodyTireForcePosition` did both jobs, and in the original it is only the arm. On the human's minute,
S32's tick-1 vertical shove went from 0.077 to 0.00086. The brief's four captures were not made: no
capture can replay bit-exact until a run starts from the original's wheel state. That is S60, and
S58 is blocked after it. Details in `1-tyre-slip-PART.md`.
