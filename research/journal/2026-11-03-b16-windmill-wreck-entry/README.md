# B16 — the windmill wreck's tick and the bones' entry

2026-11-03. Task: `./kit plan B16`.

R14 found that, from its `south-bones-before.ram`, the port wrecked a tick early and its bones
entered the wreck with the bike's velocity. The tick was the test's doing. The script teleports the
bike but not its nearby search, so the original's first tick still collides against the list from
where the bike was. Seeded the same way, the port falls on tick 1 and is thrown on tick 2, bit for
bit with the capture.

The bones' velocity was a side effect of the early tick. With the tick right, a real gap shows: the
original's body actor rides one more tick on the thrown bike and enters the wreck a tick after the
verdict. The port enters on the verdict's tick. Filed as B19.

Test: `tests/replay.rs the_windmill_wrecks_the_bones_with_the_bike`, plus the new helper
`seed_bike_search_kept`.
