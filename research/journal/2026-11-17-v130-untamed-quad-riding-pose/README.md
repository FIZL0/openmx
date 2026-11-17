# V130 — untamed quad riding pose

2026-11-17. Task: `./kit plan V130`.

The task asked to shift game-render's riding-pose compare (46) to Untamed's 54 on an Untamed quad. It
needed no shift. The quad's clip driver already turns Untamed's stunt ids into the table's rows (MD144b), and
its stunt word, the riding pose included, stays in those rows. So 46 is the right compare on either step;
54 would have broken the air pose. I left the code as it was, wrote the reason down at the id mapping, and
added two tests that prove an Untamed quad in the air blends the riding pose. Details in `1-…-FINAL.md`.
