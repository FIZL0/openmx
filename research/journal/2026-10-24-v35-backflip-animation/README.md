# V35 — backflip animation

2026-10-24. Task: `./kit plan V35`.

The rider never played its backflip clip, because the port raised the body's backflip request off a
trick id (24) that nothing latches. A capture of recording2 shows the original's request word is the
bike's turning flag on all 1100 ticks. The port now reads that flag, and the body plays `backflip` on
the original's ticks: all 976 riding ticks match, with the backflip on 2513..2543. A ridden flip in the
shell's sim plays it on exactly the frames the bike turns over. Details in `1-backflip-animation-*.md`.
