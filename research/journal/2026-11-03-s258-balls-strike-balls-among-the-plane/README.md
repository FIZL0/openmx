# S258 — balls strike balls among the plane

2026-11-03. Task: `./kit plan S258`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S241 left the ball-against-ball entries of hit0-11061 (free06, the plane among the knocked EyeBalls) unported.
S242 had since ported ball-on-ball strikes (`prop_props::strike_props`) for free09, and the plane lane
already runs them through the props' search, so what was left was proving them here. The proof found
one difference: the plane-hull pass (`prop_wreck::struck_on`) appended a ball's hull entries after the
mirror a later-filed ball had put on it. In the original they are part of the ball's own search and
come first. The pass now walks the node's order and puts a prop's entries before those mirrors. Against
hit0-11061, every ball's whole list matches, and after the original's node tick so does every ball's
body and the plane's velocity and rates. Details in `1-ball-on-ball-on-11061-FINAL.md`.
