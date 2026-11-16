# V119 — scare delay

2026-11-16. Task: `./kit plan V119`.

Every ScareBirds on the disc (21, in nat01, nat02 and nat19) has ScareDelay 0, so the scare's delayed
path is never taken and was not ported; a test now guards that. Details in the numbered file.
