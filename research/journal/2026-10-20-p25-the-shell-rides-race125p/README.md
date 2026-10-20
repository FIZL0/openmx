# P25 — the shell rides race125p

2026-10-20. Task: `./kit plan P25`.

When the shell is given no `--phy`, it now rides `Bike\Race125P.phy`, the file the original's loader asks
for on a freeride boot (D7), built by the port's own `track::physics_file`. It used to ride `pro125a.phy`,
the archive's first `.phy`. `--phy` still overrides it. On a bike archive without that file, the shell
falls back as before and prints a line saying so. The trace's `phy:` line shows `bike\race125p.phy`. Main's
`the_humans_tricks` (S30) fails in any slot that holds C6's recording2 capture. That is unrelated, and
written down in the entry. Details in 1-…-FINAL.md.
