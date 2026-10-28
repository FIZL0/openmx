# 1 — bike meets the train (FINAL)

Task S129. 2026-10-27.

## What I did

- Made two states from S89's along state (copy 2, `research/s129/tele.py`, `put_state.py`,
  `keep_state.py`): **braked**, with the bike braked across the z≈2319 rail at x≈3498 and the engine
  four seconds off; and **across**, with the bike 70 ft short of the rail, facing it, the train going by.
  Each was recorded at quarter speed (copy 2's NominalScalar set to 0.25 in its ini, because KIT_SPEED
  only applies when a copy is made) with `watch/s129-train.toml`. That spec is s89-rails plus the
  engine's path clock and position. The pads are `watch/pads/s129-braked.pad` (square + R2 held 60 s)
  and `watch/pads/s129-across.pad` (cross held 44 s). The 360 + 300 rows are in s2's
  `context/capture/s129`, and the states' RAM in `context/states/s129-{braked,across}.eeMemory.bin`.
  `research/s129/save_at.py` saved two more images mid-ride: braked frames 16342 and 16344,
  `s129-braked-{11,12}`.
- Added `tests/replay.rs` S129. It seeds the 21 path cars from the image (vtable scan, path keys,
  numbers, state, meshes off the disc), runs them with S81's traffic, and replays both rides from
  their states (`the_bike_meets_the_dallas_train_as_the_original_does`, ratchets `[1, 147]`).
- Found why the original's braked bike never meets the train
  (`the_parked_bike_never_gathers_the_train`; the oracle note is S129).

## What the original does

- **Order of updates.** The path cars run before the bike in a frame. A capture row's train clock is the
  port's car clock one bike tick on, in both rides and on every tick. This settles S136's open
  "order of cars and bike".
- **The wheel probe only tests the bike's own search.** The wheel probe (+0x7c8) collides only against
  the bike's nearby search (+0x7bc), through `collide_against_search` in `./kit fn 257a00`. The search
  is gathered around the body rays after the substeps. It is gathered again only once their reach leaves
  its bound, which was 44 ft in the braked image.
- **A ghost gather leaves the cars out.** The gather drops what the body rays' category list excludes.
  While the bike is a ghost that list holds 0x40, so every car, toy and bike is dropped.
- **The braked state never re-gathered.** In braked, the bike last gathered as a ghost and has not moved
  since. Its 11 candidates are scenery and race gates only. The train runs straight through it, and the
  rail stays under both wheel rays even with the front ray inside a wagon's side (frame 16344).
- **The across state holds all the cars.** In across, the bike has moved since, and its 36 candidates
  include all 21 cars. The bike reaches the wagons' side. Its wheels take swept contacts on the wagon,
  it is pushed back out toward −z, and it rides off without a wreck.
- **+0x1e0 (the previous matrix).** It holds the node's previous pose, written with the node copy.
  +0x120 and +0x1a0 are the current pose. An ELF scan of the contact code found no reader of +0x1e0, so
  the contact uses the current pose.

## How it was proved

- **Save-state captures replayed bit-exact** (`tests/replay.rs` S129) in braked:
  - The port leaves the cars out of the bike's scene when the image's search holds none of them
    (`SaveState::bike_search_holds_cars`).
  - Over all 360 ticks, these agree bit for bit:
    - the chassis cofg;
    - both wheels' ground normals, `ray_hit` and `swept_contact`;
    - the engine;
    - the train clock.
  - The test asserts these lanes. It also asserts that the original's bike is never shoved more than
    0.25 ft (it creeps 0.18 ft) and never wrecks.
- **across** agrees on every lane through tick 147, including about 60 still ticks before the gas. The
  train clock agrees on every tick.
- **RAM images:** the braked images hold no candidate in category 0x40, and across holds 26. From image
  12's seeded wheels, over the scenery alone, the port's wheel probe gives the original's two rail
  contacts:
  - the normals are bit-exact;
  - the points and fractions are within 0.001.

## Not verified / not 1:1

- **The tick-2 creep in braked.** At tick 2 the original's chassis velocity has about 0.003 ft/s in z,
  and both wheels spin at 0.00144; the port has neither. Every velocity, quaternion and force lane parts
  there by thousandths. Positions stay bit-exact, apart from cofg_world/origin a few ulps off from tick
  144. Source not found.
- **The parked bike's search is not ported.** The braked replay stands in for it by leaving the cars out
  of the scene. The port's own `ride.rs wheel_hits` still gathers afresh every call over every prop, so a
  ghost-gathered parked bike in the shell would meet cars the original never does. Filed as S185.
  `bike_search_holds_cars` is a test-side switch, not the original's rule. The distance cut, "a
  candidate only within the wheel probe's reach", is also not ported.
- **The wheel record at frame 16344 is not bit-exact.** Its points and fractions differ from the port's
  by a few ulps and about 90 ulps in fraction. Assumption, not proved: the probe ran from a substep's pose,
  not the frame-end pose the image holds.
- **across parts at tick 148**, where the wheels' swept contacts and the body meet the wagon. Not ported
  bit-exact: filed as S186. The temporary bike_col (S137 body rays) run parted at 148 too.
- **No wreck and no roof ride were recorded.** Filed as S187. S146 and S170 still need them. Braked does
  not answer S170: its bike was a ghost-gathered one, and a bike that rode onto the rail would have the
  cars in its list.
- **+0x1e0 having no reader is a static scan** of loads in the contact code only, not a live trace. A
  reader through a computed base would be missed.
- **The states are teleported (tele.py)**, not ridden into place. The captures and images exist only in
  s2's context; elsewhere the tests skip and read as passing.
- **The braked "ghost" history** is inferred from the category list and the ghost switch S93 read. The
  moment of the last gather was not seen.

## Next

S185 (the bike's persistent search), S186 (across's contact with the wagon), S187 (record a wreck and a
roof ride).
