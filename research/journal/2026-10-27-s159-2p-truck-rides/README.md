# S159 — 2p truck rides

2026-10-27. Task: `./kit plan S159`.

S159 was meant to replay, tick by tick, every stretch of the 2p recording where a rider is on a free09
trailer or truck. There are none. On all 12,917 rows of `context/capture/2p/2p.csv`, every wheel's
path-car and owner word is 0, and no rider comes within 183.9 ft of a path car. I checked this with C10's
owners.py and near.py, which agree with C10's own index. The task is blocked until the human makes the
second 2p recording (plan/HUMAN.md). Details in `1-2p-truck-rides-READY.md`.

The second attempt (`2-2p-truck-rides-FINAL.md`) used C12's truck recording (`split/truck.p2m2`), where P1
rides path car 3. The carried velocity on P1's wheels matches the original bit-exact in twelve RAM images
taken through the ride. The capture shows P1 staying on the deck throughout and rolling back under drag
with nothing held. The port also keeps P1 on the deck. Not 1:1: free09's presampled path cars (S181) and
how fast the port's bike rolls back (S182).
