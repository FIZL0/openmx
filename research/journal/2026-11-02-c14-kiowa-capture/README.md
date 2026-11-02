# C14 — a captured Kiowa take-off, hover and landing

2026-11-02. Task: `./kit plan C14`.

The capture half is done: the original flying the Kiowa on free09 from a generated tape (climb, hover,
forward, rudder, bank, descend, land), 854 ticks a row each, taken twice. The replay half waits on the
port: the measure listener (S220a) and the input step and tick (S220b) are not in yet, so there is no
`Kiowa` step to replay the rows through. Blocked `--after S220a,S220b`. Details in
`1-kiowa-capture-PART.md`.
