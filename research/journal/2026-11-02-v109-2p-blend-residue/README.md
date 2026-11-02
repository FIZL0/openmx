# V109 — 2p blend residue

2026-11-02. Task: `./kit plan V109`.

C11 had the original's eased axes, reverse clock and ramp in V90's 2p replay, and P1's reverse arms
still posed only 3/17 (33) and 10/24 (34) within 1e-4. The layer that parts is the **snapshot** a fade
starts from. The original copies the joints' transform nodes, which the riding ragdoll has written over
after the blend; the port copies its own blend buffer. With the snapshot read off the original's nodes
(a new capture), every fading tick whose ramp the port has bit for bit poses within 1e-4. Test-only;
the port fix is filed. Details in `1-2p-blend-residue-FINAL.md`.
