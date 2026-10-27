# P148 — plane max g range

2026-10-24. Task: `./kit plan P148`.

The Biplane max G knob now goes up to 15 g (`MAX_G = (3, 15)`), still in 1 g steps. With the limit Off, the fastest plane (500% top speed and acceleration, 784 ft/s) pulls 47.9 g with the File scheme and 87 g with New, so 15 g still bites. P137's hold overshot above 13 g. To fix that, above 9 g the hold now aims 0.004 lower for each g, so 10–15 g stay under the limit. The original's ride (Off) is unchanged. The task was blocked first on main's red replay gate (S119), which is green now.
