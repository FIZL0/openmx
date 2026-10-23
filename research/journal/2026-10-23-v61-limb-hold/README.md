# V61 — limb hold

2026-10-23. Task: `./kit plan V61`.

V10's "limb controller" is not a limb solve. It is one of the riding ragdoll's four hold pins, and
the weight it reads off its bone is that bone's inverse mass, written by the body's mass setter. The
pin's solve was already ported and proved by S51. The original has no inverse kinematics on the drawn
rider, and so no clavicle or wrist solve.

It holds only while its ragdoll is placed, and the ragdoll is not placed while the body machine is
`Posed`: every gesture, every no-hander and a backflip's hold draws the clip untouched. That rule was
proved on recording2 (1499 of 1499 riding ticks), and so were the weights (to the bit). The rule is
now wired into the shell's riding ragdoll and into the default clip draw's stand-in hold, which
replaces the port's distance window (`holding`, `HOLD_WITHIN` and `LET_GO_AT` are deleted).

`aim` stays as the stand-in until S111 turns the riding ragdoll on, and V63 then deletes it. The
details are in `1-limb-hold-FINAL.md`.
