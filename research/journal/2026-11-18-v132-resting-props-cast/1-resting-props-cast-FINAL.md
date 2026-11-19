# 1 — resting props cast

Task V132. 2026-11-18. User: "the props dont have shadows until they're touched, fix".

## What I did

- **Cause.** Under Shadows On, only movers cast. A knockable prop (S131) counted as a mover only once
  `move_scene_prop` / `move_owned_prop` had moved it. So a resting crate cast nothing, and the first
  strike made its shadow appear.
- **`Renderer::loose_props(owner, ks)`** (`crates/game-render/src/gpu.rs`): marks props as movers from
  now on. It sets the same flag a strike sets, so the prop casts into the movers' maps under On and
  leaves the baked world map under All, as a struck one does. `Renderer::SCENE_PROPS` names the
  scene's owner for it.
- **The ride** (`crates/openmx/src/main.rs`): right after `set_scene`, it loosens every knockable
  prop's scene model (`prop_looks`). After each world tile's `add_props` (at load and when a tile
  streams in), it loosens that tile's knockable props. `world_loose` finds them by name and place,
  the same match the ride already uses for a struck one.
- **Docs**: `shadow.rs`'s `Caster` and the prop loop's comment now say a knockable prop is a mover,
  struck or not.
- **Test**: `crates/openmx/tests/shadows.rs`
  `untouched_knockable_props_cast_their_shadows_under_on`. It shoots free09's farmyard garage from over
  its mouth, Off and On, with no prop touched (`props moved: 0 of`). On darkens 1,923 pixels by more
  than 20 grey levels. With the loosen call commented out it darkened **0**, so the test fails without
  the fix.
- opentennis has no knockable props, so nothing was taken from it.

## What the original does

The original's projected shadow (V108) is a pool of up to eight 128x128 casters, made in
`./kit fn 0x001a5950` (one per racer slot). I did not find where it hands a caster to a prop. So the
original's own rule for prop shadows is still unknown (see below). The task asked for the rule the user
sees: a knockable prop casts the same before and after a strike.

## How it was proved

Looked right on screen, plus a test.

- Shots of free09's garage, `--shot-pose 4640,170,3790,0,-45,60`, Shadows On / High:
  - before the fix: the crates and barrel cast nothing;
  - after: each casts on the floor.
- The same spot inside world_mxu_free (free09 tile at 15840,-720, so the pose is 20480,170,3070), Off
  beside On: On shows the crates' shadows with `props moved: 0 of 196 bodies`. That checks the world
  tiles' path.
- The test above: 1,923 pixels with the fix, 0 without.

## Not verified / not 1:1

- **No PCSX2 shot of the same spot.** The brief asked for one. My copy (1) would not take it:
  - `first_recording.p2m2_SaveState.p2s` loaded into slot 8 never landed (frame counter stayed 0 with
    the VM running);
  - this copy's own newer free09 states (slots 1 and 3) did load, and `research/s76_tele.py` (with
    the rider found the way `research/c37/save_at.py` finds it) moved the bike to 4645,137,3780;
  - but the game then ran at about 2 frames a second, and `./kit pine save 9` (paused, then
    running) wrote no file, so there was no Screenshot.png to read;
  - the pad is unavailable (`/dev/uinput` root-only), so neither the hotkey nor `./kit shot` could
    take a picture.
  So whether the original draws shadows under resting props, and how dark, is not checked. The fix
  follows the user's report and the brief.
- **Which props the original shadows is not read.** The projected-shadow pool's casters were not traced
  past their creation. If the original gives props no projected shadow at all, then our On shadows for
  knockable props, struck or not, are an addition. That was already true of struck ones before V132.
- **Under Original quality** props still cast nothing, struck or not. Its maps take racers alone (V108's
  rule), and this task did not change that.
- **The world tiles' path** was checked by one On/Off pair on world_mxu_free. I did not take a shot of
  it from before the fix.

## Next

None: the PCSX2 picture is the one open check, above.
