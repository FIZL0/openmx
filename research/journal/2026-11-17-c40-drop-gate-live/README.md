# C40 — drop gate live

2026-11-17. Task: `./kit plan C40`.

The live proof of P313c's drop gate sink, on sx17 (an AI-ridden single race, PCSX2 copy 4). The sink
matched the port bit for bit, tick for tick: the height steps, the clock, the trigger counter, and the
two collision pieces going to the identity. The gate runs after the score manager's card work in a
frame, as the port has it. One bug came out of it: the original measures the trigger to the race
file's own line, before the supercross loader moves the line onto the finish gate. The port measured
to the moved line (2.2 ft off on sx17, a tick early). Fixed. Details in the numbered file (FINAL).
