# S12c — s12 ai avoidance

2026-11-06. Task: `./kit plan S12c`.

The motocross AI's avoidance pass (`ai::avoid`: the rider loop's near/far swerve and brake, the feeler
fan, the look gate on flags 0x80/0x40/0xc0, the off-the-track push), ported bit-exact and oracled over
10,000 passes of the original in Ee (file 1, 2026-11-06). Its first merge hit conflicts with S12b/S12d in
ai.rs; file 2 (2026-11-09) is the merge, with the oracle run again on the merged code.
