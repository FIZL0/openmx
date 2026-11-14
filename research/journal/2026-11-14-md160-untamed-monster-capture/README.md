# MD160 — untamed monster capture

2026-11-14. Task: `./kit plan MD160`.

I captured the monster truck in MX vs. ATV Untamed (SLUS-21701) in Free Ride on Rocksbury, with PCSX2 copy 3
and menus driven by tape. Kept: states at spawn and in the air, and seven PINE drives (throttle ×2, steer,
jump, two-jump roll, wreck ×2) recorded at Untamed's car offsets. The wreck drive proves live that Untamed's
body integrate damps the rates with `powf(d, 0.333/h)`: in the air the rates decay ×0.85219 per tick from a
stored damping of 0.999. Nothing was ported. Details in 1-untamed-monster-capture-FINAL.md.
