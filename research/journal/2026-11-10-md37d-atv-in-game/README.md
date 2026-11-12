# MD37d — atv in game

2026-11-10. Task: `./kit plan MD37d`.

MD37d puts MX vs. ATV's quad in the game: in the garage, the machine pick and the pause Switch To
Machine, ridden with the pack's clips, cameras, sound and particles. The first session found that two
things it needs do not exist yet. Nothing builds a quad from its `.phy`: every quad so far is seeded from
a SLUS-21104 RAM image. And the quad's steps only run against SLUS-21104's own Desert Oasis world, with
some calls still answered by the interpreter. Those two are filed as MD122 and MD123. MD37d is blocked
until both land, and no code was changed.

**2026-11-12, part 2 (FINAL):** MD122 and MD123 have landed, and the quad is now in the shell as an
offline addition, off by default:
- The Garage's Quad row (or `--atv NAME`) picks one of the pack's eight `.phy` files.
- The pause's Switch To Machine takes the quad, which rides on MX vs. ATV's step with the chase
  camera following it.
- With nothing picked the ride is unchanged.

It is not drawn yet, and has no FC_ATV cameras, sound, particles or online play. Those are filed as
MD135, MD136 and MD137. See `2-atv-in-game-FINAL.md`.
