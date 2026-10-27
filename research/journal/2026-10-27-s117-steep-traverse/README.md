# S117 — steep traverse

2026-10-27. Task: `./kit plan S117`.

S58's traverse capture never held a steep side slope. This task recorded two held traverses of S106's
46–48° face on free09 in the original, from teleported save states: full gas from standing, the stick
part uphill. They are cases `steep` and `steep2` in `S58_CASES`. The port's tire side law needs no fix.
Both rides are bit-exact for as long as both wheels are on the face (74 and 97 ticks, side force up to
1.8 times the wheel's load). steep's slip flags agree on all 250 ticks. The first thing to part is the
chassis' angular acceleration on the first tick both wheels are off the face, which is air control.
That is filed as S177. One harness fix came out of it: the first tick after a state load rides on the
controls the state was saved holding. Details in `1-steep-traverse-FINAL.md`.
