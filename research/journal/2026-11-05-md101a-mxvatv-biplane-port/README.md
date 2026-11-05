# MD101a — mxvatv biplane port

2026-11-05. Task: `./kit plan MD101a`.

With `mxvatv` set, `Flight` now steps the plane's body with SLUS-21104's later body (MD75's `atv_body`
integrate and end of substep) and no longer uses MX Unleashed's `biplane::Body` step. The substep count is the
same as before. When the port's two body calls are hooked into the original's frame, every row of MD101's
biplane and biplane3 flights stays bit-exact. A whole port `Flight`, seeded off a capture's image every 100
frames, flies every airborne stretch bit-exact. It parts only at ground contact (tires and ground are MD101b).
`AGREES_THROUGH` ratchets this. Details in 1-mxvatv-biplane-port-FINAL.md.
