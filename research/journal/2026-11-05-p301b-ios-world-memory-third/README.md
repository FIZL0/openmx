# P301b — ios world memory third

2026-11-05. Task: `./kit plan P301b`.

P301's third step: cut whatever still holds a world ride over ~2 GiB on an iPhone 15 Pro. The first
session had no phone log and stopped. On 2026-11-21 I drove the phone over omarchy-desktop's USB on
0.18.0, which has P301 and P301a. I rode world_mxu_free, world_ultra, world_mxvatv and world_mxu from
fresh launches and pulled the logs. The highest footprint was 1302 MB, on world_ultra, with GPU at
most 665 MB. Every world stays under 2 GiB, so nothing was cut. A frame-rate drop seen on world_ultra
after 7 minutes went on P346 as a note. Details in 2-….md (FINAL).
