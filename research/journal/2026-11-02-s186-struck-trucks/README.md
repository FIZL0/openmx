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

S251 (file 3, PART, blocked): the original's strike-tick list holds the truck's path mesh as a mover,
not its `f.col` as a body, and the port already builds those two contacts bit for bit. The rows part
on the mover's velocity. The original reads the car's motion after the wake moved its node, and the
port's wake puts the node about 0.5 ft from the original's. The f.col code is parked on
`wip/S251-fcol`.

S252 (file 3, FINAL): every search that strikes a truck's path mesh now wakes it. A search marks the
world's `Prop` (`strike_heard`), and `Traffic::heard` takes the marks at `frame_late`. The ragdoll's
copied cars carry their prop. A PCSX2 capture of the biplane parked in ctruck's way wakes the truck
on 11092, and so does the port, with the plane on the capture's rows (two ticks later on the port's
own creeping plane).

S252a (file 4, FINAL): PCSX2 captures of free06's ctruck waking on a downed bike's wreck. In one run
the wreck lies still and in the other it rolls in. The port wakes the truck on the original's tick
and at its point in both runs, with the wreck pinned to the capture's rows and with the port's own
wreck. In the original the strike stands the path mesh down mid-search: the wreck keeps no entry
for the mesh, and the woken truck's own search later that tick meets the wreck. The ragdoll's
strike was captured but has no test yet (S272).

S252b (file 5, FINAL): the port's plane crept short of the original's after the put-back because
the test left it unflown, and an unflown plane's controls hold the brake on. The original's state
has it flown with the brake off. `controls_off_the_save_state` reads the gate, the pad and the
surfaces from the image. The port's own plane now matches the capture's rows bit for bit and wakes
ctruck on 11092.

S273 (file 6, FINAL): the ragdoll's strike now has a test. The port's down rider takes its 18 bones from
S252a's `doll` state, and ctruck wakes on the original's 11962 at the original's point. That holds with
the bones pinned to the capture's rows and with the port's own bones. With the bones 100 ft up, it drives on.
