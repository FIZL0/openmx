# P89 — biplane diagonal stick

2026-10-21. Task: `./kit plan P89`.

A held diagonal on a real stick fought itself because the original reads each axis through a square
±0.3 dead zone and cubes each one, leaving a round gate's corner at a fifth of the pull and of the roll —
and that fifth of an aileron is outside `AileronDeadZone`, which switches the pitch loop off, so the pull
fell below the loop's own trim and the nose dropped. The keyboard's arrows were fine. Under the New
scheme the plane's step now reads the stick round (`vehicle::round_stick`), keeps the pitch loop at any
aileron and fades its coordinated-turn feed-forward out as the roll leaves the dead zone; every corner
and half tilt pulls and rolls together on every settled tick. Original is untouched.
