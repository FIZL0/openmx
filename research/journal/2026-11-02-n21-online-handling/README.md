# N21 — online handling

2026-11-02. Task: `./kit plan N21`.

MD40's Handling setting reaches the lobby: the room carries it beside its trick pack (`Room::handling`,
the id of the pack whose `mx_*.phy` the bikes are read from, empty for the original's), the handshake
hashes that pack on every machine and refuses a seat whose files differ or are missing, and every seat of
a lobby ride — a drop-in's too — reads its bike from the room's pack before the tricks are laid. Done;
`1-online-handling-FINAL.md` has the detail.
