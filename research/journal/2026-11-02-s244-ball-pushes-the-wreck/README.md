# S244 — a ball's push on the wreck bike, and the Sim's wiring

2026-11-02. Task: `./kit plan S244`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S197b left two things unproved: the ball's push on the wreck bike's body, since hit2 matched with or
without it, and `Sim::wreck_strike`'s wiring, which no test covered. Two new free09 captures in PCSX2 copy 3
cover them. hit5 (ball 4 thrown into the wreck from behind) has a ball's impulse throw the wreck bike in
the props' step at 2855. hit3 showed that S197b had the wreck's rays wrong: they *do* strike a ball. That
is now ported (`prop_wreck::wreck_rays_on_props`, `strikes`, `Sim::wreck_pushed_back`), and hit3-2832 fails
without it. hit2, hit3 and hit5 match bit for bit, by hand and through a `Sim`. Details in
`1-push-and-wiring-FINAL.md`.
