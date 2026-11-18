# K79 — one fps mark reads its own byte

2026-11-18. Task: `./kit plan K79`.

tests/one_fps.rs said "1 bytes read" after the first frame in slot 5 every run. The shell's read
counter guessed the size of its own look at `/proc/self/io`, and the file grows a byte when a counter
gains a digit. The mark now counts its own look exactly. Details in file 1 (FINAL).
