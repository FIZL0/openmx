# S197a — props struck in every step path

2026-11-02. Task: `./kit plan S197a`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S197 gave only single-player `Sim::step` the bike's strike on the knockable props. Now `Sim::step_all`
(a session of several riders), and `Sim::step_bumped` / `Sim::step_among` (the one-player Sims online
and split-screen seats are, which openmx's bike loop goes through) strike too, each rider in its own
frame order, and the props step after with the riders as strikers. Proved by a port-only test (no 2p
knock of the original was captured): in every path the struck free09 ball wakes, its mirrors name the
striker, and its body after the tick is bit for bit the one-player `Sim::step`'s. Done; details in
`1-every-path-FINAL.md`.
