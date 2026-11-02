# S242 — a beach ball's rays strike another ball

2026-11-02. Task: `./kit plan S242`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S197b left the props' rays on other props' meshes out. A PCSX2 capture of free09's ball 1 dropped on
ball 0 at rest (drop2) shows how the original does it. An awake ball's rays meet any other ball's
`f.col`, at rest or awake. The struck ball is woken, and if it was not filed it goes on the end of the
props node's list and is searched in the same pass, so its rays strike back. Mirrors land at the head
or the tail of the struck list depending on node order. Ported as `prop_props::strike_props`, which
runs after the collections' own searches in node order (`Object::filed_at`), and `step_named`, which
hands a collection the props its list names. Five drop2 ticks match bit for bit, lists and bodies, and
so does a run of five. S195's knock replay now agrees for ball 2 until 3273 (it was 3092), because the
game's "struck again at 3093" was ball 4's rays. It parts at 3274 on the node's shared listener list,
which is filed as a follow-up. Details in `1-ball-on-ball-FINAL.md`.
