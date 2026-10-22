# P106 — markers up down hold

2026-10-22. Task: `./kit plan P106`.

The user asked for Down (place) and Up (ride back) to take a hold again after P93 put every marker
direction on the press. They fire now on the tick the d-pad's own held timer crosses 200 ms
(`markers::HOLD_MS`, which was 250 in P88), once per hold. Left and Right still fire on the press. The
slot display's hold bar is back and fills for Up/Down only. Landed; details in 1-.
