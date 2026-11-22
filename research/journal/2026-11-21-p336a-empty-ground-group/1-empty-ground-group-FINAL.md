# 1 — empty ground group (FINAL)

Task P336a. 2026-11-21. Follows `research/journal/2026-11-15-p336a-webkit-stage-log/` (blocked then on a log).

## What I did

- Drove the iPhone 15 Pro on omarchy-desktop myself (iFizBox `tools/ui-15.sh`, lock `P336a`, waited about
  15 minutes for P301b's lock, given back after, phone put back to portrait). Recipe as P257a's.
- Safari, `https://beta.openmx.dev/?fps`, build 998d107 (has P336), WebGPU. The disc was still in the page's
  storage from P257a. Clicked Play through `ios webinspector eval` (`document.getElementById("ride").click()`;
  eval needs `--timeout=40`, the default timed out once), then the touch HUD: X, Race, X, Los Angeles (sx01),
  X, Start Race.
- The page went back to its launcher by itself a moment after the race loaded. The log, read off the phone
  with `ios webinspector eval 2 "openmxLog.lines.join(String.fromCharCode(10))"`, is
  `research/p336a/openmx-log-iphone-2026-11-21.txt`:
  `gpu: validation error in the views, frame 5959: number of bind groups set(2) is less than the pipeline uses(3)`,
  then `webgpu failed in a ride … starting again on webgl2` and the reload on `?webgl2`.
- "the views" is the stage that draws the views into the low texture (render scale under 1). The only
  pipelines there with 3 or more groups are the ground's: its layout was
  `[view, ground, meshes.map(layout)]`, so a ground with no roads and ruts (sx01: no disc map meshes) had a
  missing group 2, and the draw set only groups 0 and 1. Chrome's WebGPU (and wgpu native) treat a missing
  group as needing nothing; Safari's counts it and wants a bind group set there. The stand-in's pipeline
  (`[view, ground, meshes?, cover]`) had the same hole.
- Fix (`crates/game-render/src/gpu.rs`): a ground without meshes keeps `no_meshes`, an empty bind group layout
  and an empty bind group. Both ground pipeline layouts take it as group 2 (`GpuGround::group2`), and
  `draw_views` sets the empty bind group at 2 when there are no meshes. No other pipeline layout in
  game-render has a missing group (checked every `bind_group_layouts`), and every draw with a ground
  pipeline (whole, fading, stand-in) is in that one loop of `draw_views`, after group 2 is set.
- Test `a_ground_without_meshes_sets_an_empty_group_2`: a meshless ground holds the empty group, and a frame
  drawn with it passes wgpu's validation with the ground drawn (it ran, not skipped).

## What the original does

Not applicable: rendering on the web.

## How it was proved

The failing call is named by the phone's own log (committed above). The fix is proved only on this machine:
the new test, under wgpu native. Not yet on the phone (below). The test cannot see Safari's rule: wgpu
native accepts a missing group, so before the fix only its `no_meshes` check would have failed; its frame
proves only that the empty group breaks nothing.

## Not verified / not 1:1

- The fix is not ridden on the iPhone: beta serves 998d107 until this branch merges and deploys, and there is
  no other HTTPS preview the phone can open. Filed a check for after the deploy (see Next).
- Whether another WebKit-only error waits behind this one in a later stage or a later frame: the page restarts
  at the first error, so nothing after frame 5959 of that ride was seen.
- The stand-in pipeline's hole (group 2 with a cover at 3) was fixed the same way but never seen failing: no
  ride tried here uses a stand-in.
- Why the user's 11-12 log failed at frame 414 in the menus ("encoder state is invalid", pre-P336): not
  reproduced; today's menus drew with no error on the same phone (P257a and this ride).

## Next

P396 (filed, low, after P336a): after the deploy, ride sx01 on beta in Safari (WebGPU, `?fps`) by the same recipe and read
the log for any `gpu: validation error` line (the filed follow-up).
