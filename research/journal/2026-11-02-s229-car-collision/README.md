# S229 — car collision

2026-11-02. Task: `./kit plan S229`.

The cars now have their collision object. `game_sim::car::Collision` (the Kiowa's class) is built off
`[Mass Properties]` and the car's `m.col`. `Machine::step_struck` searches it before the world steps,
resolves its contacts with impulses ahead of the car's forces each substep, refills its nearby list in
the post, and sets the measure's terrain switch (body rays search the terrain once the car tips past
45° off the ground's normal). The truck's run into free07's scenery (s149b truck.csv fc 720–727) now
replays bit-exact, row by row and carried, so `STRUCK` is gone from car_drive.rs. Baja and buggy stay
exact. The shell's car steps with it too. The tire contact sort is split off as S229a: no captured row
has one. Details in `1-collision-object-FINAL.md`.
