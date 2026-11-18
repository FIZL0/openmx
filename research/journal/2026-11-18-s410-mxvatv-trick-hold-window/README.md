# S410 — mxvatv trick hold window

2026-11-18. Task: `./kit plan S410`.

On a trick-pack ride, a trick's node now grows only in MX vs. ATV's hold window: its buttons held and
its clock held at HoldTime (window min(HoldTime, 0.3) to HoldTime + 0.04). Before, it grew whenever
the port's clock counted the trick as held. The rule matches the port of the original's trick step on
20 000 generated cases. A ride test shows a held Varial growing only from its hold. The clock itself
is still MX Unleashed's. Details in 1-mxvatv-trick-hold-window-FINAL.md.
