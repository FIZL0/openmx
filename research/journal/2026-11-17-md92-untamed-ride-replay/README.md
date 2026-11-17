# MD92 — untamed ride replay

2026-11-17. Task: `./kit plan MD92`.

MD92 captured an MX vs. ATV Untamed MX ride with a wreck (2026-11-03) and replays it here. The
interpreter runs Untamed's own sim frame off state 8, and both clean captures come out bit-exact on
all 665 rows, through the wreck and its slow motion (tests/untamed_bike_ride.rs). The pad reader, the
put-back spot, the visit mark and the slow motion's wall-clock ms are supplied from the rows. The app
ride on untamed_tricks isn't done: it waits on F31f, and is filed as MD92b. Hooking the Untamed ports
into this replay is MD92a. Details: 1-untamed-ride-replay-FINAL.md.
