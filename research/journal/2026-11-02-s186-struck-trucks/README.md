# S186 — the struck HasPhysics trucks

2026-11-02. Task: `./kit plan S186`.

The struck free06 trucks now become rigid bodies in the port. `game_sim::physics_car` loads the
`.pco`, the `m.col` and the `f.col`. It provides the wake, the world node's step and the settle, then
puts the truck back on its path `ResetTime` after it comes to rest. `Traffic` drives this through
`strike`, a snapshot section and the hash. I checked it against the interpreter on s89-along. Both trucks
match bit for bit on every frame from the wake to the put-back. Which searches strike a truck is not
wired yet, and there is no PCSX2 capture; both went to S186a. Details are in the numbered file
(FINAL).

S186a (file 2): a riding bike's body rays now strike a truck (`Traffic::strike` from
`Sim::car_contacts`). The `HasPhysics` cars update after the bikes (`Traffic::frame_late`). With
both, S207's ctruck and S219's rear captures meet the truck on the original's tick, and rear wrecks
as the original does. The strike tick's push still parts. The f.col's two-way contact is S251, and
the other searches' strikes are S252.

S252 (file 3, FINAL): every search that strikes a truck's path mesh now wakes it. A search marks the
world's `Prop` (`strike_heard`), and `Traffic::heard` takes the marks at `frame_late`. The ragdoll's
copied cars carry their prop. A PCSX2 capture of the biplane parked in ctruck's way wakes the truck
on 11092, and so does the port, with the plane on the capture's rows (two ticks later on the port's
own creeping plane).
