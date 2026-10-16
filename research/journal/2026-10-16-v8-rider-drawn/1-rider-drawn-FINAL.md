# 1 — rider drawn (FINAL)

Task V8. 2026-10-16.

## What I did

Put the rider on the bike and posed it every frame, out of the animation driving the simulation was
already running and throwing away. Five pieces, all additive:

- **`game_sim::rider::Rider::animation`** — the sim's rider ran `motion::Driver::frame` for the one
  field it needs (the crossup the landing test reads) and dropped the `motion::Frame` the driver
  answered. It now keeps it. The rider itself still reads nothing out of it; the shell does. The
  alternative was a second `Driver` in the renderer, which would be two state machines where the
  original has one and the second one guessing at the first one's inputs. `impl State for Rider`
  walks its fields by name and was not touched, so replay hashes are unchanged (`tests/replay.rs`
  is green).
- **`game_render::anim::Rig::driven`** — the back half of `Rig::frame`, split out: rewind on a
  restart, clock, sample, pose. `Rig::frame` now calls it, so a rig driven from outside and a rig
  driving itself run the same code. It answers `(Played, bool)`, the bool being whether the frame
  wrote a pose at all — a blend with no clip behind it writes nothing and leaves whatever was
  there. `Rig::bound` is the constructor for a rig whose clip table was filled elsewhere.
- **`game_render::rider::Body`** — a rider's three files (`.sub` → `.mcf` → `.psg`), its animation
  `Set`, and the clip table, loaded in one call: the set's own motions (`load_listed`), then one
  clip per gesture slot, then the three airborne poses. `Body::pose(dt, frame)` poses it from a
  frame of the simulation's driving; `Body::skin_into` writes the frame's triangles into the loaded
  model the scene draw already knows about, so the rider goes through the same lighting, fog and
  texture path as everything else and `world.rs` did not have to change.
- **the shell** (`crates/openmx/src/main.rs`) — `--rider thq1|none`, the rider archive opened a
  second time for the clips (a `Set` owns the archives it reads `.mot` files out of), the body
  posed after `engine.step` from `world.rider.animation`, and the geometry drawn at the bike's own
  transform.
- **`first-draw --pose NAME[:PHASE]`** — a still of the body in one of its own clips. The shell's
  `--shot` writes the first frame of a ride and the first frame is the one frame that is still the
  bind pose (below), so without this there was no repeatable way to look at a posed rider.

### The clip names are the rider's, not the bike's

`game_sim::motion::Library::stunt_name` builds `<prefix>bike_<stunt>` and asks for
`bike_air_Fwd/_Mid/_Bck`. Those are the **vehicle's** clips. The body's own set has none of them:
it holds `rider_<stunt>` per gesture slot, and its three airborne poses are plain `air_Fwd`,
`air_Mid`, `air_Bck` with no prefix at all. A body loaded with the bike's rule binds nothing.
`game_render::rider::stunt_name` is the rider's rule, truncated to 31 bytes the same way (the
original's buffer is 32 with the terminator), and `AIR_POSES` the three names.

### Where the body sits

V4 had to choose a placement (`bolt_onto` on the foot anchors) and said so: the bind pose is
modelled at the origin and the game never draws it. The clips answer the question instead. Every
clip in a rider's set writes the pelvis's position channel, and posed by a clip the rider rides in
the four dummy anchors the two skeletons share land within **a third of an inch** of the chassis's
four — feet on the pegs, hands on the grips — against five inches out in the bind pose. So a posed
rider is drawn at the bike's transform and bolts nothing. `bolt_onto` stays for what it was written
for: a body with no clip to pose it (`first-draw` with no `--pose`).

Gestures are the exception that proves it: `hold_knee_left` takes a foot four inches off its peg,
`wreck_sep_over_bars` throws the body over the bars. That is the stunt being done.

### The first frame is the bind pose, on purpose

`Driver::frame` restarts the motion on the first frame of a ride, and a restart puts the cross-fade
ramp back to 1 — so the first frame asks for the grounded idle and draws all of the pose the rider
was already in, which before anything has posed it is the pose the body was modelled in. It fades
out of it at the bike's blend rate: 3 per second on this `.phy`, so 20 frames. A screenshot of
frame 1 shows a rider with its arms out and nothing is wrong.

## What the original does

- **The vehicle's clip table** is `rider_load_clips` (`./kit fn 0x00258790`, `research/oracle.txt`):
  two passes over the 24 records, `snprintf` into a 32-byte buffer with the format at `0x004336d0`
  (`"%sbike_%s"`), then `"bike_air_Fwd"`, `"bike_air_Mid"`, `"bike_air_Bck"` at
  `0x00433700..0x00433720` straight into the three named fields. S20 ported this and V7 bound it.
- **The rider's prefix** is a different pair of strings: `"ff_rider_"` at `0x0042ead8` and
  `"rider_"` at `0x0042eb10`, chosen on a mode flag and handed to the body actor's spawn in
  `./kit fn 0x0020e4f0` — the same two modes the disc spells `[Stunts]` and `[ff_Stunts]` in the
  rider archive's `motions.ini`. The `snprintf` that joins that prefix to a stunt name for the body
  is not located (see the last section); the rule is read off the prefix strings and the disc.
