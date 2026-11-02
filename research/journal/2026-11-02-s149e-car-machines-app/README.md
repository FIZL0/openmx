# S149e — the car machines in the app

2026-11-02. Task: `./kit plan S149e` (split from S149).

The three cars are machines in the shell: a fresh `game_sim::car::Machine` is built off the car's .phy
and model as the original's constructor builds it (checked field by field against three RAM images of
the parked cars), and the shell switches into each from the pause menu or a tape's `switch` line and
drives it with the bike's follow camera. Drawing, the car cameras, the engine sound and the dust are
not done; they are filed as follow-ups. See `1-car-machines-app-FINAL.md`.
