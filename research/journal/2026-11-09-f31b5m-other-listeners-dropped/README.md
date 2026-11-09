# F31b5m — other listeners dropped

2026-11-09. Task: `./kit plan F31b5m`.

The pure frame now leaves out the frame's other listeners: records other than the bike's (none on
state 4), the five other vehicles' PreProcess, the tail's first other leaf and the manager's own work.
The manager's walk still steps the rider. The sim lists' getters and remove are Rust's
(`tail::list_get` / `list_remove`), proved by a 10 000-case oracle. With every port on and the others
dropped, C15 ride-2 and flip-1 replay bit-exact. The bike and its blocks match the handing run's after
every frame, and the frame reads no byte where the two RAMs differ. FINAL.
