# V105 — split screen toy

2026-11-04. Task: `./kit plan V105`.

A ride with two views on a disc freeride track now reads its `<stem>_ss.toy` (out of
`Tracks\<stem>_ss.res`) wherever it read `<stem>.toy`, as the original loads the whole `%s_SS` track
there. free09_ss's windmills then always step (AnimateWhenNotVisible True), and free11_ss's first three
cars run at their own TimeScale. Only the `.toy` is swapped. Done; details in `1-split-screen-toy-FINAL.md`.
