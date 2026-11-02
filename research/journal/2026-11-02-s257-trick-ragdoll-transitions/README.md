# S257 — trick ragdoll transitions

2026-11-02. Task: `./kit plan S257`.

How a trick starts and ends on the original's riding body. A stunt runs in the body's posed mode (3),
not in riding mode pulled toward the clip: the ragdoll is let go and the clip is drawn as written. It goes
posed the tick after the vehicle animation's from_table goes up (a backflip's holding: the same tick), and
riding again the tick after it drops. The riding entry starts each bone at the clip's node of the last
posed tick, moving by that node's last move. A bailed trick goes from posed straight to the wreck. The port
now does the same on recording2's six tricks (the exits went from 2.1 ft off to 0.03), and its shell lets
go on the original's tick, no longer one late. Details in `1-trick-ragdoll-transitions-FINAL.md`.
