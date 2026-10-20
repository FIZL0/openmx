# N3 — n3 lobby and transport

2026-10-20. Task: `./kit plan N3`.

Online play's finding and joining. Entry 1 brought opentennis's transport over as `crates/openmx-net`:
- iroh with hole punching, the host relaying, and a LAN-only mode over mDNS;
- room codes through pkarr, and a DHT lobby board for quickplay;
- chat, and a handshake that agrees the seed and refuses a peer whose track, bike sets, wrecks or biplane differ.

Its tests cover joins over the LAN and over a local relay, and a refused bike set. Entry 2 added the online screens to P36's front end:
- Online (quickplay, host, find, join by code, LAN only), Find Games, and a Lobby with the seats' riders and bikes,
  ready, chat and the room code;
- a `Disc` hashing the `.phy` and track archives the ride loads;
- N2's rollback ride over the lobby's connection, back to the lobby when a peer drops.

These are proved by menus on two real LAN networks and a ride of 230 settled ticks in step. Not proved: two shells
on screen, and the real internet (see 2's list).
