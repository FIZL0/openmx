# S161 — backwards landings bail

2026-10-28. Task: `./kit plan S161`.

The original rides out a landing crossed up past about 135 degrees (S150: its crossup is an arcsine),
and the task asked for an option, off by default, for such a landing to bail. Run unattended, so nobody
could say whether the user wants it; it was added, since off it changes nothing and the user's S150
report ("instead of bailing it flings you around") reads as wanting it. The switch is Backwards Landings
Bail on the Game Modifiers page beside Air Wrecks, carried everywhere Air Wrecks is (settings.txt, the
sim's Sense and snapshot, tapes, `--backwards-bail`, the room and the handshake). With it on, S150's two
captured ride-outs (148 and 169 degrees) wreck on their touchdown tick, and the 118-degree one still
wrecks as the original does.
