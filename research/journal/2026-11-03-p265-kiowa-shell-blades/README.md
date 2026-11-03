# P265 — kiowa shell blades

2026-11-03. Task: `./kit plan P265`.

The shell builds the Kiowa's four thrown blades (S220a1a's `kiowa::Blades`) off `wreckblade.phy`,
`bladebent.psg` and `bladebentm.col`, with the terrain's edge. It searches and steps them beside the
heli tick and posts them after the crash test's act. It draws `bladebent` on each shown blade's node on
both renderers. A fast landing on free09 throws all four. They fly off the rotor head and come down on
the ground until the put-back hides them. That was looked at beside PCSX2's C17 wreck (frame 1302) and
tested in `kiowa_switch.rs`. Details in the numbered file.
