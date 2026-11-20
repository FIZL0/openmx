# R51 — Untamed's follow camera backed into a wall

Journal: research/journal/2026-11-20-r51-untamed-camera-wall. Test: game-sim tests/untamed_camera.rs
`r51_the_eye_backed_into_rocksburys_wall_is_the_originals`. Setup as MD147 (research/md147/README.md):
`KIT_CONFIG` a SLUS-21701 kit.toml, PCSX2 started with the AT-SPI variables; set the copy's
`[Framerate] NominalScalar` to 0.3 first or the recorder misses rows.

- back.pad: from md147/spawn, Cross + Square held (the quad's reverse) 650 ticks, straight back into
  Rocksbury's south wall, then let go.
- camera.toml: the recorder spec, every tick: the target block, the filter (672 bytes), the camera
  (192 bytes), the quad's place and speed.
- play.sh: MD147's play.sh with camera.toml. Made back.csv:
  `KIT_CONFIG=… KIT_PCSX2=3 ./kit pcsx2 with -- bash research/r51/play.sh PID research/r51/back.pad md147/spawn 730 back`
  → context/capture/r51/back.csv (720 rows) and context/recordings/r51/back.p2m2.
- cam.py: print a capture's eye goal, eye, last clear goal, contact ring and boost; `*` marks rows where
  something moved. dump.py: quadwords of an Untamed RAM image. track.py: an MD147 capture's path.
