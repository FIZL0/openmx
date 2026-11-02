# P258 — garage pickers

2026-11-02. Task: `./kit plan P258`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

The Garage's Bike, Paint and Class rows lagged, stepped twice when slow and seemed to change the bike on
a Paint step. Dressing now runs on a worker thread (inline on web, after a 150 ms settle) with the
neighbouring picks prefetched and six kept; the stage keeps drawing the last finished pick until the asked
one is ready, and a finished dress for a pick no longer asked is kept, never shown. Mips are made on the
worker, so the upload on the show frame is 2–9 ms instead of 16–21. Key and pad events that are only
stamped on receipt get spread back over a late poll, so a push and release inside one long read is one
press. The "paint changes the bike" bug was the Bike row's text: it came from each paint's team logo
(KTM 125 SX → One Industries 125 SX); it now reads the model's own name and the Paint row names the
livery. FINAL; details in `1-garage-pickers-FINAL.md`.
