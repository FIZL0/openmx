# F44 — trail sender part virtuals

2026-11-05. Task: `./kit plan F44`.

The virtuals F39's sync, network and parts still called out are ported on state 4's objects: the
trail's on/off and the scene node's shown/hidden walk, the bike's send (its beat and which of three
packets), and the parts' step. A 10 000-case oracle against SLUS-21104 matches word for word, four
mutants fail it, and both C15 replays stay bit-exact with the ports in the tail (the trail's path is
never reached on them). The three packets are still the originals (F44a). Details:
1-trail-sender-part-virtuals-FINAL.md.
