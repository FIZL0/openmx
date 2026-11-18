# F50 — stunt packet runs

2026-11-17. Task: `./kit plan F50`.

F44a left two follow-ups: port the connection's queue, and stop stubbing the stunt packet in the other test
files. The queue was already ported and proved by F31b5k. This task removes the stunt packet's do-nothing hook
from the six other test files and redirects its RCNT0 read to a stand-in word, as mxvatv_bike_ride.rs does, so
the original stunt packet now runs in every replay. All their tests still pass. Details: 1-stunt-packet-runs-FINAL.md.
