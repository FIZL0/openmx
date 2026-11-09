# C31 — sx01 laps capture

2026-11-09. Task: `./kit plan C31`.

Two live sx01 single races captured in PCSX2, 12,000 ticks each, with every score card's lap-line and
race-handler words (plus raw bytes) on every tick. The laps come from three AI riders: the boot asks for
4 racers instead of 1, and each AI rider rides 3 laps and finishes. A generated gas-held tape sends the
player off the track 37 times, which covers the off-track clock. Two findings: the score manager's
+0x60 is 1 in a single race, where the lap line does finish cards (S196's +0x60 = 2 image was a
practice); and the card list is re-sorted by place, so cards are read at fixed addresses. Nothing in the
port is held against the captures yet (S13c/S293/S294).
