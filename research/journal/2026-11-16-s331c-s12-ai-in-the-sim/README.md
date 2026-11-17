# S331c — s12 ai in the sim

2026-11-16. Task: `./kit plan S331c`.

The Sim's AI riders now read the path nets as the original's race brain does. The trick plan's jump and
the habits' node come from the node the brain's own spot is on in the AI net. The avoidance's
keep-on-the-track edge and the habits' cross-up spot come from the card's spot on the race file.
All three reads are proved bit-exact on sx01's race image, over 10,000 cases each for the edge and the
spot, through the original's own path functions. Built on S331b's branch. Details in 1-…-FINAL.md.
