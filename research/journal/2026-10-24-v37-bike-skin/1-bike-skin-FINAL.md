# 1 — bike skin (FINAL)

Task V37. 2026-10-24.

## What I did

Went down the bike's chain the way the task lists it — the `.sub`, the chassis model, the parts, their
materials and textures, the LOD groups, node flags — for the shell's default bike (`125sx`) and then for
all 54 bike archives on the disc. Three faults, all in how names are read, none in the geometry:

- **The white plastics and the "missing" seat.** `125sx.sub`'s `[Material Changes]` repaints the parts'
  plastics `250SX_PlasticsAll=125 SX_PlasticsAll`, with a space. The original's text reader drops every
  space outside quotes before it splits a line (the rule `scene::slt_name` already documents for `SLT=`
  names), so to the original that is `125sx_plasticsall.psm`. `Vehicle::parse` kept the
  space, the material was not on the disc, and every mesh painted with it drew with no texture,
  which the GPU path draws white: `250SX_Plastics` (230 triangles: seat, side panels, rear fender),
  part of `250SX_Engine` (20) and the whole `250SX_Front_Fender` (108). The black seat on a white
  fender read as no seat. `Vehicle::parse` now reads the `SLT`, `Placement`, `MCF` and both sides of
  every material change through `slt_name`. Only `125sx` on this disc has a spaced name.
- **A repaint the archive has no material for.** `500_03.sub` paints its pipe
  `RM-250_RS_02_Exhaust=CR-250_Exhaust`, and its archive has no `cr-250_exhaust.psm`, while it does ship
  `rm-250_rs_02_exhaust.psm`/`.pst`, which only that pipe uses. `Assets::resolve` now keeps the mesh's own
  material when the rename's `.psm` is on no archive. (`bin/materials --bike` said "ok" for this slot,
  because its fallback report follows the unrenamed name; the new test is what found it.)
- **The fast50s had no chassis.** Their `.sub` names `bike50.mcf`, whose `SLTFile` is `bike50.psg`; the
  shell loaded `bike.psg` for every bike, which a fast50 archive does not have, so a fast50 was its
  parts floating on nothing. `vehicle::chassis_model` reads the set's `SLTFile` up front and
  `Assets::load_as` loads that file under the shell's one chassis key (`CHASSIS`, still `"bike"`), for
  player 1 and for a picked bike of another player; `first-draw` and `materials` use it too.

LODs and flags: every part has 4 LOD groups (`250SX_Plastics` 315/228/100/42 triangles), each group's
meshes resolve their own materials, and no node of any part is hidden — nothing else was wrong there.

Took nothing from opentennis: it has no repaint-by-`.sub` chain.

## What the original does

Reads `.sub`/`.mcf` lines with every space dropped (as `slt_name`'s doc says, from the earlier scene
work); no new decompilation was read for this task.

## How it was proved

- `crates/game-render/tests/bike_skin.rs`, against the disc: for all 54 bike archives, load as the shell
  does (repaint, chassis by `chassis_model`, parts) and check **every mesh of every LOD group of every
  part** has a texture, and that it is pixel-for-pixel the texture its own material (renamed by the
  build) names in the archive. `assert_eq!(checked, 54)` so a skip cannot pass for it. A second test pins
  `125sx`'s plastics to the chassis' own atlas on all three parts and `fast50_01` to `bike50`.
- `scene.rs` unit test: `125sx.sub`'s lines, cut down, read without spaces.
- By eye in the shell (`--ride --shot`, and `--shot-pose` from the side, behind, in front, and high
  behind): 125sx before — white rear fender and seat; after — orange plastics with the KTM graphics, black
  seat, number plates. 250sx, 125sx_thq_01, 250sx_thq_01 and fast50_01 (now on wheels and forks) checked
  from the side the same way.
- The original in PCSX2 (copy 3, `first_recording`'s state, `./kit shot`): from the chase camera and, after
  a wreck, from above. Its bike has a black seat on its plastics and every part textured.

## Not verified / not 1:1

- **No same-bike picture of the original.** `first_recording`'s state does not ride the 125sx: from above
  after a wreck its bike is a white livery with flames and orange rims (its RAM holds `bike\tex\250sx_*`
  names, but none of 250sx, 125sx_thq_01 or 250sx_thq_01 looks like it in the shell). Reaching the 125sx in
  the original takes the front end's bike select, which I did not drive. So the fix is proved against the
  disc's own files and the original's reader rule, and compared with the original only as "a seat and
  textured plastics", not part by part.
- **The fallback to the mesh's own material** (500_03's pipe) is inferred from what the archive ships, not
  read in the original's material loader; it could instead draw something else for a missing repaint.
- **nat10's parked bike props.** `CR_250_prop` and `YZ_250_prop` on nat10 use the bikes' material names
  (`cr-250_plasticsall`, `tire`, …). The shell sets the player's bike repaint before it loads the scene's
  props, so on nat10 that prop is painted like the player's bike. Whether the original's repaint is
  global like that or per bike was not checked (no nat10 state). Rendering only, so noted and left.
- **The fast50 rider sits high.** On a fast50 the rider is posed for a full-size bike, well above the
  little seat. Not this task's (placement, not skin), not compared with the original.
- The orange in the shell is brighter than the original's under free09's haze; that is lighting, not
  the texture choice, and is not chased (AGENT.md: render differences are noted, not chased).

## Next

Nothing.
