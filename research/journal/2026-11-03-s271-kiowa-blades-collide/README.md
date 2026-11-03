# S271 — the Kiowa's thrown blades collide

2026-11-03. Task: `./kit plan S271`.

The four blades the MX helicopter throws now run the world node's collide pass, and the run is
checked against the original: thrown low over free09's terrain, they land, bounce, slide and go to
sleep where they lie, bit for bit. Each blade's collidable mesh (`helicopter\col\bladebentf.col`) is
now loaded, and posed where its node stands it for others' searches.

Code: `game_sim::kiowa::{Blades::shape, Blades::knocks}`; `Blades::new` takes the shape's bytes.
Test: `tests/kiowa.rs` `thrown_blades_land_and_sleep_as_the_originals`.
Entry: [1-kiowa-blades-collide-FINAL.md](1-kiowa-blades-collide-FINAL.md).
