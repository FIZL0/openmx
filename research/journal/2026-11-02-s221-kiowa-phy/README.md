# S221 — kiowa phy

2026-11-02. Task: `./kit plan S221`.

S221 ports the Kiowa's `.phy` reader and its wreck pieces as `game_sim::kiowa::Phy`. The reader covers
both rotors, the aerodynamics, the flight controller and `[Wrecks]`. The wreck pieces are the rotor's
eight wreck rays, `wreckblade.phy`'s blade part, and the pelvis box. It is proved word for word (213
words plus the rays) against S148's live helicopter. The wing setter's chord rule gets a further
10,000-case oracle. Details are in 1-kiowa-phy-FINAL.md.
