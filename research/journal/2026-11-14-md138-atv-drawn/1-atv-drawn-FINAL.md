# 1 — atv drawn (FINAL)

Task MD138. 2026-11-14.

## What I did

- **The rig** (`crates/game-render/src/quad.rs`): `quad::pose(model, wheels, bars)` poses the ATV
  chassis' 43-node skeleton from four `car::Tire`s (spin, steer, compression; scale not read) — each
  upper A-arm swung about its own z by `asin(compression / arm length)` so its hub rises by exactly the
  compression, the hub turned back level and steered about y, the lower arm aimed (about z) at the
  hub's `…HubNode`, the tie rods at `TieRod…Node`, the swingarm turned about x so `RearAxle` rises by
  the more compressed rear wheel on its own circle, the wheels spun, `HandleBars` turned by the
  fronts' mean steer, then the strut pairs (`FrontUpper…Strut`↔`FrontLower…Strut`,
  `RearUpperStrut`↔`RearLowerStrut`) aimed at each other. The baja's helpers in `car.rs` are reused
  (made `pub(crate)`, nothing else changed). Two unit tests: at rest the pose is the model; a
  compression lifts the hub and axle by it, the hub stays level, the lower arm follows.
- **The rider** (`quad::bind`, `quad::seat`, `quad::rider_palette`): the pack's ATV motion set
  (`atv_ridermotions.mcf`, `SLTFile=mrider.Slt`, so the clips bind to the MX rider's own skeleton)
  bound into the rider body's set under its `NAME=`s; a row (`ATV_turn_riding_*`, `ATV_turn_stop_*`,
  `ATV_air_*`) sampled at the stick's lean as a phase of the clip and mixed from `_Mid` towards `_Fwd`
  or `_Bck` by the stick's fore and aft; held at full strength to the posed chassis' `Dummy_Foot_*` /
  `Dummy_Hand_*` (`rider::anchors_of`), the body's pose, hold and feet restored after, as the plane's
  pilot is.
- **The shell** (`crates/openmx/src/quad_draw.rs`, new; `main.rs`): with `--atv`, the pack's first
  player build (`ATV_…`, no `_AI`, no ghost) whose chassis is the one `vehicle::atv_chassis` rides on is
  loaded — for `atv_Race250P` `Veh/Anim/ATV_R_250_250RTHQ1.sub`: `250R_atv_chasis` and its 7 parts,
  repainted by the `.sub`'s material pairs, textures off the pack. Drawn while the quad is up at
  `AtvRide::matrix`, slid between ticks as the car is (the seam carry moves it too), on the GPU (the
  chassis skinned by the pose, parts on their placement nodes) and in software (a `quad@seat{p}` copy
  skinned); the rider drawn at the quad's origin by both. `--trace-picture` says
  `picture atv: seat P R chassis at x y z wheels spin/steer/comp ×4 parts N row ROW phase fore_aft`
  and the usual `pilot:` line. The log says `atv: drawn as <sub> (N parts, 65 rider clips, 17 quad clips)`.
- **Test** `crates/openmx/tests/atv_picture.rs`: free09 headless, switch to the quad, gas, stick left;
  both renderers: the chassis drawn ≥100 frames with its parts, the springs take the weight, every
  wheel spins, the fronts steer left and the rears not, it moves, the riding row is reached, a rider
  every frame rooted at the quad's origin and sat (middle 1.5–5 ft up); without `--atv` nothing drawn.
  Skips without a disc or the pack.

## What the original does

Not read for this task. MD37b found which nodes the original drives; the measure step that poses them
and the ATV rider's pose choice are not ported (see below).

## How it was proved

Looked at by eye: shots of the test's ride from the chase camera and from `--shot-pose
4279,149,3411,-15,-18,50` (frame 148 of the same tape), GPU and software. The quad stands on its four
wheels in its red THQ plastics with the frame, exhaust, handguards and crossbar on; the fronts are
steered left, the A-arms and struts meet their hubs, the rider sits on the seat with both hands on the
grips and feet at the pegs. The two renderers' pictures agree. No picture of the original to set it
beside (no PCSX2 run of MX vs. ATV here).

## Not verified / not 1:1

- **The rig is the port's own**, matched by eye, not the original's ATV measure step: the arm swing
  (`asin(c / L)`), the swingarm turn, the bars turned by the fronts' mean steer, the strut aims are
  guesses with no capture of the original's node matrices behind them.
- **The wheels' spin direction per side** is the step's `spins[k].angle` as it is; the right side's
  angle runs opposite the left's in the trace — whether the original's right-side wheel nodes are
  mirrored to match was not checked (they looked right by eye at speed, which proves little).
- **The rider's row and lean are the port's choice**: air while airborne, stopped under 1 ft/s, else
  riding; phase = the stick's sideways lean, mix = its fore and aft. The original's choice of row,
  its lean source and the direction of the phase are not ported or measured (MD138a). `ATV_reverse_*`,
  `ATV_turn_still_*`, `ATV_get_ready_idle_01`, `ATV_wreck_air_flail` are bound and never played.
- **The `ATV_Bike_*` clips are read, not played** (MD138a): the quad step starts no stunt since MD123,
  so nothing asks for them; the rider's ATV stunt clips likewise.
- **One build per handling file**: the first player build of the matching chassis; no livery or
  build pick (the Garage has none for the quad).
- **No shadow-casting, blur, particles, or wreck pose** for the quad or its rider; while wrecked the
  rider stays seated on the chassis (the quad's wreck draws nothing of its own).
- **A split's second seat** gets the quad's copy (`quad@seat1`) and its rider by the same code, but no
  two-seat quad ride was run.
- Offline only, as the quad is.

## Next

Land. MD138a has the clips and the original's rider choice.
