# S282 — s12 ai route follow

2026-11-10. Task: `./kit plan S282`.

S282 asked for the whole AI route follower, which is too big for one task. It was split. This task ported
the track path's three queries the AI uses (locate, segment-holds and node-direction) as `ai::path`, plus
the avoidance's keep-on-the-track test built from them (`path::off_track`). Both are oracled over 10,000
generated paths and 10,000 whole avoidance passes. The free-ride target seeker (S282a), the race path
follower (S282b) and the path data off the disc (S282c) are filed. One finding: recording2's free09 AI
riders have no path at all and ride targets. Details in `1-s12-ai-route-follow-FINAL.md`.
