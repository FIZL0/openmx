# S260 — the seats' wreck bikes on the props' node

2026-11-03. Task: `./kit plan S260`.

Seats that share one prop set (online, split screen: `prop_seats::Shared`) now put every downed
seat's wreck bike on the props' node, as one Sim's riders are since S243: the hull in every prop's
kept world, one ray record over the scene and the bike. S197b's after-pass search (`wreck_strike_as`)
is no longer run for them. Details in `1-shared-wreck-on-node-FINAL.md`.
