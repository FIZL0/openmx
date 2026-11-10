# S282c — s12 ai path data

2026-11-10. Task: `./kit plan S282c`.

The track's AI path is not the .wpt: it is `<track>AI.tdf`, a second race file read by the same reader
into act+0xbc, and the AI follower's net (ai+0xdc). The avoidance's edge asks the race file (act+0xb8)
instead. `ai::path::Net::from_track` turns either loaded file into the net the path queries walk, with
paths (first node, branches). It is proved against a new racing sx01 image node for node on both nets,
plus 10,000 runs of the original's locate on the real nets. Heights are as filed (the ground snap is S308).
Details in `1-s12-ai-path-data-FINAL.md`.
