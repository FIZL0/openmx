# S241 — the plane strikes a prop

2026-11-02. Task: `./kit plan S241`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

The plane lanes (Sim::step's, and openmx's Garage/fly_lane) stepped the knockable props with no
strikers, so the biplane rolled through free06's EyeBalls. A PCSX2 capture of the plane rolling into
ball 0 (hit0) showed the original's way: the plane's own rays strike the ball's f.col (its entry names the
ball, the ball's list takes the mirror first and the ball wakes), the balls' rays strike the plane's hull
as they strike a wreck bike's (S197b), and the plane's world steps before the props'. Ported
(`Flight::frame_props`, `Scene::knocking`, `prop_wreck::struck_on`) and wired into both lanes. The
search, the plane's and the ball's resolves and the hull pass are bit-exact against the hit0 states;
a Sim-level run wakes the ball on the original's tick. Details in `1-plane-on-ball-FINAL.md`.
