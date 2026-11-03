# P282 — baked track pictures

2026-11-03. Task: `./kit plan P282`.

The Deck went down picking world_ultra: the menu's still job shot a second whole world (`--world-ground
all`, 8.4 GB on the desktop) beside the ride's load, and earlyoom took both. Now a card's picture is
taken once and kept: a mod.json newer than its still, or a still of an older framing, no longer shoots
one again; a world is never shot on the player's machine (no picture is a card with none — F28 files a
cheap top-down map for that); no still is shot while a ride loads or runs (a guard around both
`ride(...)` calls in main.rs, and one being shot when a ride starts is killed and shot again after it);
one still at a time. The store's upload bakes `previews/track.png` into a track or world that has none
(the kept still, else one shot on the uploader's machine), so a download arrives with its picture.
openmx-mods' importers/stitch half was already filed there as T14 (d7a1ad9).
