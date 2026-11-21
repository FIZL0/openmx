# S501 — switched car safe spot

2026-11-21. Task: `./kit plan S501`.

The original records a safe spot for a car the human drives in free ride, twice a frame: level, clear
ground under the root node goes into the car's store, which a pause-menu Restart puts the car back from.
The port's recorder (`car::Store::record`) matches the original bit for bit on all 16 calls of 8 whole
frames off s342. The shell records from the driven car into its parked copy (not in a race) and carries
the stores across a Restart. Switching out drops the card, so the store keeps the last spot (read from
the code and one image). Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
