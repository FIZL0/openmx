# F20 — node listeners

2026-11-03. Task: `./kit plan F20`.

The knockable props' world node now keeps one listener list for every collection (`PropSet::listeners`),
walked once after every collection has stepped, a finished listener's slot taking the list's last, as the
original's node does. s195 knock's ball 2 now puts its collection to sleep on 3275, a tick after its body,
as the game does, and agrees bit for bit to the capture's end (the countdown and the put-back at 5075
included). Details in `1-node-listeners-FINAL.md`.
