# 1 — mod hidden props (FINAL)

Task MD13. 2026-10-23.

## What I did

- `game_sim::track`: `ActivityModel` (the three `[Model<n>]` `Type`s MX vs. ATV builds as activity
  models), `ActivityModel::free_ride_hides`, `switch` (MX vs. ATV's ini switch), and a `FreeRide` pass in
  `Scene::parse`. It drops the models a free ride hides, and the `[Object<n>]` shapes a free ride takes
  out of the world. `Scene::parse` now hands back the scene as the Free Ride activity leaves it. That is
  the only ride the port has on these maps.
- `game_render::scene::Scene::parse` skips the same models, through the same predicate. `world.rs` draws
  `scene.models`, so it needed no change, and the sim's `Props::load` reads `scene.collision`.
- Tests: `track.rs` unit tests `a_free_ride_leaves_out_the_hidden_race_props_and_their_shapes` and
  `the_switch_reads_as_the_ini_reader_does`, `scene.rs` `a_free_ride_draws_no_hidden_race_prop`, and the
  disc test `tests/track.rs` `every_disc_scene_parses_with_no_activity_prop`. That one goes through all
  57 scenes in the 57 `Tracks\*.res` (not `_2`). Each `[Model]` with an SLT and each `[Object]` with a
  COL is in the parse, and no model's `Type` is an activity class. I proved it ran: with the count assert
  raised to 40000 it failed with "57 scenes in 57 track archives".
- Nothing taken from opentennis: it has no scene format like this.

## What the original does

Read from SLUS_211.04 in the MX vs. ATV Ghidra project at `openmx-mods-slots/s1/extracted/mxvatv`, with
read-only use of its `context/decomp` and `KIT_CONFIG=…/kit.toml ../ps2-decomp-kit/kit dis`. MXScene.cpp:

- **The model factory** (`0x00323cc0`, MXScene's override of the scene's create-model virtual). It reads
  `Type` and compares it without case (stricmp). `ActivitySpecific` gives an ActivityStaticModel,
  `Waypoint` a WaypointStaticModel (one that also reads `Waypoint`, default 0x7fffffff), and `StartZone`
  a StartZoneStaticModel. `PlayerFacing` gives a PlayerFacingStaticModel, which is its own class and is
  never in the masks below. Every other model goes to the base factory (`0x00319690`).
- **Their keys** (`0x003227f0`). `Activity` is a string with default `None`. Flags at +0x34:
  `AlphaFade` (default 0) is 4, **`Hide` (default 1)** is 1, `Invisible` (default 0) is 2, **`HasCOL`
  (default 1)** is 8, and `IsTrigger` (default 0) is 0x10. The ini reader takes keys without case
  (`HasCol` in the files is `HasCOL`). Its switch (`0x00142ed0`) is on for a `T`, `ON` or `Y` prefix,
  off for `F`, `OFF` or `N`, and anything else is strtol.
- **Once the scene has loaded**, `0x00325540` runs over every model of the three classes. AlphaFade sets
  the alpha to 0.5. Hide calls the model's hide virtual. HasCOL walks the scene's collision objects and
  takes out of the world each one whose shape name *contains* the model's name. The match is a strstr
  that ignores case (`0x0026cc80`, shape name at +0x9b, model name at +0x110).
- **`0x003256c0` is the inverse**: alpha 1, show, and the collision put back. GAME_REL calls it in one
  place only, `0x00524050`, with the StartZone class's mask. The class destructor `0x00523e70` hides
  them again. That class is built by case 17 of the activity factory's switch (`0x00325ff0`, table
  `0x003abf80`, 18 cases). Case 17 is the 18th activity name, `free ride` (`0x003a388b`, listed from
  `supercross` at `0x003a3785`). So **a free ride shows every start zone, whatever its `Activity`, and
  nothing else**. Waypoint and ActivitySpecific models stay hidden, and their named collision stays off.
  Other activities pick their own gates by `Activity` name (`0x00323fd0`). The port runs none of those.
- **`[Object<n>]` reads none of these keys.** The collision-object reader `0x003217c0` reads COL,
  position, vectors and its own flags, but no Type, Hide or Activity. An Object leaves the world only
  through a model's HasCOL name match. Nat03's `[Model230]` `SLT=AirRace_Start` is how `[Object189]`
  `COL=AirRace_Start.col` goes.

So the brief's "a Hide=True model or object is neither drawn nor collided" is right in effect for every
map but one. It comes about through the model's two flags, not through the object's own keys. I ported
the original's rule.

## How it was proved

- A scratch test, run once and removed, parsed all 56 imported scenes under
  `~/repos/openmx-mods/out/mods/tracks/mxvatv_*` with the new reader. The fs, sx, sm and mc02–mc08
  maps are unchanged (they have only start zones, or no activity models). Nat03 goes from 268 to 232
  models (its 36 Waypoint gates) and from 213 to 189 shapes. Nat04: 478→182 models and 251→245 shapes.
  Mc09: 66→27 and 52→26. Every count matches an independent Python tally of the same rule over the same
  files. On every map but mc09, each `[Object]` that carries a race `Type` is among the shapes removed.
- The disc test above: MXU's 57 scenes are unchanged.
- The rule itself comes from reading the decompilation, listed above. No live MX vs. ATV run checked it:
  there is no emulator setup for that game here, and the port does not ride it.

## Not verified / not 1:1

- **The strstr name match.** The model's name (+0x110) is taken as its SLT with the spaces dropped. The
  shape's name (+0x9b) is taken as the COL value as written. Neither offset was traced to where it is
  filled. Every typed Object but one on the 56 maps does match its model this way. A shape name kept as
  a bare stem or as a full path would match the same.
- **mc09's `Ring_Trigger.col`** (`[Object52]`, Type=Waypoint, Hide=True, IsTrigger=True) stays in the
  free ride's world. Its models say `HasCol=False`, so in the original nothing takes it out either.
  Whether the original's world treats it as solid was not followed. The collision-object reader's flags
  may make it a trigger volume. In the port it is a solid shape, high up on an air-race course.
- **AlphaFade is not ported.** An activity model with `Hide=False` and `AlphaFade` on is drawn at half
  alpha in the original and at full alpha here. None of the 56 imported scenes has a Hide=False activity
  model, so nothing on them is drawn wrongly.
- **`Invisible` and `IsTrigger` are read by the original but not by the port.** Neither appears in the
  hide/show passes. What reads them later (triggering, the waypoint logic) was not followed. A free ride
  never shows those models.
- **The activity number.** That switch case 17 is Free Ride is inferred from the name table's order (18
  names up to `free ride`, 18 cases). The enum was not traced to where the activity is chosen.
- **Collision put back twice.** If a shape's name matches both a hidden gate and a start zone, the port
  keeps it once. The original takes it out and puts it back. Whether putting back a shape that was never
  taken out adds it twice was not looked at. No imported map has such a pair.
- **Other activities.** Only the free ride state is ported, since the port has no MX vs. ATV races. A
  race on a mod map would need `Activity`-matched gates shown (`0x00323fd0`) and the start zones hidden.
- **`notes/openmx-asks.md`** is in the openmx-mods repo, not this one, and I did not edit it. Its MD13
  entry ("what the game does … is still to be found") is answered here, and openmx-mods can mark it
  from this entry.

## Next

none — done.
