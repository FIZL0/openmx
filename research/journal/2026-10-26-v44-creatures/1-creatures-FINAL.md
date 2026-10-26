# 1 — creatures (FINAL)

Task V44. 2026-10-26.

## What I did

- `crates/game-render/src/creature.rs`: a track's `.toy` bird manager (`[BirdSplineN]` `.vue` splines,
  `[BirdN]` single birds, `[BirdFlockN]` flocks laid out the original's way, `[TriggerN]` with
  `ScareBirds` actions) and its `[SequenceAnimatedToyN]`s, each body read from its `.mcf` (`SLTFile` →
  `.psg`, `VUE` → `.mot`, an unlisted `MotionToPlay` name → `<name>.mot`, `Looping` read as a switch so
  the files' `TRUE` counts). `Creatures::step(dt, riders)` is one tick; `placed(blend)` is the picture.
- `crates/openmx/src/main.rs`: loaded after the ride is built (not in a world), stepped in the
  `if simulated` block with every seat's rider position, drawn on both renderers (GPU with a pose
  palette, software skinned into a copy of the model per creature).
- Tests `crates/game-render/tests/creature.rs`: `.vue` axes, the flock offset pattern, free06's 50
  crows (layout same every load, all moving after 300 ticks), nat02's three perched vultures sitting
  until a rider stands in `vulture_10_trigger` then flying away, nat04's seven camels moving through
  their sequences, nat07's mills and elk, nat01's nothing (its spline files and `crow_02.mcf` are not
  in any archive on the disc).
- Looked at: `openmx --track nat04 --headless --frames 40 --shot … --shot-pose 2471.6,104,3121.3,15,-12,60`
  shows Camel_01 upright and posed by its clip; free06 `--shot-pose 4270,330,4516,-90,-5,60` at frame 40
  shows a crow in flight.

## What the original does

See research/oracle.txt V44 for the recipes. In words:

- Flocks: a third of the flock size (`0.333 × N`) is the spacing in nodes; `int(N/3 + 1)` groups of
  `3 + int(rand·2)` birds each sit on one node, aimed at the next. Every bird after the manager's first
  then gets an offset from a pattern of eight that widens by `BirdFlockOffset` each ring.
- A flying bird moves at its spline's `FPS` feet a second toward its target; within 2 ft it steps a
  node, re-aims (on a looping spline the target is raised by the offset's height and pushed sideways
  across the node's up), and faces the target with the current node's up.
- At the end of a non-looping spline: land (idle, facing `PerchLookVector`) if the next spline is not
  looping; else switch to it (`TakeOff` when going from a non-looping to a non-looping spline, else `Fly`).
  On a looping spline, passing a free landing spline's `FromNode` turns the bird onto it, where it
  slows to between 0.35 and 1 of the speed and plays `Land` three nodes from the end.
- `ScareBirds` runs the transitions on every sitting bird within `ScareDistance`, steps it back a node,
  re-aims and sets it flying.
- A sequence toy, whenever its clip has run out, rolls `rand` against each step's percent in turn from
  its cursor (after `count` misses the next one plays regardless).

## How it was proved

Read from the decompilation (`./kit fn` of the functions in oracle.txt V44) and checked against the disc
data by `./kit check -- -p game-render --test creature` (6 tests, disc present, none skipped), plus the
two screenshots above. No PINE capture of the original's birds: they are random per boot there
(`srand` from the clock), so a frame-by-frame comparison is not possible without seeding its RNG.

## Not verified / not 1:1

- **The original's random stream.** Its `rand()` is shared with the rest of the game and seeded from the
  clock, so its flock groups and camel sequences differ every boot. The port has a generator of its own
  on the library's default seed: same every run, never equal to a given boot of the original.
- **No animation level of detail.** The original animates only the `MaxAnimatingBirdsPerTick` birds
  nearest the camera within `FarDistanceToAnimate` (others hold their pose; they still move), and a
  sequence toy only while it is on screen (`AnimateWhenNotVisible=False` everywhere). The port animates
  all of them every tick, because the tick has no camera. Off-screen camels therefore change clips at
  different moments than the original's.
- **Trigger shape.** I took `Box` as half-extents along right (up × look), up and look about `Position`,
  and the rider's position as a point that fires the trigger every tick it is inside. The original adds a
  collision shape (`0x001e7b10`) touched by the bike's body; whether `Box` is half or full, and whether
  the contact is with the rider, the bike or both, was not proved. nat02/nat19's scares are the only use.
- **Perches** (`[BirdPerchN]`) are not ported: the only perches on the disc (nat17's toucans, nat01) have
  their landing/take-off `.vue`s missing from the archives, so no bird could reach them.
- **Motion changes are immediate.** The original's set-motion blends or queues (`0x00146490`) unless the
  current clip has stopped; the port always starts the new clip from its start.
- **Facing.** Toys use `upright(Position, LookVector, UpVector)`, which is the convention the scene props
  were proven with (V41). Nat04's camel then shows its tail toward its `LookVector`; that matches the
  same placement but has not been compared with a screenshot of the original. Birds keep identity
  orientation until they reach their first node, as the original's init leaves it; then face along
  travel (the crow's head is on its +Z, by its skeleton).
- **nat19's perched vultures** write `Position=` rather than `StartPosition=`; the original's reader
  defaults StartPosition to zero, so they start at the origin, are never within the scare's reach, and
  never move. The port does the same; not seen in the original.
- **Online rollback** does not rewind creatures: they are stepped with each simulated tick and not part
  of a snapshot. Visual only, so a resimulated tick only nudges them.
- **Planes**: a seat flying the biplane still triggers with its rider's position.
- **World mods** load no creatures, as they load no home-tile cars.
- **Performance**: the software renderer re-skins every creature every frame (50 crows on free06);
  not measured.

## Next

Nothing for this task. Follow-ups filed with `./kit plan add`.
