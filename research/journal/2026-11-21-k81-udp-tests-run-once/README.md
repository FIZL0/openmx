# K81 — udp tests run once

2026-11-21. Task: `./kit plan K81`.

Builds the picked option A for online_put_back's load red. The 11 real-socket `net::udp` tests now run only in
the openmx bin, not in the ten test files and the web lib that mount `net`. That cuts ~12 copies racing on
loopback sockets and a 2 s clock per gate down to one. Details in the numbered file.
