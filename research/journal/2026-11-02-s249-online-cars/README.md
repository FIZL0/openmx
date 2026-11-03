# S249 — cars online

2026-11-02. Task: `./kit plan S249` (an S149e follow-up).

Before this, an online ride had no cars: a car pick in the pad's switch byte was sent as "no pick",
and no peer's garage held a car. Now a seat can switch into the buggy, the monster truck or the
baja online. The switch goes over the wire, and every machine drives the car as part of the seat's
rolled-back state. The one numbered file has the details (FINAL).
