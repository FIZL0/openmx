# S89 — dallas rail grip

2026-10-21. Task: `./kit plan S89`.

On Dallas (free06) the port's bike slid on the railway, and the original's does not. The railway is
62 raised ballast-bed props of their own (surface 1) over terrain of surface 0/12. The original's
wheels stand on them through the swept contact alone and read the terrain's surface byte. The port's
slide came from its own collision bundle holding the bike up by the belly, so no tire force reached
the ground. Entry 1 fixed that (`ride::belly_reach`, the port's own) and made the surface byte 1:1.
Entry 2 captured rides along and across the bed in the original from two save states and replayed
them. The port now rides the bed as long as the original does, on the same contact and surface, with
no slide, and the slip flags agree through the bed. Bit-exact replay waits on S107 (tick 1), and the
bundle's replacement on S112.
