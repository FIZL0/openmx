# S14 — s14 freeride opponent

2026-10-15. Task: `./kit plan S14`.

Where the tick-exact acceptance window has to end, and why. Free09 puts three AI riders in the level
whether anyone asked for them or not — a one-player freeride is forced to four racers — and with the
recording's save state loaded and nothing pressed, the shared LCG still advances two to nineteen draws
a tick. So a port without opponents cannot match the RNG past tick 0, and the window has to close at
the first wreck or contact, where the player's own path starts drawing from it. `OPPONENT_ENABLED` and
`OPPONENT_DIFFICULTY` are front-end identifiers rather than settings keys; the simulation-side truth is
four racers and an AI difficulty of 90, the event constructor's default. A crash does not swap the
bike's parameter set, but it does hand the body to the ragdoll, so S10's landing test must stop at the
wreck. Two corrections fell out: the AI-interface bit 0x40 is set on Free10/Free06 and cleared
elsewhere (`research/oracle.txt` had it inverted), and `watch/acceptance.toml`'s pointer chain reaches
an AI rider rather than the human — a replacement chain, live-verified, is in the entry. The one thing
not measured is the contact frame itself: PCSX2 plays an input recording from its GUI only.
