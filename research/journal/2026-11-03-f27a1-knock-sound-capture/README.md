# F27a1 — the knock's collision sound, against PCSX2

2026-11-03. Task: `./kit plan F27a1`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

This is the brief's third check for F27a's collision sounds. A free09 beach-ball knock was captured in
PCSX2 with Video Capture on, and the port's playback of the same ride was recorded. Each was then
matched-filtered for `Impact\ball.vag`, the bike-on-ball hit (material 21). In the original, the bike
strikes ball 2 at 18.98 ft/s, so the hit is level 75, the same as knock-2882. The clip is found once
(NCC 0.55, runner-up 0.35) and it is one voice: its first 33 ms match at NCC 0.97, and no second copy
is within ±3 ticks. The port finds the clip at NCC 0.92–0.94, but as two or three equal voices, one a
tick. Reads of the original's queue on the capture's RAM images show the strike queued on two
consecutive ticks (2903 and 2904), as knock-2881/2882 already showed. So the original's sound layer
plays one voice where the port's `HitTable::sound` stacks the repeats. That is filed as a bug. Which
rule suppresses the repeat is not found. Done; details in `1-knock-sound-FINAL.md`.
