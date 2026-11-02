# S243 — the props gather their own world, the wreck bike in it

2026-11-02. Task: `./kit plan S243`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S197b's props searched a fresh nearby list every tick and met the wreck bike in a search of their own
after the pass. Now each prop's collision object keeps its nearby list from tick to tick, gathered
again only where the original's body wrapper gathers it (its post while awake, its wake, its asleep
check). The wreck bike's hull is a collidable in that same world, so one ray record covers the scene and
the bike and a ray keeps one contact, the nearer. A prop's sweep now carries the init's category list,
which leaves out the wreck bike and the toys. The Sim's own step paths use this; the seats' shared props
keep S197b's stand-in. hit2-2828 -> 2831 stay bit-exact through the new path, and the wreck bike's
distance in the ball's kept list matches the state's. Details in `1-kept-search-and-the-hull-FINAL.md`.
