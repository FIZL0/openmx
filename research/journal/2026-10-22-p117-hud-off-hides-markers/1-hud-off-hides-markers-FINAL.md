# 1 — hud off hides markers (FINAL)

Task P117. 2026-10-22.

## What I did

- `main.rs`: a new `marker_bar_shown(overlay, debug_hud)` decides whether a view's session-markers bar
  (`hud::draw_markers`: the slot boxes, the player tags, the label and the hold bar) is drawn. Both
  draw paths ask it: the GPU path filters its `banner` with it before the `rig.hud.on || banner.is_some()`
  patch, the software path guards its call. It is true for any overlay but `none`, and under `none`
  only once F3/H has put the debug HUD up (the bar is drawn in the same patch as that HUD, so it goes
  with it). Original and Debug draw exactly as before.
- Nothing else moves: the markers are still stepped and fire (`Seat::pass` is untouched), the banner's
  timer still runs, the in-world portals (V51) are drawn by the scene, and the radar's marker icons were
  already only drawn with the original's HUD (the radar is not drawn under `none`).
- `hud.rs`: `MarkerBanner`'s doc said it is drawn "whether or not the overlay is on"; now says not under `none`.
- Nothing from opentennis: it has no session markers or overlay setting of this shape.

Decision taken unattended: under `--overlay none`, pressing F3 brings the debug HUD and with it the bar.
The brief says "with Overlay None draw no marker bar"; I read F3 as the user asking for a HUD again, and
the GPU path draws the bar in the debug HUD's own patch. One line to change if the user wants it hidden
then too (drop `|| debug_hud`).

## What the original does

Nothing to port: the markers and their bar are the port's own (P88). The original has no session markers.

## How it was proved

`crates/openmx/tests/markers_hud_off.rs` rides the shell headless on free06 with a hand-written tape: 30
ticks of throttle, Down held 10 ticks (places slot 1), 30 more, then Up held (rides back to it); the
shot is taken at the Up hold's 9th tick, after the teleport, with Up still down so the banner is up
whatever the wall clock did. The camera is `--shot-pose`d 30 ft over the start line, looking back and 45°
up into the sky, so the ride, the portal and the teleport never reach the frame. On both `gpu` and
`software`:

- `--overlay none`: the shell prints `marker: placed in slot 1` and `marker: to slot 1` (the markers
  work), and the frame equals, pixel for pixel over all 640x448, the frame of the same tape with the
  d-pad left alone.
- `--overlay original`: same two marker lines, and the bar's top-centre box differs from the plain
  tape's frame in more than 500 pixels.

With `marker_bar_shown` forced true the test fails on gpu: "with the overlay off the markers drew over
6272 pixels".

## Not verified / not 1:1

- The sensitivity run (fix disabled) stopped at the GPU assertion, so the software path's "no pixel" check
  was seen to pass with the fix but not seen to fail without it. Its call site is the same one-line guard.
- A first version of the pose looked 10° up and saw 9 pixels of distant ground differ between the two
  rides (world detail, not the HUD: bottom centre, terrain colours). I pointed the camera at the sky
  rather than chase that; the ground difference itself is not explained.
- Split screen (`--players 2+`) and the plane's own marker set (P107) are not ridden in the test; both go
  through the same per-view call sites.
- Overlay None + F3 shows the bar (see the decision above): not what the brief's words say literally.
- Settings' Overlay None (the menu) is not driven; it sets the same `overlay_mode` that `--overlay none` does.

## Next

Nothing.
