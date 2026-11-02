# S218 — steep2's part at 189

2026-11-02. Task: `./kit plan S218`.

S179 left S117's steep2 replay parting its path at tick 189 (`ang_hist.prev.x` -1.23 vs 0.72). It was
already closed when this task ran. S177 (merged after S179) found that steep2's row 189 is a late row: the
recorder read it a tick out of step, so it holds tick 190's ride word for word. S177 marked it in
`S58_OWN_STATES` (`late: &[189]`) and set steep2's ratchet in `S58_CASES` to every tick (`u64::MAX`). No
sim code changed. Details in `1-steep2-row-189-FINAL.md`.
