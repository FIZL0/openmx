# 1 — ios edge to edge (FINAL)

Task P240. 2026-10-30.

## What I did

The user's ask: at Auto, keep only the island clear, not the home bar; in a ride draw past the safe
area and keep only the HUD in it; backgrounds run on to the window's edges. Before this, M106 made the
iPhone window's `width`/`height` the whole safe area and drew black in the notch/island and home-bar strips.

Taken from opentennis (`crates/hst-base/src/widescreen.rs`, B608/B623/B630/B657, `phone_window`):

- **The rule for what runs on.** opentennis's `bleed_in`/`bleed_bars`: an edge of a backdrop on the view's
  edge moves out to the window's; past the picture only what spans it runs on. opentennis tags nodes with
  `Bleed`; openmx's menus and HUD are flat sprite lists with no such tag, so the rule is applied to every
  sprite by shape: `game_render::gpu::bleed_out` runs on a *plain, axis-aligned* sprite that spans the frame
  side to side or top to bottom (walls, the band, popup/pause dimmers, the loading cover, fades), each of its
  edges on the frame's edge moved out by the bleed. Text, pills, buttons and side cards never span the frame
  and stay put. Images stretch with it, as opentennis's `Bleed` stretches plain images.
- **The 3D picture fills the window, the HUD the inset rect** (B608): new `Renderer::bleed` (left, top,
  right, bottom, target pixels, default 0). A view whose edge is on the frame's edge is drawn out to the
  bleed (`view_rect`, `picture_rect`), and its projection and cull are built from that bigger rect, so the
  picture widens rather than stretches (same vertical field of view, more to the sides). Both the full-size
  and the lowered render-scale paths do it; the sprites' scissor is the bled rect, the CPU overlay patches'
  stays the frame. `render_over` (the Garage's bike over the menu) never bleeds. With a zero bleed every
  rect is what it was, so the desktop and the web draw unchanged.
- **iOS window** (`ios_gpu.rs`): the insets kept are `aspect::kept_insets` — the safe area with its bottom
  (home indicator) dropped, the user's rule on top of opentennis's (which keeps the home indicator's 21 pt);
  none at Stretch. `width`/`height` are the room inside them; `draw` places the frame with
  `aspect::phone_frame` and sets `renderer.bleed` to the rest of the window along each axis the frame fills
  (all round at Auto; not across a fixed shape's bars). The tap overlay's dimmer covers the window, its
  text is centred in the HUD's rect. The window asks for `prefers_home_indicator_hidden` and defers all
  screen-edge system gestures, as opentennis's `phone_window` (B623).
- **Stretch**: `Aspect::frame` records whether the shell draws at Stretch (`aspect::stretched`), the iOS
  window reads it each poll and flags a resize when it changes, so Stretch gets the whole window.
- **Touch pad**: laid out from the same kept insets, so its bottom-anchored buttons come down by the home
  bar's 63 px; the shoulders (from the top) stay.

- **A load flake on the way** (AGENT.md: a test that fails under load is a bug in the test):
  `tests/loading.rs::a_cancelled_load_goes_back_to_the_pick_with_nothing_running` red in this task's gate
  at load 36 with "cancelled at Props" — `OPENMX_LOAD_CANCEL=10` counted frames from the load's start, and
  a busy machine drew ten before the load reached its `OPENMX_LOAD_HOLD` stage. Back now waits for the hold
  as well when a hold is set (main.rs's loading screen); passed alone before and after.

## What the original does

Nothing: front end and platform presentation (AGENT.md: menus are ours, presentation by eye).

## How it was proved

- `crates/openmx/tests/aspect.rs`: an iPhone 15 Pro sideways (2556×1179 px, 59/0/59/21 pt): kept insets
  177/0/177/0; HUD rect `[177, 0, 2202, 1179]` (59 pt at the island, 0 at the bottom); picture = the whole
  window; scale 2's leftover half pixel still bleeds; upright keeps the island's top, not the 34 pt foot;
  Stretch is the whole window with no bleed; 4:3 keeps its bars (no bleed); a menu wall and a bottom band
  reach x 0 and 2556 while a one-edge card and a button stay.
- `game-render` `a_bleed_runs_the_picture_past_the_frame` (headless GPU, ran — not skipped, checked with
  `--nocapture`): a frame 8 px in from each side, no bleed → black outside; bleed → the view's backdrop at
  the target's edges, a full-frame sprite at the edges, a one-edge sprite not; at render scale 0.5 too.
- `touch_pad::tests::the_pad_sits_over_the_home_bar`.
- `tools/ios-build.sh check -p openmx --bin openmx`: compiles for aarch64-apple-ios, no new warnings.

## Not verified / not 1:1

- **Not seen on an iPhone.** No device run: the tests prove the rects and the renderer's bleed, not how it
  looks under a real island, nor that UIKit honours `prefersHomeIndicatorAutoHidden` /
  `preferredScreenEdgesDeferringSystemGestures` set at window creation through the winit fork (opentennis
  does the same through Bevy's window). Needs the human's phone.
- **Images stretch, not crop**, when a spanning picture runs on (the menus' grey `background`
  gradient by ~16 % sideways on an iPhone sideways). Same as opentennis's plain `Bleed`; a gradient shows no
  difference, a photo-like backdrop would.
- **The rule is by shape, not by tag**: any plain axis-aligned sprite spanning the frame runs on, so a
  full-width HUD or menu element meant to stop at the safe edge would run on too. None found that should
  not; not audited sprite by sprite.
- **Fixed shapes (4:3, 16:9, …)** keep black bars round the frame on the phone: the picture does not run on
  across the bars and menu walls do not run into them (opentennis B629 does). The task asked for Auto.
- **A see-through sprite is drawn once, stretched** — not opentennis's B657 inside/outside split, which only
  matters for its two-camera copy; here there is one sprite, so no double blend.
- **Split screen on a phone**: a split's outer edges run on (each view's edges on the frame's edge), the
  inner seams stay; not looked at.
- **CPU overlay patches** (`Patch`) stay inside the frame; only sprites run on.

## Next

Nothing for this task; a look on the phone by the human.
