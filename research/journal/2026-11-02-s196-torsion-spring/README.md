# S196 — the props' torsion spring

2026-11-02. Task: `./kit plan S196`.

Every supercross track's banners (`banner.pco`) hang on a pin with a `BodyTorsionSpring` that pulls
them back upright; the port read the constraint and dropped it. It is now `prop_world::Spring`, made
by the reader off the body's inertia and run first on the collection world's force list, proved bit
for bit on sx01's banners against the original in the interpreter. On the way, the port's collections
had never read `[PhysicsParts] ResetTime` (free09 has none): free11's bowling pins and spare tyres
now count their five seconds down as the original's do, proved the same way. No `.pco` on the disc
has a `PinToBody`. Done; details in `1-torsion-spring-FINAL.md`.
