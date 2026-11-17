# F29 — world_ultra's CPU peak

2026-11-16. Task: `./kit plan F29`.

world_ultra's VmHWM fell from 1855–1929 MB to 1548–1735 MB on the 32-core desktop, and from 1417 to
1094 MB on 8 cores. The far maps' skies are read near the eyes on a thread (`SkyStream`, 68 of 77
left for later), the look reads 8 maps at a time, and the CPU's pages and detail maps go before the
ground is built. The peak turned out to be in the ground's first build, not the look: filed as F29a.
Pictures at a far shot pose match HEAD to the pixel.

Code: `crates/openmx/src/worlds.rs` (`SkyStream`, `LOOK_READERS`, `WorldLook::sky_later`),
`crates/game-render/src/sky.rs` (`Sky::held_in`), `crates/openmx/src/main.rs`.
Tests: `world_look.rs` `a_far_maps_sky_is_read_when_a_view_jumps_over_it`, `world_ultra_load.rs`.

Entry: [1-world-ultra-cpu-peak-FINAL.md](1-world-ultra-cpu-peak-FINAL.md).
