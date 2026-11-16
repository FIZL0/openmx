# MD172 — quad on the wire

2026-11-16. Task: `./kit plan MD172`.

A seat's quad out online (MD152) now goes in the ride's state bytes, so a resync mends a desync, a
drop-in join takes the joiner at once, and a restart after a leave carries on, all with quads out.
Proved on the in-memory Netplay harness of `crates/openmx/tests/netplay.rs`, on the pack's quad over a
flat floor. Details in 1-quad-on-the-wire-FINAL.md.
