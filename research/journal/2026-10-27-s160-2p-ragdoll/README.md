# S160 — 2p ragdoll

2026-10-27. Task: `./kit plan S160`.

The ragdoll through each of C10's 17 wrecks of the 2p recording, both riders: from the RAM image
before each, the original's player and body updates run to ten rows past the put-back, and the
port's actor tick runs beside them on its own carried state. The port is the original's on every
tick of all 17 runs (2,824 ticks). 15 wrecks are entered, put back and ridden on again; P2's two
Scenery wrecks are never entered (S176). The one gap was the test's, not the port's: the body's
update runs the vehicle animation, whose posing flag and clip the carried port now takes from the
original. The bones are the capture's only up to the wreck, on five of P1's runs (S176). Details in
1-2p-ragdoll-FINAL.md.
