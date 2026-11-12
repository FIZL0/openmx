# F56 — effects items turning

2026-11-11. Task: `./kit plan F56`.

The effects items' turning virtuals (on, off, key, and the pause and unpause a key reaches) for the seven
item classes C15 flip-1 reaches are now ported as `game_sim::handling::effects::{on, off, key, pause,
unpause}`. A 20,000-case oracle shows them bit-exact. The flip-1 and ride-2 every-port replays now run
them in Rust, with the RAM still matching. The objects' calls (the emitters') and the item steps are
still the interpreter's (F57–F59).
