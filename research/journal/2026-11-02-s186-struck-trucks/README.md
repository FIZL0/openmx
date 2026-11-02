# S186 — the struck HasPhysics trucks

2026-11-02. Task: `./kit plan S186`.

The struck free06 trucks now become rigid bodies in the port. `game_sim::physics_car` loads the
`.pco`, the `m.col` and the `f.col`. It provides the wake, the world node's step and the settle, then
puts the truck back on its path `ResetTime` after it comes to rest. `Traffic` drives this through
`strike`, a snapshot section and the hash. I checked it against the interpreter on s89-along. Both trucks
match bit for bit on every frame from the wake to the put-back. Which searches strike a truck is not
wired yet, and there is no PCSX2 capture; both went to S186a. Details are in the numbered file
(FINAL).
