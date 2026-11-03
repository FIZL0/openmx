# S274 — wreck bike meets the plane

2026-11-03. Task: `./kit plan S274`.

C16's joust aftermath: after P2's wreck 7 the plane parts a tick later, because the original's wrecked
bike goes on meeting the plane's hull. I found what the wreck bike files on the plane's list: its rays
strike the hull, and each hit's kind-1 mirror lands on the plane's list. I ported that
(`game_sim::wreck_plane`, `flight::Met::wrecks`, `flight::WRECK_FROM`) and seeded a wrecked P2 off
137910's image (`tests/c16_fly.rs` `wrecked_p2`). The plane now moves the right way after the wreck, but
not bit for bit, so no pin rose. Blocked: see 1-…-PART.md, "Next".
