# S278 — rider touching flag

2026-11-05. Task: `./kit plan S278`.

F41 found that the bike's wreck clock waits on one word the rider writes: its "a ragdoll part is
touching" flag (the bike's `SETTLES`, +0x920). S278 traced that flag back to where it is made and ported
the four pieces between the rider's parts and the clock. These are the wreck arm's walk, the parts'
clear, the impact sum and the hand-over into the bike's record (`game_sim::handling::rider`). Each is
proved by an oracle, and both C15 replays stay bit-exact with the port handing the flag over. What
sets the parts' touched words is the push-out of the rider's ragdoll world's bone collisions. They
have MX Unleashed's layout. That world, and the rest of the rider's mode machine, are too big for this
task: filed as S278a and S278b. Details: `1-rider-touching-flag-FINAL.md`.
