# S197b — a ball strikes the wreck bike

2026-11-02. Task: `./kit plan S197b`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S197 filed only the riding bike's body rays on the props. A PCSX2 capture of a free09 ball striking the
tumbling wreck bike (hit2) showed the original's way round: the wreck bike's rays leave props out, and it
is the ball's own rays that search the wreck's f.col. That is ported (`prop_wreck.rs`), both bodies pushed
(the ball in its props step with the wreck as a striker, the wreck in its own world step with the ball as
an other), and wired into every Sim step path. Bit-exact against hit2-2828 -> 2831. The rest of the brief
(rider/ragdoll and plane striking props, props by props, the broad-phase gather) is filed as follow-ups.
Details in `1-ball-on-wreck-FINAL.md`.
