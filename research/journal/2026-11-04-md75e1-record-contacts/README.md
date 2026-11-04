# MD75e1 — record contacts

2026-11-04. Task: `./kit plan MD75e1`.

The quad's collision record's resolution of its contacts (vt `+0x88`, the vehicle gravity's call) is
ported as `game_sim::atv_contact`. It is MX Unleashed's contact resolution, with a mask on the own body's
impulse turn and a relative velocity kept per entry. It is proved bit-exact over 2 × 10,000 oracle cases.
`quad::Steps` runs it when the bike holds the record (`Bike::contacts`). A whole frame off `atv-r-spawn`,
with two contacts laid into the record, equals the original's. A captured image with real contacts, and
the record's collide, are MD98. Details: `1-record-contacts-FINAL.md`.
