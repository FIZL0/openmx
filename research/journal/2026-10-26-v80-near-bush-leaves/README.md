# V80 — near bush leaves

2026-10-26. Task: `./kit plan V80`.

V56's cut-out bushes lost their leaves up close on the GPU. The cause was the GPU's bilinear alpha
being cut at the original's high test (242 of 255 on free06): it shaved magnified one-texel leaves to
slivers, and the kept-coverage levels kept that thinness far off as well. The fix makes a cut-out
texture's texels all or nothing by that test and cuts the filtered read at half. On free06's and
free09's bushes the GPU now draws what the software renderer does, within a few percent, at 3 ft and
at 100 ft. Before, free06's bushes on the GPU drew about a third of that up close and an eighth far
off. Details in 1-near-bush-leaves-FINAL.md.
