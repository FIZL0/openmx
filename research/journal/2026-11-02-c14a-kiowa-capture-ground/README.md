# C14a — kiowa capture ground

2026-11-02. Task: `./kit plan C14a`.

C14's Kiowa capture (`context/capture/c14/flight.csv`) now replays bit-exact from 1205 to 2057, through
the take-off, the flight, the touchdown at 1816 and its bounces. It runs through `Kiowa::tick` with the
helicopter's own collision object (`kiowa::Collision`, the plane's class) searching free09 and
`contact::resolve` answering on it each substep (`kiowa::Grounded`). It is seeded from the save state
alone. Done; the rotor's own contact search is left to C17.
