# P72 — reverse l3

2026-10-25. Task: `./kit plan P72`.

Per the user's 2026-10-22 note, reverse is the original's own rule under both layouts, not a button. Gas and
brake held together below 5 ft/s back the bike up: L2+R2 under New, Square+Cross under Original. At speed
the two together are a brake. The rider backing up now plays the reverse pair. It never did before because
the riding request read a reverse command that no pad set. The rule matches the original's captured `+0x648`
bit for bit on 682 S68 rows. A new capture shows the original running state 34 with the same clock while
backing up. Details in `1-reverse-l3-PART.md`.
