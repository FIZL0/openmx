# F44a — net packets

2026-11-05. Task: `./kit plan F44a`.

This task ports the bike's three net packets: the stunt, the whole state and the change. It also ports the quantiser table they keep
(lookup, keep, quantise and state 4's decimal hash) and the sender's own send. A 10 000-case oracle against SLUS-21104 matches word for
word on all 26 paths, and four mutants fail it. The original stunt packet no longer needs a stub in mxvatv_bike_ride.rs: its RCNT0 read
is redirected to a scratch word. Both replays stay bit-exact with the ports. The connection's queue is still the original (F49).
Details: 1-net-packets-FINAL.md.
