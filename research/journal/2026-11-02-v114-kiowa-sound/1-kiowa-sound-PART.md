# 1 — kiowa sound (PART)

Task V114. 2026-11-02.

## What I did

- `crates/openmx/src/audio.rs`: `HeliEngine` reads `otherengine.aud`'s `[Helicopter]` (EngineLoop,
  PitchVariance, Min/MaxSoundDistance, DopplerFactor, using the original's defaults 0/100/300/0.25).
  `HeliEngine::rate` is the original's ranged-loop pitch rule. `HeliNote` is the one looped voice
  (Engine bus, placed on the body, with the bike engine's Doppler shift). `Sounds::helicopter(res)` and
  `Sounds::helicopter_wreck(res)` load them out of `machine.res` (`machine.aud`'s `[helicopterWreck]`
  through the plane's `MachineWreck`).
- `crates/openmx/src/particles.rs`: `KIOWA_PARTICLES` (`helicopterparticles.dat`). `Entry::model` keeps
  `AttachedToCollisionModel` (default 0). `PlaneFrame::models` / `ModelFrame` carry the further models'
  contacts. A `CollisionModel` entry now emits at its own model's contacts only; model 0 is the body's
  as before, so the plane's and the bike's lists behave exactly as they did.
- `crates/openmx/tests/kiowa_particles.rs`: the model index rule, the wash's height fade, and both disc
  copies of the list (free09's and free09_ss's) loaded with nothing skipped.
- Not hooked into `main.rs`: there is no flown Kiowa (P216) to read the collective, the body, the
  spindle node or the wreck from.

## What the original does

The helicopter's sound setup is vtable slot +0x1ec of both helicopter vtables, in code Ghidra did not
make a function. Read it with `./kit dis 0x0038f360 0x0038f690`. It parses `otherengine.aud` and reads
`[Helicopter]`'s EngineLoop (a string), PitchVariance (an *int*, default 0), MaxSoundDistance (300),
MinSoundDistance (100), DopplerFactor (0.25) and RolloffFactor (1). It compares AttenuationCurve
against "Under" and picks a curve function when the two differ. It builds a ranged loop object (vtable
built by `./kit fn 0x001ba6f0`) at heli+0x544. Its setup (`./kit dis 0x001ba740 0x001ba790`) stores
the input range low 0.0 (+0x200), high 1.0 (+0x204), a pointer to the input (+0x208), which is
heli+0x35c, the controller's filtered collective (`game_sim::kiowa::Commands::filtered`: 1 climbing,
½ holding, 0 descending), and the variance (+0x20c). It passes 20.0 and 100.0 on to the 3-D sound's
init, then the body's position and velocity (body+0x60, +0x70), and registers the sound with the world.

Each frame the loop's update (`./kit fn 0x001ba790`) works out `f = (input − low) / high × 2`. For
each sample it sets the rate to `(int)(variance × f) + (own rate − variance)`. So holding plays the
sample at its own rate, and climbing or descending moves it by up to ±PitchVariance Hz (400 on the
disc). The volume is not keyed.

The same setup builds the wreck emitter (+0x548, from `machine.aud`'s `<prefix>Wreck`, fed the body,
the wreck word +0x3d8 and the collision record's touching flag) and the touchdown emitter (+0x54c).
`machine.aud` has `[helicopterWreck]` (5 sounds, MinImpulse 5, MaxImpulse 20, 30..150 ft) and no
`[helicopterTire…]` section, so the touchdown has an empty list. This matches S220b's note that its
pick draws a rand only when the list is not empty.

Particles: `tracks\helicopterparticles.dat` in each freeride track's `_2.res`. A `_ss` track has its
own copy, which comments out the dirt and chunks and uses `helicopterdust_SS.emt`: 25..50 a second
against 150..250. The entries are MainBody Dust/Dirt/Chunks (CollisionModel 0), Blade 1..4 Dust/Chunks
(CollisionModel 1..4), and `[Dust]`, the wash: HelicopterDust on RotorSpindle, Disable Wrecking, fading
from 15 ft to 40 ft. `helicopterdust.emt` matches terrain contour, takes wind, and spreads its points
±25 ft in x and z.

## How it was proved

- `audio::tests::the_helicopter_loop_rides_the_filtered_collective`: the rate rule at 0, ½, 1 and a
  truncating input (0.3337 gives +266, not 266.96). Hand-derived from the disassembly above; no oracle
  run.
- `audio::tests::the_helicopter_section_reads_with_its_defaults`: the disc's section text and a bare
  one.
- `tests/kiowa_particles.rs` (4 tests): the disc's two lists entry by entry (kind, model, node,
  Disable, rate, which collide). A blade's dust comes only off its own model's contacts, never the
  body's, and an object contact switches it off. The wash fades from 15 to 40 ft and is off while
  wrecked.
- `biplane_particles`, `bike_particles` and `wreck_particles` still pass, except
  `a_plane_flown_into_the_ground_puffs_where_it_hits`. That one is red with HEAD's own `particles.rs`
  too (checked by swapping the file back), so it is not this change; it is already filed as F9.

## Not verified / not 1:1

- **Nothing was heard or seen.** The task's by-ear and by-eye match against PCSX2 is not done, because
  the shell cannot fly a Kiowa until P216. That is why the task is blocked.
- The two floats 20.0 and 100.0 the setup passes to the 3-D sound's init are not identified. The shell
  plays the loop at the engine level (`ENGINE_GAIN`) with the file's distances.
- RolloffFactor and AttenuationCurve ("Under") are read by the original but not by the port. The
  mixer has one distance curve.
- Whether a *parked* Kiowa's loop sounds in the original is unknown. The setup sits in a vtable slot
  whose caller I did not trace. The port only means `HeliNote` to run for a flown one; check this in
  PCSX2 beside a parked helicopter when hooking up.
- The Doppler shift is the bike engine's `doppler()` with the helicopter's factor (1.3). The
  original's 3-D sound computes it itself and I did not port that for this loop.
- `PlaneFrame::models` has no producer: `game_sim::kiowa` does not simulate the thrown blades as bodies
  with contacts, so the four blade-dust entries stay off. Before this change they would have emitted at
  the body's contacts. That the original's index means "the vehicle's k-th collision model, the blades
  1..4" is read from the list and S221's four blade parts, not traced in the manager.
- The rate rule is not oracle-tested against `0x001ba790` itself, only hand-ported.

## Next

When P216 lands (V114 reopens then):
1. Load `Sounds::helicopter` and `Sounds::helicopter_wreck` beside the plane's in `main.rs`. Each tick,
   run `HeliNote::update` with `kiowa.commands.filtered.0`, the body's position and velocity, and
   silence it on leaving the Kiowa. Run a `MachineWreckEmitter`-style clank on the Kiowa's wreck word,
   touching flag and shock peak, as `PlaneCrash` does.
2. Load the Kiowa's particles the way `load_plane_fx` loads the plane's, with
   `particles::KIOWA_PARTICLES`, binding `game_render::kiowa::SPINDLE` through `Kiowa::node`. Fill a
   `PlaneFrame` from the flown Kiowa: the spindle node, `wrecked`, the body's velocity, and the body's
   collision record through `PlaneFrame::touch`.
3. Record PCSX2 on C14's free09 state (take-off, hover, descent: `research/journal/*-c14-*`) with Video
   Capture for the audio. Compare the climb and descent pitch and the wash's spread and fade at 15 and
   40 ft by ear and eye.
