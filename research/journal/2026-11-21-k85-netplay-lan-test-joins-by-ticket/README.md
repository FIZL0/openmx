# K85 — netplay lan test joins by ticket

2026-11-21. Task: `./kit plan K85`.

The netplay test's LAN drop-in ride now joins by the host's ticket, not its room code, so it no longer looks
the code up on the machine's shared mDNS group (option A, picked by the person). 16 copies at once all pass.
