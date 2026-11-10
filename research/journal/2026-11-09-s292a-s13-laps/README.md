# S292a — s13 laps

2026-11-09. Task: `./kit plan S292a`.

The rest of the score manager's mode-1 (machine race) handler around S292's gate line and zones: odometer,
gate distances and the 50 ft aim past the gate, progress, clocks, the over-race finish estimate, the time
limit, and the close with its own gate ranking. Ported as `LapRules::machine_tick` / `machine_card` /
`machine_close` / `gate_rank` and proved bit for bit against the handler and the ranking over 20,000
generated cases each. The 0x80000 reset helper and the path-steer branch are left to S292c.
