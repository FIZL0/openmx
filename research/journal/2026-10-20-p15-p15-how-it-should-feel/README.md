# P15 — p15 how it should feel

2026-10-20. Task: `./kit plan P15`.

P15 put the original's ride numbers next to the port's for Race125 over Free09. The original's side comes
off both human recordings; recording2 was played back over AT-SPI and captured for the first time. The
port's side comes off the shell on the disc's own `race125p.phy`, through new `--trace-feel`, `--trace-at`,
`--trace-stick`, `--trace-switch` and `--phy` flags.

Landings match, at about 6–8 ft/s lost off a 52 ft/s lip, and turn rates overlap. The rest does not:

- The port accelerates 2–3× harder and tops out at 121.8 ft/s, against the original's 80, a lower bound
  because it was still accelerating.
- In the air it falls 1.5× faster per velocity unit, and it leaves lips flatter.
- It bleeds speed in held turns.
- It brakes at 54 ft/s² against the one 25 ft/s² flat bite on record.
- The shell's default `.phy` is pro125a, not race125p.

The ride legs in both ride.rs files were raised to what the hand-written fixture carries, and each gap is
a task, P20–P26. Details in 1-p15-how-it-should-feel-FINAL.md.
