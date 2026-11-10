# MD37d — atv in game

2026-11-10. Task: `./kit plan MD37d`.

MD37d puts MX vs. ATV's quad in the game: in the garage, the machine pick and the pause Switch To
Machine, ridden with the pack's clips, cameras, sound and particles. The first session found that two
things it needs do not exist yet. Nothing builds a quad from its `.phy`: every quad so far is seeded from
a SLUS-21104 RAM image. And the quad's steps only run against SLUS-21104's own Desert Oasis world, with
some calls still answered by the interpreter. Those two are filed as MD122 and MD123. MD37d is blocked
until both land, and no code was changed.
