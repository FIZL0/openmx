# P275 — biplane climb camera

2026-11-03. Task: `./kit plan P275`.

The plane's follow camera lost the plane off the top of the picture in a steep climb or a loop. The
camera is the original's race camera, ported to the bit (V29), and the original loses it too: in V29's
capture of the original's own camera the plane is up to 46 degrees above the middle of an 85-degree
view, out of the picture on 36 of 1718 rows. The shell now turns the look (not the eye, not the
camera's state) the least it takes to keep the plane inside 0.7 of the half picture, on every preset,
1p and split. Wherever the original already frames it, the view is left alone to the bit. Done.