- **The pose** comes out of the animation set's own blend entries, which V7 ported
  (`Set::apply` → `pose`, `pose_faded`, `pose_mixed_faded`); the cross-fade's `under` buffer is the
  motion object's snapshot, taken by `anim_rewind` on a restart (`research/oracle.txt`, S20).

## How it was proved

`./kit check`: **462 tests pass** (457 before this task — 5 new, all in the new
`crates/game-render/tests/body.rs`, all against the user's own disc and all skipping with a reason
when there is no disc. Past the disc itself they are loud rather than skipping: a disc with no
rider, no stunt table or no bike fails them).

1. **every gesture slot and airborne pose is bound** — `thq1` loaded with the disc's own stunt
   table: 63 clips bound, all 24 gesture slots and all 3 airborne poses, each bound clip checked to
   be the one the name asks for, to have a length, and to reach at least one bone of the rider's
   skeleton through its joint map.
2. **a posed body stands on the bike with nothing bolted** — the four shared anchors, measured
   against the chassis's own nodes, for six riding poses: `turn_still_Mid`, `turn_riding_Mid` and
   the three airborne poses at the midpoint the grounded idle plays them at, and `backflip`. The
   furthest anchor of any of them is **0.258** (`air_Fwd`); the bind pose's is **5.050**.
3. **a minute of riding poses the body every frame** — the real `.phy`, the ported rider on
   `game_sim::world::Flat`, dropped in with a stunt asked for on the way down and then full
   throttle: 3600 frames, every frame posed, every joint transform finite, the geometry skinned to
   a non-empty triangle list every sixth frame, no panic. The blends it went through: 3598
   `Blend::One`, 2 `Blend::Two`.
4. **a stunt's first and last frames are different poses** — the shortest stunt the disc's table
   gives this rider (`cancan`, 1.330 s), asked for in the air through the real gesture decode: the
   furthest joint moved **4.770** between the frame the stunt started on and the last frame it was
   in progress.
5. **the first frame of a ride fades out of the bind pose** — frame 0 is a restart, writes a pose,
   and that pose is the bind pose to within 0.001; 20 frames later (the `.phy`'s blend rate of 3)
   the body is 4.959 away from it and riding. This one is here so the next reader of a `--shot`
   does not have to re-derive it.

Looked at, not committed (screenshots never are): `first-draw --pose turn_riding_Mid` and
`--pose backflip` at 1280x896 — the body is in gear, crouched over the bike with its hands at the
grips and its feet on the pegs, at the right scale, and the two poses differ as the clips do. And a
live 60 s run of the shell on `Supercross01`: 3568 frames, ~59.8 fps, ~18k triangles a frame, no
panic, the rider drawn throughout.

## Not verified / not 1:1

- **Nothing here was compared against a frame of the running original.** No capture, no
  side-by-side. The proofs are against the disc's own data and the ported driving; that a drawn
  rider *looks like* the game's is my eye on a screenshot and nothing more. A PINE recording of the
  rider's joint array would settle it and is a task of its own.
- **The rider's clip-name rule (`<prefix>rider_<stunt>`) is inferred**, from the two prefix strings
  in the image (`0x0042ead8`, `0x0042eb10`), the disc's `motions.ini` sections, and the fact that
  all 24 of this rider's slots bind with it. The `snprintf` that builds the body's clip names was
  not found in the decompilation — the one I did find (`0x00258790`) is the vehicle's, with
  `bike_` hard-coded in its format string.
- **The rider's own animation state machine is not ported.** The body is posed from the *vehicle's*
  driving (`game_sim::motion::Driver`, S20/V7). Whether the original runs a second, rider-specific
  selection on the body actor — the `get_ready_*`, `wreck_*` and `reverse_*` clips in the `.mcf`
  suggest something asks for them — is unknown, and nothing in this task asks for those clips.
- **`Blend::Three` is still never drawn**, as V7 left it: no arm of `Driver::select` produces one.
- **One LOD, no IK, no gear swap.** The body is drawn from LOD group 0 of its `.psg` at every
  distance; the original's five groups and whatever picks between them are not ported, and neither
  is the inverse kinematics the four dummy anchors exist for (which is presumably what keeps a
  hand on the bar through a whip).
- **The rider archive is opened twice** — once for the geometry and the gear, once for the clips —
  because an animation `Set` takes ownership of the archives it reads from and `Res` is not
  `Clone`. The original has one copy. This costs load time and memory, not behaviour.
- **The blend rate is the bike's**, from `Phy::motion()`, which is how S20 ported it; the rider's
  body may have one of its own in a file nobody has read yet.
- **`first-draw --pose` is not a port of anything.** It is a debugging still.

## Next

Nothing outstanding for V8 itself. Two things a later task could take up: compare a drawn rider
against a captured frame of the original (the only proof this task could not make), and find out
whether the body actor runs a selection of its own for the `get_ready_*` and `wreck_*` clips.
