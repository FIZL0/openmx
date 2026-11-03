# MD82c — flier follow camera

2026-11-03. Task: `./kit plan MD82c`.

The states and the camera captures behind MX vs. ATV's biplane and Kiowa, both models: MD80a's four
Desert Oasis Free Ride states, and MD82b's camera spec read every tick while MD80a's flight tapes play
(`research/md82c/fly.sh`). The fliers follow with the same MXCameraFollow object and update as the cars
and the ATV, from `FC_BiplaneDefault.pb` and `FC_HeliDefault.pb`. With no port change, MD82b's
`ModFollow` replays all four flights bit for bit (6603 rows), and MD82a's preset reader matches each
state's eight preset records. Details in `1-flier-follow-camera-FINAL.md`.
