# F27 — the material table's collision sounds

2026-11-03. Task: `./kit plan F27`.

A track's `Col\template.cmt` gives every material a `CollisionSoundFile`. The original plays it when
a collision object's ray probe strikes something of that material. F27 ports the rule that picks
which of a frame's hits play and how loud, plus the table the audio reads. Feeding the rule hits from
the ride, and the PCSX2 capture comparison, are F27a.

- `1-material-hit-sounds-FINAL.md`: who carries the listener, the rule, the tests.
- `2-feeding-the-queue-F27a.md`: the listener on the bikes', props' and plane's probes, the shell's
  queue and play, the knock-2882 queue compared (F27a).
