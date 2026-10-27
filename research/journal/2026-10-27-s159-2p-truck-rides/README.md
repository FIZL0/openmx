# S159 — 2p truck rides

2026-10-27. Task: `./kit plan S159`.

S159 was meant to replay, tick by tick, every stretch of the 2p recording where a rider is on a free09
trailer or truck. There are none. On all 12,917 rows of `context/capture/2p/2p.csv`, every wheel's
path-car and owner word is 0, and no rider comes within 183.9 ft of a path car. I checked this with C10's
owners.py and near.py, which agree with C10's own index. The task is blocked until the human makes the
second 2p recording (plan/HUMAN.md). Details in `1-2p-truck-rides-READY.md`.
