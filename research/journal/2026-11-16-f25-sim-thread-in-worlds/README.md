# F25 — sim thread in worlds

2026-11-16. Task: `./kit plan F25`.

The sim thread (N23a) never took a tick in a world of tiles, because the picture drawn meanwhile reads
the world's tiles (ground heights, ownership, wraps, the software ground) while the tick owns them. Now
the picture reads its own copy of the world (`Tiled::picture_copy`): the same tiles, with the loaded
terrains' bytes shared, not copied. The copy is brought up to date each time a tick goes off
(`Tiled::copy_into`). Over 1800 ticks of a two-player ride on world_mxu_free, with seat 0 on the gas for
3,600 ft, a wreck and tiles loading on the way, 1798 ticks ran on the thread. Every seat's hash after
every tick matches the ride with the thread off. A new gate test does the same on the md8 test world.
