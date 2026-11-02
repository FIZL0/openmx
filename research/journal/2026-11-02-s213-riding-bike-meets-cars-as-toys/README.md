# S213 — riding bike meets cars as toys

2026-11-02. Task: `./kit plan S213`.

The brief (filed from S171 on 2026-10-28) asked to move the riding bike's wheel search, body rays and
sweep-back, and prop_world's scene, from the scenery world to `Scene::flown` if the original's probes
search toys. That move had already landed: S189 (2026-11-01) put the bike's one kept nearby list on the
flown world (`refresh_ridden`, `nearby_scene` → `Scene::flown`), and S200 put the props' probes on
`Scene::probed` (flown). What was left was the check against the original, and the brief's test. Both are
in `crates/game-sim/tests/traffic.rs` (S213 section); no product code changed. Details in the numbered file.
