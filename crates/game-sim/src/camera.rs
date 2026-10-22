//! The camera for normal play: what it points at, how far behind it lags, and the limits it refuses to
//! go past.
//!
//! The original is one C++ class — a follow camera that hangs off the rider and writes a position, a
//! look-at point and a field of view into the viewport every frame. Everything it does is driven by a
//! *preset*: a 160-byte record the disc ships one of per camera (`Race Cam 1`, `Freestyle Cam`, the
//! look-back, the attract cams), holding the angles, the distance, the filter time constants and the
//! limits. [`Preset`] is that record, [`Follow`] is the camera, and [`Follow::update`] is the frame.
//!
//! # Does it feed the simulation?
//!
//! No, and that was the first thing checked, because the answer decides how exact this module has to
//! be. The original's frame reads the rider (body point, two basis rows, velocity, lean) and writes
//! only its own fields and the viewport's position, look-at and field of view. Nothing it calls — the
//! axis-angle rotation, the camera's collision push-out, the viewport setters — reaches the game's
//! random numbers, and none of it advances the frame clock. So by the house rule for presentation
//! (`AGENT.md`: "Cameras, cutaways, effects … match by eye. Just check they do not touch the
//! simulation's RNG or its timing") this did not have to be bit-exact.
//!
//! It is anyway. Every number here goes through [`ps2_float`], in the original's own operation order,
//! because the camera's arithmetic turned out to be plain FPU and VU0 work that is no harder to port
//! exactly than approximately. [`CurveTable::build`] too: the game builds its sine table with the SDK's
//! *double*-precision `cos` and `sin` and chops the results to `f32`, and [`crate::air`] ports those
//! (S104; the biplane's autopilot reads this table, so it has to be the disc's). [`CurveTable::from_entries`]
//! still lets a proof hand the port the game's own table out of a RAM image.
//!
//! # The frame, in the original's order
//!
//! 1. [`Follow::field_of_view`] — the preset's field of view plus the player's offset, clamped, with
//!    the offset walked back to the clamp so it cannot wind up.
//! 2. The target point: the rider's body point plus [`Preset::offset_from_body_point`] turned into
//!    world space through the rider's own axes.
//! 3. The heading: the rider's forward axis flattened into the horizontal plane and normalised.
//! 4. The swing: how far the camera leans with the rider, from the field-of-view ratio, the preset's
//!    [`Preset::camera_swing`] and the rider's lean.
//! 5. [`Follow::switch_forward_reverse`] — whether the rider is travelling backwards, which ramps
//!    three offsets up or down instead of snapping the camera round.
//! 6. [`Follow::aim`] — yaw and pitch, wrapped and clamped (this is "its limits").
//! 7. The direction: the heading turned by pitch about the sideways axis, then by yaw about up.
//! 8. [`Follow::limit_distance`] — the follow distance against the preset's two limits and the
//!    viewport's own near clip plane, which is what the original's call site hands it (2.4 feet in
//!    play, 1.0 out of the viewport's constructor): a camera is not allowed to sit closer to the
//!    rider than the plane that would clip the rider away. It is *not* a collision distance — the
//!    class's answer to an obstacle is step 12.
//! 9. [`Follow::look_at`] — the look-ahead point (this is "its target"), lifted by the ground probe:
//!    [`Follow::probe_ground`] asks where the surface under a point ahead of the rider is
//!    ([`Follow::ground_probe_point`]) and the look-at climbs out of it, which is what keeps the view
//!    from looking into the inside of a hill.
//! 10. [`Follow::position`] — the target point plus the direction times the distance.
//! 11. [`Follow::filter`] — eight first-order lags, three on the look-at, three on the position, one
//!     on the field of view (this is "its lag").
//! 12. [`Follow::update_probed`]'s last step — the probe: one segment from above the rider out to
//!     where the camera ended up, swept against the scene, and the camera parked short of whatever
//!     it ran into ([`Follow::probe_segment`], [`push_out`]). With step 9 these are the only two
//!     questions the class asks the world, and a camera with nothing to answer them sinks into the
//!     ground and slides through the props.
//!
//! # What this module is not
//!
//! It does not read the camera file off the disc and it does not poll the pad: [`Follow::nudge`]
//! takes the two stick axes and the two modifier buttons as arguments, because this crate does no
//! input. The two probes are here, but the world they ask about is not: [`Surface`] and [`Obstacles`]
//! are the seams, [`Nothing`] is the empty world a fixture rides in, and [`crate::world`] answers
//! both from a track's terrain and collision shapes. The journal entries for S7 and S23 list each of
//! those, and what is known about them.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{self, AcosTable};
use crate::phy::Ini;
use ps2_float::fpu::Acc;
use ps2_float::{Dest, F, Vf, fpu, libm, vu};

/// The original's own literals for the circle, to the bit. `TWO_PI` and `PI` are the values the code
/// holds inline, not the nearest `f32` to the real constants, and `INV_TWO_PI` is the reciprocal it
/// multiplies by to wrap an angle.
const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;
const HALF_PI: f32 = 1.5707964;
const INV_TWO_PI: f32 = 0.15915494;

/// Three quarters of a turn, which the reverse test compares an arc cosine against — see
/// [`Follow::switch_forward_reverse`], where it can never be reached.
const THREE_QUARTER_TURN: f32 = 4.712389;

/// How far the camera may be pitched up or down: 85 degrees, as the original's literal.
const PITCH_LIMIT: f32 = 1.4835324;

/// Degrees to radians, as the camera file's reader multiplies by — [`Preset::angle_y`] and
/// [`Preset::angle_x`] are in radians here because that is what the reader leaves behind.
pub const DEG_TO_RAD: f32 = 0.01745329;

/// World up, and the axis yaw turns about. The original keeps it as a constant vector with `w` of 1.
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

/// The "leave this alone" value the filter setters test against: a filter whose new value or new time
/// constant is `FLT_MAX` keeps the one it had.
const KEEP: f32 = f32::MAX;

// --- the preset ------------------------------------------------------------------------------------

/// One camera preset, field for field as the disc's camera file holds it and the game's reader builds
/// it. Everything the camera does comes from one of these.
///
/// The reader zeroes the record before filling it and leaves any key the file does not mention at
/// zero, which is why [`Default`] is all zeroes rather than anything prettier: a preset with no
/// `ReverseFollowingOffset` really does ramp at zero per second, and the camera has to behave the same
/// way. The two angles are in radians — the reader converts them from the file's degrees by
/// [`DEG_TO_RAD`]; everything else is in the file's own units.
///
/// [`Preset::low_speed_angle_x_vel_cutoff`] and [`Preset::low_speed_angle_x_scale_factor`] are here
/// because the reader reads them, not because anything is known to use them: no reader of either was
/// found in the executable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Preset {
    /// The preset's name, as the file gives it (`Race Cam 1`). Nothing in the camera reads it.
    pub name: String,
    /// Time constants for the three look-at lags, in `x`, `y`, `z`.
    pub look_filters: Vf,
    /// Time constants for the three position lags, in `x`, `y`, `z`.
    pub pos_filters: Vf,
    /// Where the camera aims relative to the rider's body point, in the rider's own axes: `x`
    /// sideways, `y` along the rider's second basis row, `z` along its forward row.
    pub offset_from_body_point: Vf,
    /// The yaw the camera sits at behind the rider, in radians (the file says degrees).
    pub angle_y: F,
    /// The pitch the camera sits at, in radians (the file says degrees).
    pub angle_x: F,
    /// How far behind the target point the camera sits, before the player's zoom and the limits.
    pub following_distance: F,
    /// The field of view this preset asks for, before the player's offset and the limits.
    pub fov: F,
    /// How much pitch the reverse ramp adds once the rider is travelling backwards.
    pub reverse_angle_x_offset: F,
    /// How much distance the reverse ramp adds.
    pub reverse_following_offset: F,
    /// How much look-ahead the reverse ramp replaces the computed one with.
    pub reverse_look_ahead_offset: F,
    /// The *rate* the three reverse offsets ramp at, per frame-time step — despite the name, the
    /// original uses this as a speed of change, not as a threshold.
    pub forward_reverse_switch_speed: F,
    /// Whether the player's camera button cycles through this preset. Nothing in this module reads
    /// it; the front end does.
    pub cycle_thru_this_preset: bool,
    /// Whether the lags run at all. A preset with this clear snaps to its position every frame.
    pub filter_this_preset: bool,
    /// How far ahead of the rider the look-at point runs, as a multiplier on the computed distance.
    pub look_ahead_distance: F,
    /// The look-back camera: it looks behind the rider and skips the swing and the look-ahead.
    pub is_look_back_cam: bool,
    /// How much of the rider's lean swings the camera's yaw.
    pub camera_swing: F,
    /// How much of it swings the look-at point sideways.
    pub look_swing: F,
    /// The closest the camera may sit, also fought over by the collision probe.
    pub min_following_distance: F,
    /// The furthest it may sit.
    pub max_following_distance: F,
    /// The narrowest field of view the player may zoom to.
    pub min_fov: F,
    /// The widest, and the denominator of the field-of-view ratio the swing and the look-ahead use.
    pub max_fov: F,
    /// Read by the file reader; no reader of it was found in the executable.
    pub low_speed_angle_x_vel_cutoff: F,
    /// Read by the file reader; no reader of it was found in the executable.
    pub low_speed_angle_x_scale_factor: F,
}

// --- the lag ---------------------------------------------------------------------------------------

/// One first-order lag, as the original lays it out: the value it is holding, the time constant, and
/// the blend it last used. The game keeps eight of these for the camera and runs them all through one
/// routine.
///
/// The step is `value += min(dt, tau) / tau * (target - value)` — a one-pole filter with its blend
/// clamped so that a frame longer than the time constant lands exactly on the target instead of
/// overshooting it. `tau` of zero divides by zero, as it does in the original; a preset that asks for
/// no filtering clears [`Preset::filter_this_preset`] instead.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Lag {
    /// What the filter is holding, and what it hands back.
    pub value: F,
    /// The time constant, in the same units as `dt`.
    pub tau: F,
    /// The blend the last [`Lag::step`] used. The original stores it in the filter, so it is here.
    pub alpha: F,
}

impl Lag {
    /// A filter with a time constant and nothing in it. `alpha` starts at one, as the game's filter
    /// setters leave it.
    pub fn new(tau: F) -> Lag {
        Lag { value: F::ZERO, tau, alpha: F::ONE }
    }

    /// One frame: blend towards `target` and hand back the new value.
    pub fn step(&mut self, target: F, dt: F) -> F {
        self.alpha = (if dt < self.tau { dt } else { self.tau }) / self.tau;
        self.value = self.value + self.alpha * (target - self.value);
        self.value
    }

    /// Put a value in without filtering — what the game does when a camera snaps.
    ///
    /// [`KEEP`] means "leave it alone", which is how the original's one setter serves both the caller
    /// that has a new value and the caller that only has a new time constant.
    pub fn set_value(&mut self, value: F) {
        if value != KEEP {
            self.value = value;
        }
    }

    /// Give the filter a new time constant, and reset the stored blend to one as the original does.
    pub fn set_tau(&mut self, tau: F) {
        if tau != KEEP {
            self.tau = tau;
            self.alpha = F::ONE;
        }
    }
}

// --- the sine table the rotation reads -------------------------------------------------------------

/// How many entries the engine's startup code builds, and the step and index scale it builds them
/// with: entry `i` is the cosine and sine of `i * STEP - pi`, and a lookup finds its entry by
/// `(angle + pi) * INV_STEP`.
pub const CURVE_ENTRIES: usize = 6000;
const CURVE_STEP: f32 = 0.0010473721;
const CURVE_INV_STEP: f32 = 954.77045;

/// The engine's `{cos, sin}` table: 6000 entries around the circle, with the last duplicated so that
/// an index that rounds up off the end still reads something.
///
/// Every rotation in the game goes through this, including both of the camera's. It is coarse — a
/// fifteenth of a degree per entry — so the lookup does not interpolate between neighbours but
/// corrects with the derivative instead: `cos(a + f) ≈ cos a - f sin a`, `sin(a + f) ≈ sin a +
/// f cos a`, with `f` the leftover angle. That is what makes a 6000-entry table smooth enough to aim
/// a camera with.
///
/// [`CurveTable::build`] is the game's builder to the bit (S104); [`CurveTable::from_entries`] hands a
/// proof the table out of a RAM image instead.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveTable(Box<[[f32; 2]]>);

impl CurveTable {
    /// Build the table the way the startup code does: the index to a float, times the step, minus pi,
    /// and then the cosine and sine of that.
    ///
    /// The game's own builder widens that angle to double, calls the SDK's double-precision `cos` and
    /// `sin` (fdlibm's, [`crate::air`]'s `cos_sin`), and narrows back by chopping, as the arc cosine
    /// table does; so every entry is the disc's to the bit (S104: single-precision ones put two
    /// entries in three a bit off, and the biplane's turn term reads them).
    pub fn build() -> CurveTable {
        let mut t = vec![[0.0f32; 2]; CURVE_ENTRIES + 1];
        for (i, e) in t.iter_mut().take(CURVE_ENTRIES).enumerate() {
            let a = F(fpu::from_i32(i as i32)) * CURVE_STEP - PI;
            let (cos, sin) = air::cos_sin(air::widen(a.0));
            *e = [air::narrow(cos), air::narrow(sin)];
        }
        t[CURVE_ENTRIES] = t[CURVE_ENTRIES - 1];
        CurveTable(t.into_boxed_slice())
    }

    /// The table from entries somebody else built — a save-state, or the game's own builder under the
    /// oracle. The duplicate last entry is added here if it is missing.
    pub fn from_entries(entries: &[[f32; 2]]) -> CurveTable {
        let mut t = entries.to_vec();
        assert!(t.len() >= CURVE_ENTRIES, "a curve table has {CURVE_ENTRIES} entries, got {}", t.len());
        if t.len() == CURVE_ENTRIES {
            t.push(t[CURVE_ENTRIES - 1]);
        }
        CurveTable(t.into_boxed_slice())
    }

    /// The entries, for the test that compares them with the game's.
    pub fn entries(&self) -> &[[f32; 2]] {
        &self.0
    }

    /// The cosine and sine of `angle`, wrapped into `-pi..pi` and corrected off the nearest entry
    /// below it.
    pub fn at(&self, angle: F) -> (F, F) {
        let a = wrap_turn(wrap_turn(angle));
        let s = (F(PI) + a) * CURVE_INV_STEP;
        let i = s.to_i32();
        let leftover = F(CURVE_STEP) * (s - F(fpu::from_i32(i)));
        let (cos, sin) = (F(self.0[i as usize][0]), F(self.0[i as usize][1]));
        (cos - leftover * sin, sin + leftover * cos)
    }

    /// The sine of `angle`, as the engine's own sine helper takes it off the same table: the entry's
    /// sine plus the step times its cosine times what is left over, multiplied in that order (S53).
    pub fn sine(&self, angle: F) -> F {
        let a = wrap_turn(wrap_turn(angle));
        let s = (F(PI) + a) * CURVE_INV_STEP;
        let i = s.to_i32();
        let (cos, sin) = (F(self.0[i as usize][0]), F(self.0[i as usize][1]));
        sin + F(CURVE_STEP) * cos * (s - F(fpu::from_i32(i)))
    }

    /// The cosine and sine of `angle` off the entry below it, with no correction: the lookup the
    /// put-back's ring of probes is fanned out by (S66).
    pub(crate) fn coarse(&self, angle: F) -> (F, F) {
        let a = wrap_turn(wrap_turn(angle));
        let i = ((F(PI) + a) * CURVE_INV_STEP).to_i32();
        (F(self.0[i as usize][0]), F(self.0[i as usize][1]))
    }

    /// The sine of `angle` off the entry below it, with no correction: the lookup the quaternion's
    /// axis and angle divides by.
    pub(crate) fn coarse_sin(&self, angle: F) -> F {
        let a = wrap_turn(wrap_turn(angle));
        let i = ((F(PI) + a) * CURVE_INV_STEP).to_i32();
        F(self.0[i as usize][1])
    }
}

/// `v` turned by `angle` about `axis`, which must be a unit vector: the rotation every part of the
/// game uses, and the one the camera turns its heading by twice.
///
/// It is Rodrigues' formula built on the dot product rather than on a matrix — the component of `v`
/// along the axis stays put, and what is left of `v` is swung in the plane. `w` comes out as 1,
/// because the original writes a literal there.
pub fn rotate(angle: F, axis: Vf, v: Vf, curve: &CurveTable) -> Vf {
    let (cos, sin) = curve.at(angle);
    let (ax, ay, az) = (F(axis.x()), F(axis.y()), F(axis.z()));
    let (vx, vy, vz) = (F(v.x()), F(v.y()), F(v.z()));
    // How much of `v` lies along the axis, and that much of the axis: the part the rotation leaves.
    let along = Acc::mula(ax, vx).madda(ay, vy).madd(az, vz);
    let (lx, ly, lz) = (along * ax, along * ay, along * az);
    // What is left is perpendicular to the axis, and swings with `axis x perp` as its other side.
    let (px, py, pz) = (vx - lx, vy - ly, vz - lz);
    let sx = Acc::mula(pz, ay).msub(py, az);
    let sy = Acc::mula(px, az).msub(pz, ax);
    let sz = Acc::mula(py, ax).msub(px, ay);
    Vf::new(
        (lx + Acc::mula(sin, sx).madd(cos, px)).0,
        (ly + Acc::mula(sin, sy).madd(cos, py)).0,
        (lz + Acc::mula(sin, sz).madd(cos, pz)).0,
        1.0,
    )
}

// --- what the camera reads, and what it writes -----------------------------------------------------

/// The rider the camera follows, as far as the camera can see it: a point, two of its axes, how fast
/// it is going and how far it is leaning. The app layer samples these off the rider's transform.
#[derive(Clone, Copy, Debug, Default)]
pub struct Target {
    /// The point on the rider the camera aims at, before the preset's offset.
    pub body_point: Vf,
    /// The rider's forward axis. Flattened and normalised, this is the camera's heading.
    pub forward: Vf,
    /// The rider's second basis row — up, as far as the offset is concerned. Crossed with `forward`
    /// it gives the sideways axis the offset's `x` runs along.
    pub up: Vf,
    /// The rider's velocity, which drives the look-ahead and the reverse test.
    pub vel: Vf,
    /// How far the rider is leaning, which swings the camera.
    pub lean: F,
}

/// What one frame of the camera hands the renderer. The original writes these three into the viewport
/// and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    /// Where the camera is.
    pub position: Vf,
    /// What it is pointed at.
    pub look_at: Vf,
    /// Its field of view, in the file's own units (degrees).
    pub fov: F,
}

// --- the camera ------------------------------------------------------------------------------------

/// The follow camera. One of these per viewport, holding the presets it may use, which one is
/// current, the eight lags, and the four offsets the player's right stick has nudged.
///
/// Everything here is state the original keeps in the object, named for what it is. The fields are
/// public because a camera is a presentation object that the app layer legitimately reaches into
/// (`./kit shot` comparisons set the preset and the user offsets by hand), and because the proofs in
/// `tests/camera.rs` compare them field by field against the original's object.
#[derive(Clone, Debug)]
pub struct Follow {
    /// The presets this camera may use, in the camera file's order.
    pub presets: Vec<Preset>,
    /// Which one is current.
    pub preset: usize,
    /// Which one was current before, which decides whether a preset change keeps the player's nudges.
    pub previous: usize,
    /// Set when the camera must jump rather than lag on the next frame.
    pub snap: bool,
    /// [`Preset::offset_from_body_point`], copied in when the preset was set.
    pub offset: Vf,
    /// The follow distance after the limits.
    pub distance: F,
    /// Where the camera is pointed, after the player's nudge, the swing and the wrap.
    pub yaw: F,
    /// How far up or down, after the player's nudge, the reverse ramp and the clamp.
    pub pitch: F,
    /// [`Preset::fov`], copied in when the preset was set.
    pub base_fov: F,
    /// [`Preset::camera_swing`], copied in when the preset was set.
    pub camera_swing: F,
    /// [`Preset::look_swing`], copied in when the preset was set.
    pub look_swing: F,
    /// The three lags on the look-at point, in `x`, `y`, `z`.
    pub look_filter: [Lag; 3],
    /// The three lags on the position, in `x`, `y`, `z`.
    pub pos_filter: [Lag; 3],
    /// The lag on the field of view.
    pub fov_filter: Lag,
    /// The eighth lag the original's filter routine takes. This camera hands it a zero every frame
    /// and throws the answer away; it is here so that the routine is the routine.
    pub spare_filter: Lag,
    /// How much the player has zoomed out, added to the follow distance and walked back by the limits.
    pub user_zoom: F,
    /// How far the player has swung the camera round.
    pub user_yaw: F,
    /// How far up or down, walked back by the pitch clamp.
    pub user_pitch: F,
    /// How much field of view the player has added, walked back by the field-of-view clamp.
    pub user_fov: F,
    /// Where the four nudges above are parked while the look-back camera is up, in that order: zoom,
    /// yaw, pitch, field of view. Switching *to* a look-back preset copies them in here and switching
    /// away from one copies them back, so a glance behind leaves the camera where the player had it.
    pub saved_user: [F; 4],
    /// The three reverse offsets, each ramped between zero and the preset's own by
    /// [`Follow::switch_forward_reverse`]: pitch, distance and look-ahead.
    pub reverse_angle_x: F,
    /// See [`Follow::reverse_angle_x`].
    pub reverse_following: F,
    /// See [`Follow::reverse_angle_x`].
    pub reverse_look_ahead: F,
    /// How fast the rider must be going before travelling backwards counts as reverse. The original
    /// takes it as a constructor argument.
    pub reverse_speed: F,
    /// What the look-ahead divides the rider's speed along the heading by. The original's constructor
    /// sets it to one and nothing was found that changes it, so the ratio saturates at one.
    pub speed_reference: F,
    /// How far above the target point the look-at sits. The original's constructor sets it to three.
    pub height_bias: F,
    /// While this is set the player's stick may not pitch the camera *down*, only up. The base class
    /// never sets it; the race camera's floor ([`Race::keep_off_the_ground`]) does, once a frame,
    /// to whether it had to lift the eye — an eye already sitting on the ground may not be pitched
    /// further into it.
    pub pitch_down_locked: bool,
    /// The lift the ground probe answered last frame, which this frame's decay multiplies — see
    /// [`Follow::probe_ground`]. The original keeps it in the *derived* camera object rather than in
    /// this one, because the probe is that class's override and this class's is a stub; it is here
    /// because this is the object the frame runs on.
    pub ground_lift: F,
    /// The least height the race camera's eye keeps above the ground, before the near plane's say —
    /// see [`Race::keep_off_the_ground`]. The derived constructor writes three; the race's setup
    /// writes two over it, which is what every race camera read live holds. Kept here for the same
    /// reason as [`Follow::ground_lift`].
    pub min_height: F,
    /// The highest the race camera's eye may go, in world `y`: [`F::MAX`], or 200 ft on a scene
    /// whose fog reaches past 1500 ft ([`Race::ceiling_for_fog`]).
    pub ceiling: F,
}

impl Follow {
    /// A camera over a list of presets, with the first one applied.
    ///
    /// The original constructs the object and then calls its own set-preset, which is what the second
    /// half of this does: without it the camera would sit at the origin with a zero field of view.
    /// `reverse_speed` is the constructor's one argument.
    pub fn new(presets: Vec<Preset>, reverse_speed: F) -> Follow {
        let mut cam = Follow {
            presets,
            preset: 0,
            previous: 0,
            snap: false,
            offset: Vf::ZERO,
            distance: F::ZERO,
            yaw: F::ZERO,
            pitch: F::ZERO,
            base_fov: F::ZERO,
            camera_swing: F::ZERO,
            look_swing: F::ZERO,
            look_filter: [Lag::new(F::ZERO); 3],
            pos_filter: [Lag::new(F::ZERO); 3],
            fov_filter: Lag::new(F::ZERO),
            spare_filter: Lag::new(F::ZERO),
            user_zoom: F::ZERO,
            user_yaw: F::ZERO,
            user_pitch: F::ZERO,
            user_fov: F::ZERO,
            saved_user: [F::ZERO; 4],
            reverse_angle_x: F::ZERO,
            reverse_following: F::ZERO,
            reverse_look_ahead: F::ZERO,
            reverse_speed,
            speed_reference: F::ONE,
            height_bias: F(3.0),
            pitch_down_locked: false,
            ground_lift: F::ZERO,
            min_height: F(RACE_CTOR_MIN_HEIGHT),
            ceiling: F::MAX,
        };
        if !cam.presets.is_empty() {
            cam.set_preset(0);
        }
        cam
    }

    /// The current preset.
    pub fn preset(&self) -> &Preset {
        &self.presets[self.preset]
    }

    /// Switch to another preset: copy its angles, distance, field of view and swings into the camera,
    /// give the six vector lags their new time constants, and throw away the player's nudges.
    ///
    /// Two details that are easy to miss. The camera only arms its snap when it is moving from an
    /// *unfiltered* preset to a filtered one — coming the other way there is nothing to catch up. And
    /// the player's four nudges survive a trip through the look-back camera: moving *to* a look-back
    /// preset parks them in [`Follow::saved_user`], moving away from one takes them back out, and only
    /// a move between two ordinary presets throws them away.
    pub fn set_preset(&mut self, index: usize) {
        if !self.presets[self.preset].filter_this_preset && self.presets[index].filter_this_preset {
            self.snap = true;
        }
        self.previous = self.preset;
        self.preset = index;
        let p = self.presets[index].clone();
        self.yaw = p.angle_y;
        self.pitch = p.angle_x;
        self.distance = p.following_distance;
        self.base_fov = p.fov;
        self.camera_swing = p.camera_swing;
        self.look_swing = p.look_swing;
        self.offset = p.offset_from_body_point;
        for i in 0..3 {
            self.look_filter[i].set_tau(F(p.look_filters[i]));
            self.pos_filter[i].set_tau(F(p.pos_filters[i]));
        }
        if self.presets[index].is_look_back_cam {
            self.saved_user = [self.user_zoom, self.user_yaw, self.user_pitch, self.user_fov];
        }
        if self.presets[self.previous].is_look_back_cam {
            [self.user_zoom, self.user_yaw, self.user_pitch, self.user_fov] = self.saved_user;
        } else {
            self.clear_nudges();
        }
    }

    /// Throw the player's four nudges away without touching anything else — the third of the small
    /// routines the class keeps beside the two in [`Follow::set_preset`], and the only one of the
    /// three that nothing in the camera itself calls. The front end does, when it puts a camera back
    /// to where its preset says.
    pub fn clear_nudges(&mut self) {
        self.user_zoom = F::ZERO;
        self.user_yaw = F::ZERO;
        self.user_pitch = F::ZERO;
        self.user_fov = F::ZERO;
    }

    /// The camera button: move on to the next preset the file lets the player cycle through
    /// ([`Preset::cycle_thru_this_preset`]), wrapping past the last one to the first, and switch to it
    /// with [`Follow::set_preset`]. A camera with one preset switches to that one.
    ///
    /// The search starts one past the current preset and can come all the way round to it, so a
    /// camera whose only cycling preset is the current one switches to itself — which still throws
    /// the nudges away, as the original's does. A camera with no cycling preset at all spins forever
    /// in the original; here it stays where it is.
    pub fn next_preset(&mut self) {
        let count = self.presets.len();
        if count == 1 {
            self.set_preset(0);
            return;
        }
        let Some(to) = (1..=count)
            .map(|step| (self.preset + step) % count)
            .find(|&i| self.presets[i].cycle_thru_this_preset)
        else {
            return;
        };
        self.set_preset(to);
    }

    /// The same walk the other way: the previous cycling preset, wrapping past the first to the
    /// last. Unlike [`Follow::next_preset`] a camera with one preset gets no special case, and the
    /// walk comes round to the current preset the same way.
    pub fn previous_preset(&mut self) {
        let count = self.presets.len();
        let Some(to) = (1..=count)
            .map(|step| (self.preset + count - step) % count)
            .find(|&i| self.presets[i].cycle_thru_this_preset)
        else {
            return;
        };
        self.set_preset(to);
    }

    /// The player's right stick, one frame of it: `x` and `y` already mapped to −1..1 by the input
    /// layer, and the two modifier controls the game binds beside them.
    ///
    /// With a modifier held the stick's `y` zooms or widens instead of looking around, and with
    /// neither it swings the camera. Pitching *down* is refused while [`Follow::pitch_down_locked`]
    /// is set. Both angles wrap into `-pi..pi`.
    pub fn nudge(&mut self, stick_x: F, stick_y: F, hold_zoom: bool, hold_fov: bool) {
        if hold_zoom || hold_fov {
            if hold_zoom {
                self.user_zoom = self.user_zoom + stick_y * 0.3;
            }
            if hold_fov {
                self.user_fov = self.user_fov + stick_y * 0.5;
            }
            return;
        }
        self.user_yaw = wrap_half_turn(self.user_yaw - stick_x * 0.05);
        let down = stick_y * -0.05;
        if down < 0.0 {
            if !self.pitch_down_locked {
                self.user_pitch = self.user_pitch + down;
            }
        } else {
            self.user_pitch = self.user_pitch + down;
        }
        self.user_pitch = wrap_half_turn(self.user_pitch);
    }

    /// The field of view this frame: the preset's plus the player's offset, held between the preset's
    /// two limits.
    ///
    /// When it hits a limit the player's offset is rewritten to exactly the offset that reaches it,
    /// so that holding the stick against the stop does not build up an offset that has to be unwound
    /// before anything moves. [`Follow::limit_distance`] does the same for the zoom.
    pub fn field_of_view(&mut self) -> F {
        let fov = self.base_fov + self.user_fov;
        let (min, max) = (self.preset().min_fov, self.preset().max_fov);
        // Written as the original's two tests are, so that a NaN takes the same branch: the first is
        // "not <=", not ">".
        if !(fov <= max) {
            self.user_fov = max - self.base_fov;
            max
        } else if fov < min {
            self.user_fov = min - self.base_fov;
            min
        } else {
            fov
        }
    }

    /// Is the rider travelling backwards, and ramp the three reverse offsets accordingly.
    ///
    /// The test is the angle between the heading and the direction the rider is actually moving,
    /// through the engine's shared arc cosine table — the same table the air code reads the crossup
    /// angle out of. Past a quarter turn, and above [`Follow::reverse_speed`], the camera is in
    /// reverse: its pitch, distance and look-ahead offsets ramp up towards the preset's three, at
    /// [`Preset::forward_reverse_switch_speed`] per step for the distance and a twentieth of that for
    /// the other two. Otherwise they ramp back down to zero, and the ramp down is skipped entirely
    /// once the distance offset has reached zero.
    ///
    /// The ramps are not symmetric: the look-ahead offset ramps *up* at a twentieth of the rate and
    /// back *down* at the full rate, so it unwinds twenty times faster than it wound on. That is how
    /// the original is written.
    ///
    /// The upper bound of the "is it reverse" window is three quarters of a turn, which an arc cosine
    /// can never return: the original writes the test as if the angle were signed, and the half of it
    /// that cannot fire is left here as it is written.
    pub fn switch_forward_reverse(&mut self, speed: F, heading: Vf, vel: Vf, acos: &AcosTable) -> bool {
        let going = normalize(vel);
        let dot = Acc::mula(F(heading.x()), F(going.x()))
            .madda(F(heading.y()), F(going.y()))
            .madd(F(heading.z()), F(going.z()));
        // The original clamps the same way twice over, which costs nothing and is left in.
        let dot = clamp_unit(clamp_unit(dot));
        let angle = acos.at(dot);
        let rate = self.preset().forward_reverse_switch_speed;
        if angle <= HALF_PI || angle >= THREE_QUARTER_TURN || speed <= self.reverse_speed {
            if self.reverse_following != 0.0 {
                self.reverse_angle_x = F::ZERO.max(self.reverse_angle_x - rate * 0.05);
                self.reverse_following = F::ZERO.max(self.reverse_following - rate);
                self.reverse_look_ahead = F::ZERO.max(self.reverse_look_ahead - rate);
            }
            false
        } else {
            let p = self.preset().clone();
            self.reverse_angle_x =
                p.reverse_angle_x_offset.min(self.reverse_angle_x + rate * 0.05);
            self.reverse_following = p.reverse_following_offset.min(self.reverse_following + rate);
            self.reverse_look_ahead =
                p.reverse_look_ahead_offset.min(self.reverse_look_ahead + rate * 0.05);
            true
        }
    }

    /// Yaw and pitch for this frame, with their limits. `swing` is the lean-driven swing the frame
    /// computed.
    ///
    /// Yaw is the preset's angle less the player's, less the swing, wrapped into `-pi..pi` through the
    /// library's `fmod` and then one correction — the wrap the original's own code does.
    ///
    /// Pitch is the preset's plus the player's, less whatever the reverse ramp has added, and clamped
    /// to [`PITCH_LIMIT`] either way. On a clamp the player's pitch is rewritten to exactly the offset
    /// that reaches the limit, as the field of view's is. The two rewrites use the reverse ramp with
    /// *opposite* signs, which cannot both be right — it is written here the way the original writes
    /// it, and only the upper one is reachable while the ramp is a ramp up.
    pub fn aim(&mut self, swing: F) {
        let (angle_y, angle_x) = (self.preset().angle_y, self.preset().angle_x);
        self.yaw = angle_y - self.user_yaw;
        self.yaw = self.yaw - self.camera_swing * swing;
        self.yaw = F(libm::fmodf(self.yaw.0, TWO_PI));
        if self.yaw > PI {
            self.yaw = self.yaw - TWO_PI;
        } else if self.yaw < -PI {
            self.yaw = self.yaw + TWO_PI;
        }
        self.pitch = self.user_pitch + angle_x;
        self.pitch = self.pitch - self.reverse_angle_x;
        if self.pitch >= -PITCH_LIMIT {
            if self.pitch > PITCH_LIMIT {
                self.pitch = F(PITCH_LIMIT);
                self.user_pitch = (F(PITCH_LIMIT) - angle_x) - self.reverse_angle_x;
            }
        } else {
            self.pitch = F(-PITCH_LIMIT);
            self.user_pitch = self.reverse_angle_x + (F(-PITCH_LIMIT) - angle_x);
        }
    }

    /// The follow distance and its limits. `near_clip` is the viewport's near clip distance — the
    /// field the original's call site reads for this argument, which the engine's own near/far setter
    /// writes along with the projection's depth terms — and it fights the preset's own minimum,
    /// whichever is further out winning. S7 read it as a collision distance; it is not. The class's
    /// answer to an obstacle is the probe at the end of [`Follow::update_probed`], and the floor here
    /// only keeps the camera out of its own near plane. This frame reads the field raw, and the
    /// viewport the disc sets up for a race puts 2.4 feet in it. The camera class the disc actually
    /// creates has a frame of its own which multiplies the same field by four and a half first,
    /// giving it a floor of ten and four fifths of a foot instead; which of the two beats a preset's
    /// own `MinFollowingDistance` is a question about the track's `cpresets.ini` that this port has
    /// not read those numbers to answer. Porting that frame is its own task. A caller hands over its
    /// own viewport's near clip.
    ///
    /// On either limit the player's zoom is rewritten to the offset that reaches it, as in
    /// [`Follow::field_of_view`].
    pub fn limit_distance(&mut self, near_clip: F) {
        let p = self.preset().clone();
        let floor = p.min_following_distance.max(near_clip);
        let want = self.reverse_following + (self.user_zoom + p.following_distance);
        self.distance = want;
        if want < floor {
            self.distance = floor;
            self.user_zoom = self.distance - p.following_distance;
        } else if p.max_following_distance < want {
            self.distance = p.max_following_distance;
            self.user_zoom = self.distance - p.following_distance;
        }
    }

    /// Where the camera looks: the target point, pushed ahead of the rider by however far the
    /// look-ahead reaches. This is the camera's "target".
    ///
    /// The look-ahead distance is the rider's speed *along the heading* (so braking and sliding
    /// shorten it), capped at [`Follow::speed_reference`], scaled by a factor that grows with the
    /// follow distance and the field of view and shrinks as the camera is pulled further out, and then
    /// by the preset's [`Preset::look_ahead_distance`]. A rider falling fast shortens it further; a
    /// rider launching upwards shortens it too. In reverse the whole computation is thrown away and
    /// the ramped reverse look-ahead used instead.
    ///
    /// The point itself is pushed along the heading *swung* by the rider's lean, not along the
    /// heading, which is what makes the camera lead into a turn. Its height is lifted by the rider's
    /// vertical speed — more at a narrow field of view — then by whatever the ground probe answers
    /// ([`Follow::probe_ground`], asked about [`Follow::ground_probe_point`]), and then by
    /// [`Follow::height_bias`], in that order: the probe's lift goes in *before* the bias, which is
    /// the one place the base class's two adds are not interchangeable with the derived class's.
    ///
    /// The look-back cam skips all of it and looks straight back down the heading — and skips the
    /// probe with it, so a glance behind neither asks the ground anything nor decays the lift.
    #[allow(clippy::too_many_arguments)]
    pub fn look_at(
        &mut self,
        speed: F,
        fov_ratio: F,
        swing: F,
        target_point: Vf,
        heading: Vf,
        vel: Vf,
        reverse: bool,
        curve: &CurveTable,
        scene: &mut impl Surface,
    ) -> Vf {
        let along = Acc::mula(F(vel.x()), F(heading.x()))
            .madda(F(vel.y()), F(heading.y()))
            .madd(F(vel.z()), F(heading.z()))
            / self.speed_reference;
        let pull = (self.distance * fov_ratio) / 90.0 + 1.0;
        let mut ahead =
            F::ONE.min(along) * (pull * ((F(1.8) * (F::ONE - self.distance / 600.0)) * pull));
        if speed != 0.0 {
            let rise = F(vel.y());
            if rise < -5.0 {
                ahead = ahead * (rise / speed + 1.0);
            } else if rise > 5.0 {
                ahead = ahead * (F::ONE - rise / speed);
            }
        }
        let p = self.preset();
        let ahead = if reverse { self.reverse_look_ahead } else { ahead * p.look_ahead_distance };
        if p.is_look_back_cam {
            let back = Vf::new(-heading.x(), -heading.y(), -heading.z(), heading.w());
            return add3(target_point, scale(back, p.look_ahead_distance));
        }
        let swung = normalize(rotate(swing * (F(-0.7) * self.look_swing), UP, heading, curve));
        let point = add3(target_point, scale(swung, ahead));
        let rise = F(point.y()) + (F::ONE - fov_ratio) * F(vel.y()) * 0.25;
        let ground = self.probe_ground(self.ground_probe_point(swung, target_point), scene);
        let lift = (rise + ground) + self.height_bias;
        Vf::new(point.x(), lift.0, point.z(), point.w())
    }

    /// Where the ground probe looks: the target point pushed along the swung heading — the same
    /// vector the look-ahead point rides, normalised once and used twice — by a reach that grows with
    /// how far the camera is pitched. Three tenths of a foot looking level, a foot and a half at the
    /// pitch limit, and it does not care which way the pitch goes.
    ///
    /// Asking about a point *ahead of the rider* rather than about the camera is what makes this a
    /// look-at lift and not a push-out: the view stays pointed at ground it can see over.
    pub fn ground_probe_point(&self, swung: Vf, target_point: Vf) -> Vf {
        let reach = (self.pitch.abs() / F(HALF_PI)) * 1.2 + 0.3;
        add3(scale(swung, reach), target_point)
    }

    /// The **ground probe**: how far the look-at point is lifted so the view does not sink into the
    /// dirt. Vtable slot `+0x10c`, which S7 found a `return 0.0f`.
    ///
    /// That stub is the base class's answer, and it is why the port's look-at used to add a literal
    /// zero. The class the game actually creates for a race overrides the slot with this: drop the
    /// point onto the surface under it through the world's own ground query
    /// ([`Surface::on_surface`]), and then
    ///
    /// * if the ground came out *above* the point, the lift is six tenths of how far above — so the
    ///   look-at climbs most of the way out of a hillside in one frame and the rest over the next
    ///   few, rather than popping;
    /// * otherwise the lift is last frame's times a fiftieth — a fast decay, not a snap back to
    ///   nothing, so coming out of a dip does not jerk the view down.
    ///
    /// The decay is the whole reason this needs `&mut self`: it reads [`Follow::ground_lift`]. A point
    /// with no ground under it comes back unmoved and lands in the decay arm, which is also what a
    /// camera over [`Nothing`] gets every frame — and `0.0 * 0.02` is the zero the stub returned, so
    /// a camera with no scene is bit-for-bit the camera S7 proved.
    pub fn probe_ground(&mut self, point: Vf, scene: &mut impl Surface) -> F {
        let on_surface = scene.on_surface(point);
        self.ground_lift = if F(point.y()) < F(on_surface.y()) {
            (F(on_surface.y()) - F(point.y())) * 0.6
        } else {
            self.ground_lift * 0.02
        };
        self.ground_lift
    }

    /// Where the camera is: the target point plus the direction it is pointed times the follow
    /// distance.
    ///
    /// That is the whole of it. The original's routine also computes a sideways swing from the yaw,
    /// over two arcs of three eighths of a turn either side of straight ahead, and stores it into a
    /// stack slot that nothing ever reads — dead code, kept out of here, recorded in the journal, and
    /// shown to be dead by a proof that varies the yaw and watches the position not move.
    pub fn position(&self, direction: Vf, target_point: Vf) -> Vf {
        add3(target_point, scale(direction, self.distance))
    }

    /// The lag: eight first-order filters, run in the original's order — the look-at's three, the
    /// position's three, the field of view, and the spare. This is the camera's "lag".
    ///
    /// A preset with [`Preset::filter_this_preset`] clear does not filter at all, and a camera with
    /// [`Follow::snap`] set does not either: instead it loads the filters with this frame's values, so
    /// that the frame after lags from where the camera actually is. Both vectors come back with `w` of
    /// one, because the original rebuilds them a lane at a time and puts a literal there.
    ///
    /// `fov` and `spare` are filtered in place. The original reaches them through the same setter that
    /// takes [`KEEP`] for "leave it", which is why a snap can skip them.
    pub fn filter(&mut self, dt: F, position: &mut Vf, look_at: &mut Vf, fov: &mut F, spare: &mut F) {
        if !self.preset().filter_this_preset || self.snap {
            if self.snap {
                for i in 0..3 {
                    self.look_filter[i].set_value(F(look_at[i]));
                    self.pos_filter[i].set_value(F(position[i]));
                }
                self.fov_filter.set_value(*fov);
                self.spare_filter.set_value(*spare);
                self.snap = false;
            }
            return;
        }
        for i in 0..3 {
            look_at[i] = self.look_filter[i].step(F(look_at[i]), dt).0;
        }
        for i in 0..3 {
            position[i] = self.pos_filter[i].step(F(position[i]), dt).0;
        }
        look_at[3] = 1.0;
        position[3] = 1.0;
        *fov = self.fov_filter.step(*fov, dt);
        *spare = self.spare_filter.step(*spare, dt);
    }

    /// One frame of the camera in an empty world: [`Follow::update_probed`] over [`Nothing`], which
    /// has no ground to lift the view off and nothing solid to push it out of. The second half of
    /// that is the original's own behaviour when the camera's collision object has never been made —
    /// the frame tests the pointer and skips the probe when it is null — and the first half is the
    /// base class's ground probe, which answers zero for every point.
    pub fn update(
        &mut self,
        dt: F,
        target: &Target,
        near_clip: F,
        curve: &CurveTable,
        acos: &AcosTable,
    ) -> View {
        self.update_probed(dt, target, near_clip, curve, acos, &mut Nothing)
    }

    /// One frame of the camera: everything above, in the original's order, ending with the probe.
    ///
    /// `near_clip` is the viewport's near clip distance, which bounds the follow distance (see
    /// [`Follow::limit_distance`]); `scene` answers both of the camera's questions about the world —
    /// the ground under the look-at point ([`Surface`]) and the nearest thing in front of the lens
    /// ([`Obstacles`]). The returned [`View`] is exactly what the original writes into the viewport.
    #[allow(clippy::too_many_arguments)]
    pub fn update_probed(
        &mut self,
        dt: F,
        target: &Target,
        near_clip: F,
        curve: &CurveTable,
        acos: &AcosTable,
        scene: &mut (impl Obstacles + Surface),
    ) -> View {
        let mut fov = self.field_of_view();

        // The target point: the offset turned out of the rider's axes and added to its body point.
        // The sideways term is skipped when it is zero, as the original skips it.
        let mut offset = add3(
            scale(target.forward, F(self.offset.z())),
            scale(target.up, F(self.offset.y())),
        );
        if self.offset.x() != 0.0 {
            let sideways = cross(target.up, target.forward);
            offset = add3(offset, scale(sideways, F(self.offset.x())));
        }
        let target_point = add3(target.body_point, offset);

        // The heading: forward, flattened and normalised. `y` is zeroed before the normalise and `w`
        // comes through from the rider's own row.
        let heading = normalize(Vf::new(target.forward.x(), 0.0, target.forward.z(), target.forward.w()));

        let fov_ratio = fov / self.preset().max_fov;
        let swing = fov_ratio * ((F(0.21) * self.camera_swing) * -target.lean);
        let speed = Acc::mula(F(target.vel.x()), F(target.vel.x()))
            .madda(F(target.vel.y()), F(target.vel.y()))
            .madd(F(target.vel.z()), F(target.vel.z()))
            .sqrt();

        let reverse = self.switch_forward_reverse(speed, heading, target.vel, acos);
        self.aim(swing);

        // Pitch about the sideways axis first, then yaw about up.
        let sideways = cross(heading, UP);
        let direction = rotate(self.yaw, UP, rotate(self.pitch, sideways, heading, curve), curve);

        self.limit_distance(near_clip);

        let mut look_at = self.look_at(
            speed,
            fov_ratio,
            swing,
            target_point,
            heading,
            target.vel,
            reverse,
            curve,
            scene,
        );
        let mut position = self.position(direction, target_point);
        let mut spare = F::ZERO;
        self.filter(dt, &mut position, &mut look_at, &mut fov, &mut spare);

        // The probe, after the filters and after everything else: a camera that has just been lagged
        // into a hillside is pulled back out of it here, and the lags are *not* told, so the next
        // frame's filtering starts from the position the camera would have had in the open. That is
        // the original's order, and it is why the push-out does not feel like the camera following
        // the wall.
        let (from, to) = self.probe_segment(target_point, target.up, position);
        if let Some(hit) = scene.nearest(from, to) {
            position = push_out(from, to, hit.point);
        }
        View { position, look_at, fov }
    }

    /// The segment the probe sweeps, as the original builds it: from the frame's target point lifted
    /// [`PROBE_CLEARANCE`] along the *rider's* own up axis — not the world's — out to where the
    /// camera ended up this frame.
    ///
    /// Starting above the target point rather than at it is what keeps the probe from hitting the
    /// ground the bike is standing on: the segment leaves the rider's head, not the rider's wheels.
    pub fn probe_segment(&self, target_point: Vf, up: Vf, position: Vf) -> (Vf, Vf) {
        (add3(target_point, scale(up, F(PROBE_CLEARANCE))), position)
    }
}

// --- the camera's own collision probe --------------------------------------------------------------

/// How far above the target point the probe starts, and how far short of what it hit the camera is
/// parked. One number in the original — the same register holds it for both ends of the probe.
pub const PROBE_CLEARANCE: f32 = 2.0;

/// What the camera's probe ran into: the point on the obstacle and the surface normal there, both in
/// world space.
///
/// The original gets these back from its collision object through a callback it installs on the
/// object — the callback copies the probe's hit point and the record's nearest normal into the camera
/// and sets a flag the frame reads one line later. Nothing in the class reads either vector after
/// that frame, so here they come straight back from [`Obstacles::nearest`] instead of through three
/// fields of the camera. The normal is carried because the original carries it, and for the same
/// reason: nothing in this class uses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    pub point: Vf,
    pub normal: Vf,
}

/// The world, as the camera asks about it: one segment, and the nearest thing along it.
///
/// This is the whole of the camera's view of the scene. The original's object is a one-probe bundle of
/// the same collision class the bike sweeps with, run through the broad phase against everything
/// solid, and its answer is the nearest hit — so a port only has to answer the segment.
pub trait Obstacles {
    /// The nearest hit on the segment `start -> end`, or `None` for a clear line. A hit the segment
    /// leaves from the *inside* is not a hit: the original's triangle test culls back faces, so a
    /// probe that starts under the ground or inside a prop comes out of it unobstructed.
    fn nearest(&mut self, start: Vf, end: Vf) -> Option<Obstacle>;
}

/// Nothing to run into and nothing to stand on. What [`Follow::update`] probes: the first half is
/// the original's own frame when the camera's collision object has never been created — the pointer
/// is null and the whole step is skipped — and the second is the base class's ground probe, a
/// `return 0.0f`, which a point that never moves reproduces exactly.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nothing;

impl Obstacles for Nothing {
    fn nearest(&mut self, _start: Vf, _end: Vf) -> Option<Obstacle> {
        None
    }
}

impl Surface for Nothing {
    fn on_surface(&mut self, point: Vf) -> Vf {
        point
    }

    fn has_ground(&self) -> bool {
        false
    }
}

// --- what the camera asks the ground ---------------------------------------------------------------

/// The ground, as the camera asks about it: one point, moved onto the surface under it.
///
/// This is a *different* question from [`Obstacles`], asked down a different route in the original:
/// the obstacle probe goes through a collision object the camera owns, and this goes straight to the
/// world's own ground query — the same routine the bike's ground step and eighty other callers use,
/// which takes a point in place and a normal, a flags word and a record the camera all pass as null.
/// So it is two traits here, because it is two questions there.
pub trait Surface {
    /// `point` with its height replaced by the height of the surface under it, in world space.
    ///
    /// A point with nothing under it comes back unchanged, which is what the original's query does
    /// when its own search finds no triangle: the point it was handed is the point it leaves. `x`,
    /// `z` and `w` come back untouched either way — the query moves a point *down* onto the ground,
    /// it does not move it sideways.
    fn on_surface(&mut self, point: Vf) -> Vf;

    /// Is there a world to ask at all. The race camera's floor tests its world pointer before it
    /// asks, and does nothing without one ([`Race::keep_off_the_ground`]); every real scene has one.
    fn has_ground(&self) -> bool {
        true
    }
}

/// Where the camera goes when the probe hit something: [`PROBE_CLEARANCE`] short of the hit, back
/// along the probe, so the obstacle stays behind the camera instead of in front of the lens.
///
/// The step back is along the *probe's* direction and not along the surface normal, which is what
/// makes this a pull-in towards the rider rather than a slide along the wall. `w` comes through from
/// the hit point, as the original's `vsub.xyz` leaves it.
///
/// The direction is normalised through the EE FPU — the dot product in the accumulator and then
/// `rsqrt.s` — and not through VU0's approximate reciprocal square root, which is what the rest of
/// this module normalises with. A segment of no length at all answers the zero point, which the
/// original reaches by loading a constant quadword, and the camera is left exactly on the hit.
pub fn push_out(start: Vf, end: Vf, hit: Vf) -> Vf {
    let delta = end.merge(Dest::XYZ, end.sub(start));
    let len2 = Acc::mula(F(delta.x()), F(delta.x()))
        .madda(F(delta.y()), F(delta.y()))
        .madd(F(delta.z()), F(delta.z()));
    let along = if len2 == F::ZERO {
        Vf::new(0.0, 0.0, 0.0, 1.0)
    } else {
        let inv = F::ONE.rsqrt(len2);
        Vf::new(
            (F(delta.x()) * inv).0,
            (F(delta.y()) * inv).0,
            (F(delta.z()) * inv).0,
            delta.w(),
        )
    };
    let back = scale(along, F(PROBE_CLEARANCE));
    hit.merge(Dest::XYZ, hit.sub(back))
}

// --- the small shapes the original inlines everywhere -----------------------------------------------

/// An angle wrapped into `-pi..pi` the way the rotation does it: subtract whole turns by truncating
/// `angle / 2pi`, then one correction for the half-open end. The rotation does this twice in a row,
/// because the first pass can leave an angle a hair outside the range.
fn wrap_turn(angle: F) -> F {
    let turns = F(fpu::from_i32((angle * INV_TWO_PI).to_i32()));
    let a = angle - F(TWO_PI) * turns;
    wrap_half_turn(a)
}

/// One correction into `-pi..pi`, as the camera's own wraps do it: no `fmod`, just the one subtraction
/// or addition, because the angle is already within a turn.
fn wrap_half_turn(a: F) -> F {
    if a > PI {
        a - TWO_PI
    } else if a < -PI {
        a + TWO_PI
    } else {
        a
    }
}

/// `x` held inside −1..1, written as the original's pair of selects: the high test first, the low test
/// only if the high one passed.
fn clamp_unit(x: F) -> F {
    if x <= 1.0 {
        if x >= -1.0 { x } else { F(-1.0) }
    } else {
        F::ONE
    }
}

/// `v`'s `xyz` scaled by `s`, with `w` left as it was: every one of the original's scales is a
/// `vmul` against a broadcast lane with an `xyz` destination, so `w` comes through unscaled.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul_q(s.0))
}

/// `a + b` over `xyz`, with `w` the first operand's — the original's adds are all `vadd.xyz`.
fn add3(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// Normalise `xyz` through VU0's *approximate* reciprocal square root, which is what this code uses
/// and which is not the same number as a square root and a divide. `w` comes through untouched.
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, len2)))
}

/// `a x b`, as the original's `vopmula`/`vopmsub` pair with its operands swapped between the two
/// instructions. `w` is the first operand's, and nothing here reads it.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

// --- the camera file -------------------------------------------------------------------------------

impl Preset {
    /// Read a camera file (`cpresets.ini`) the way the game's loader reads one.
    ///
    /// The count comes from `[NumberOfPresets]`'s `NumberOfPresets` key — the section and the key have
    /// the same name — and a count below one is a failed load, which comes back here as no presets at
    /// all. Each preset `i` is the section `Preset{i+1}` with `suffix` after the number: the loader
    /// builds the name with the viewport's letter, `""` for a full-screen view and `h`, `v` or `q` for
    /// the split-screen ones (`w`, `x`, `z` when the game's own layout global is 3). A section the file
    /// does not have leaves that preset **entirely zero**, because the loader zeroes the records first
    /// and then skips the missing one; so does a key the section does not have.
    ///
    /// The two angles are the only keys the loader scales, and it scales them only when the key was
    /// found: `AngleY = 0` and a missing `AngleY` both leave zero here, but a preset read twice over a
    /// file that mentions one and not the other does not drift.
    pub fn read(ini: &Ini, suffix: &str) -> Vec<Preset> {
        let count = ini.section("NumberOfPresets").int("NumberOfPresets").unwrap_or(0);
        if count < 1 {
            return Vec::new();
        }
        (0..count).map(|i| Preset::one(ini, &format!("Preset{}{}", i + 1, suffix))).collect()
    }

    /// The same, from a file's bytes.
    pub fn read_bytes(bytes: &[u8], suffix: &str) -> Vec<Preset> {
        Preset::read(&Ini::parse_bytes(bytes), suffix)
    }

    /// Which of `presets` carries `name` (`Freestyle Cam`), ignoring case — the first one, if the
    /// file names two the same. The game never asks this: it picks its presets by index. It is for
    /// a front end that wants a camera by what the file calls it, since nothing promises that every
    /// track's file puts the same camera at the same index.
    pub fn find(presets: &[Preset], name: &str) -> Option<usize> {
        presets.iter().position(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// One preset out of one section, in the loader's own key order. The order matters only for a
    /// file with a key twice, which the first-match scan settles anyway; it is kept because it is the
    /// record of what the loader reads.
    fn one(ini: &Ini, name: &str) -> Preset {
        let mut p = Preset::default();
        if !ini.names().iter().any(|n| n.eq_ignore_ascii_case(name)) {
            return p;
        }
        let s = ini.section(name);
        // `Name`'s default is the section's own name, so an unnamed preset is called `Preset3`.
        p.name = s.text("Name").unwrap_or(name).to_string();
        // Degrees in the file, radians here, and only when the key was there.
        if let Some(v) = s.float("AngleY") {
            p.angle_y = v * F(DEG_TO_RAD);
        }
        if let Some(v) = s.float("AngleX") {
            p.angle_x = v * F(DEG_TO_RAD);
        }
        p.following_distance = s.float_or("FollowingDistance", p.following_distance);
        p.min_following_distance = s.float_or("MinFollowingDistance", p.min_following_distance);
        p.max_following_distance = s.float_or("MaxFollowingDistance", p.max_following_distance);
        p.fov = s.float_or("FOV", p.fov);
        p.min_fov = s.float_or("MinFOV", p.min_fov);
        p.max_fov = s.float_or("MaxFOV", p.max_fov);
        p.look_ahead_distance = s.float_or("LookAheadDistance", p.look_ahead_distance);
        p.cycle_thru_this_preset = s.flag("CycleThruThisPreset", p.cycle_thru_this_preset);
        p.filter_this_preset = s.flag("FilterThisPreset", p.filter_this_preset);
        p.is_look_back_cam = s.flag("IsLookBackCam", p.is_look_back_cam);
        p.camera_swing = s.float_or("CameraSwing", p.camera_swing);
        p.look_swing = s.float_or("LookSwing", p.look_swing);
        p.look_filters = s.vector("LookFilters", p.look_filters);
        p.pos_filters = s.vector("PosFilters", p.pos_filters);
        p.reverse_angle_x_offset = s.float_or("ReverseAngleXOffset", p.reverse_angle_x_offset);
        p.reverse_following_offset = s.float_or("ReverseFollowingOffset", p.reverse_following_offset);
        p.reverse_look_ahead_offset =
            s.float_or("ReverseLookAheadOffset", p.reverse_look_ahead_offset);
        p.forward_reverse_switch_speed =
            s.float_or("ForwardReverseSwitchSpeed", p.forward_reverse_switch_speed);
        p.low_speed_angle_x_vel_cutoff =
            s.float_or("LowSpeedAngleXVelCutoff", p.low_speed_angle_x_vel_cutoff);
        p.low_speed_angle_x_scale_factor =
            s.float_or("LowSpeedAngleXScaleFactor", p.low_speed_angle_x_scale_factor);
        p.offset_from_body_point = s.vector("OffsetFromBodyPoint", p.offset_from_body_point);
        p
    }
}

// --- the camera the disc actually creates ----------------------------------------------------------

/// How much of the viewport's near clip distance floors the race camera's follow distance. The base
/// class reads the field raw; this frame multiplies it by four and a half first, so the race
/// viewport's 2.4 feet becomes a floor of ten and four fifths.
pub const NEAR_CLIP_FLOOR: f32 = 4.5;

/// How many near planes the race camera's eye keeps above the ground at the least: the floor is
/// this many, or [`Follow::min_height`] if that is more. At the race viewport's 2.4 ft near plane the
/// near planes win, so the eye never sits lower than 4.8 ft over the ground under it.
pub const NEAR_CLIP_HEIGHT: f32 = 2.0;

/// [`Follow::min_height`] as the derived constructor writes it.
const RACE_CTOR_MIN_HEIGHT: f32 = 3.0;

/// [`Follow::min_height`] as the race's setup writes it over the constructor's, for every race
/// camera it builds — the bike's and the plane's.
pub const RACE_MIN_HEIGHT: f32 = 2.0;

/// The race's setup caps the eye at [`RACE_CEILING`] only on a scene whose fog record's far
/// distance (`[Fog]`'s `MaximumFogDistance`) is more than this.
const CEILING_FOG: f32 = 1500.0;

/// The highest the eye may go, in world `y`, on a scene with fog past [`CEILING_FOG`].
pub const RACE_CEILING: f32 = 200.0;

/// Above this, and only while the rider is in the air, the heading comes off the velocity instead of
/// the rider's forward axis — so a flip or a whip in the air, which turns the bike's forward row
/// over, does not turn the camera with it.  (See [`Ridden::airborne`]: on the ground the heading is
/// the forward row at any speed.)
const HEADING_FROM_VEL_SPEED: f32 = 20.0;

/// Above this, and only while airborne, the *direction basis* comes off the velocity too. Lower than
/// the heading's cutoff: the two vectors disagree over the band between them.
const BASIS_FROM_VEL_SPEED: f32 = 5.0;

/// How close the speed has to be to the rider's bare vertical speed before the velocity is refused as
/// a direction basis. A rider moving straight up or straight down has no horizontal direction to
/// flatten, and the flattened vector would be noise.
const BASIS_DEGENERATE: f32 = 0.01;

/// Above this the frame checks whether the rider is actually travelling backwards, by the sign of the
/// velocity along the heading. This frame's answer to reverse: throw the velocity away. (The base
/// class instead measures the angle through the arc cosine table and ramps three offsets; none of
/// that is here.)
const REVERSE_TEST_SPEED: f32 = 4.0;

/// Below this the rider counts as standing still: the look-at stops leading and the position starts
/// leading instead.
const STILL_SPEED: f32 = 0.0001;

/// The longest step the look-at's lead is allowed to integrate over, so that one slow frame does not
/// throw the camera's target forward.
const LEAD_STEP_LIMIT: f32 = 0.2;

/// Falling faster than this shortens the look-ahead. The base class has a pair of these at plus and
/// minus five; this frame kept only the falling arm, and moved it out to nine.
const FALL_SPEED: f32 = -9.0;

/// How far the frame leads the *position* while the rider is standing still: ten frames' worth of
/// velocity, which is not nothing even at a speed below [`STILL_SPEED`].
const STILL_POSITION_LEAD: f32 = 10.0;

/// How much of the preset's pitch is trimmed away in a viewport shorter than
/// [`Switches::short_viewport`] calls short.
const SHORT_VIEWPORT_PITCH: f32 = 0.33;

/// What the race camera asks the rider for, beyond the [`Target`] the base class takes.
///
/// The original reads all three off the one actor the camera holds a pointer to: the target rows at
/// `+0x10`/`+0x30`/`+0x50`, the airborne flag at `+0xb0` and the speed at `+0xb4`. The speed is
/// *read*, not computed — where the base class takes the length of the velocity every frame, this
/// frame trusts the number the ride code already wrote, and the two are not the same whenever the
/// ride's own speed is anything but the length of that vector.
pub struct Ridden {
    /// The rows and the lean, exactly as the base class takes them.
    pub target: Target,
    /// Is the rider in the air: *no* wheel touching. Gates both of the "use the velocity" switches.
    ///
    /// The original's per-frame target writer stores `+0xb0` as `(on_ground == 0)` — `sltu` then
    /// `xori 1` on the vehicle's on-ground word, which is the snapshot's copy of "either wheel
    /// touches" — so the flag is the inverse of [`crate::air::Air::on_ground`]. S24 named it the
    /// other way round, and a shell that handed it the on-ground flag took the heading off the
    /// bike's forward row in the air, which a backflip turns upside down (P34).
    pub airborne: bool,
    /// The rider's speed, off the actor rather than out of the velocity.
    pub speed: F,
}

/// The two engine-wide bits the race camera's frame reads that are not the rider's and not the
/// preset's. Both are inputs here, because neither is the camera's to decide.
#[derive(Clone, Copy, Default)]
pub struct Switches {
    /// Bit 3 of the byte the app object holds at `+0x325`. With it set the frame takes its "standing
    /// still" branch whatever the speed: the look-at stops leading, the ground probe is not asked,
    /// and the lift decays. What the engine means by the bit was not pinned down — see the journal.
    pub no_look_ahead: bool,
    /// Is the viewport shorter than fifty-five hundredths as tall as it is wide — the original
    /// divides the viewport's two integer extents and compares. True on a horizontally split screen
    /// and false at 4:3 or 16:9, and it trims [`SHORT_VIEWPORT_PITCH`] of the preset's pitch away.
    /// The original also requires a mode word to not be three and an app sub-object's own flag to be
    /// clear; a caller that wants either of those to veto the trim passes `false`.
    pub short_viewport: bool,
}

/// The follow camera the disc actually creates for a race: the derived class whose vtable overrides
/// the frame, the look-at, the position, the aim and the ground probe.
///
/// The base class ([`Follow`]) is never instantiated on its own — its only constructor call is from
/// this one — so the bit-exact frame S7 proved is dead code on the disc, and this is the frame that
/// draws the picture. It is a near-copy, not a subclass that adds a step: it has its own look-at,
/// its own position and its own aim, each a rewrite of the base's with pieces taken out and pieces
/// put in. What survives unoverridden is the field of view, the filters and the preset handling, and
/// those are reached here through the base object this one holds.
///
/// Measured against [`Follow::update_probed`], the frame differs in these ways:
///
/// * the follow distance's floor is the near plane times [`NEAR_CLIP_FLOOR`], not the near plane;
/// * the speed is read off the rider ([`Ridden::speed`]) rather than taken from the velocity, and
///   the heading and the direction basis swing over to the velocity once the rider is airborne and
///   fast — so the camera follows the jump, not the bike's attitude through it;
/// * the lean swing does not carry the preset's `CameraSwing` into the look-at (the aim still
///   applies it to the yaw);
/// * reverse is one sign test on the velocity along the heading, which zeroes the velocity and the
///   speed, instead of an arc cosine and three ramps — the ramps are still read by the distance and
///   the aim, and nothing in this frame ever writes them;
/// * the aim scales the pitch down at low speed, through the preset's `LowSpeedAngleXVelCutoff` and
///   `LowSpeedAngleXScaleFactor` — the first reader found for either — and trims it again in a short
///   viewport, and it has no reverse term;
/// * the look-at leads from the *direction basis* and does not normalise it afterwards, drops the
///   base's cap of one on the along-heading term, keeps only the falling arm of the two vertical
///   corrections, and clamps the result at zero; the look-back cam zeroes the lead but still runs
///   the rest, where the base class returns early;
/// * the look-at's target is the target point led a step forward by the velocity;
/// * standing still, the look-at is the plain target point lifted by the height bias and the
///   position leads by ten steps of velocity instead;
/// * there is no obstacle push-out block at the end of the frame — what keeps the eye out of the
///   ground is the filter's own slot instead ([`Race::keep_off_the_ground`]).
///
/// The ground probe the base class stubs is overridden too; [`Follow::probe_ground`] is already that
/// override, so it is used from here as it is. So is the slot the base class's filter calls on the
/// position before and after the lags, a `return 1` in the base and the eye's floor in this class
/// ([`Race::filter`]).
pub struct Race {
    /// The base object this one is built on. Everything the derived class does not override — the
    /// presets, the filters, the field of view, the nudges, the ground lift — lives here and is
    /// reached through here.
    pub cam: Follow,
}

impl Race {
    /// A race camera over the given presets. The derived constructor runs the base one and then
    /// writes its own fields; of those, only the ground lift and the world pointer are ever read by
    /// the frame, and the lift is the base object's own.
    ///
    /// The race's setup then writes the eye's least height ([`RACE_MIN_HEIGHT`]) over the
    /// constructor's, and that is done here too; its ceiling is [`Race::ceiling_for_fog`]'s.
    pub fn new(presets: Vec<Preset>, reverse_speed: F) -> Self {
        let mut cam = Follow::new(presets, reverse_speed);
        cam.min_height = F(RACE_MIN_HEIGHT);
        Race { cam }
    }

    /// The ceiling the race's setup gives the eye on a scene whose fog record reaches `max_fog`:
    /// [`RACE_CEILING`] past [`CEILING_FOG`], none otherwise. Free09's 1000 ft gets none.
    pub fn ceiling_for_fog(max_fog: F) -> F {
        if F(CEILING_FOG) < max_fog { F(RACE_CEILING) } else { F::MAX }
    }

    /// **The eye's floor**: what keeps the race camera out of the ground. Vtable slot `+0x108`, which
    /// the base class's filter calls on the position twice a frame — on the raw one before the lags
    /// and on the lagged one after — and which is a `return 1` in the base class (S7's
    /// `camera_frame_noop`). This class's override is the only thing in its frame that stops the eye
    /// going under a hillside behind the rider, into a hollow, or down with a wreck.
    ///
    /// * The floor is [`NEAR_CLIP_HEIGHT`] near planes or [`Follow::min_height`], whichever is more:
    ///   4.8 ft at the race viewport's near plane.
    /// * The eye is dropped onto the ground under it through the world's own ground query
    ///   ([`Surface::on_surface`], the probe's question), as it was handed in.
    /// * Its height is capped at [`Follow::ceiling`].
    /// * If it is then less than the floor above that ground it is put exactly the floor above it;
    ///   only `y` moves. The filter always asks for the lift, so the original's "only report it" arm
    ///   is not here.
    /// * `lock` (the first of the two calls) records into [`Follow::pitch_down_locked`] whether the
    ///   eye was under the floor, which is what stops the player's stick pitching it further down.
    ///
    /// A camera with no world ([`Surface::has_ground`] false) does none of it, which is the original's
    /// world pointer left null, and also what a scene of [`Nothing`] is.
    ///
    /// The override starts with a second block, a segment probe from the target point out to the eye
    /// through the camera's collision object that parks the eye a floor's height off whatever it hit,
    /// along the hit's normal. That object is made by a vtable slot nothing in the race's setup calls,
    /// and every race camera read live holds a null there, so the block never runs on the disc and is
    /// not here (`research/oracle.txt`, S103).
    pub fn keep_off_the_ground(&mut self, eye: &mut Vf, near_clip: F, lock: bool, scene: &mut impl Surface) {
        let floor = (near_clip * NEAR_CLIP_HEIGHT).max(self.cam.min_height);
        if !scene.has_ground() {
            return;
        }
        let ground = F(scene.on_surface(*eye).y());
        let y = F(eye.y()).min(self.cam.ceiling);
        eye[1] = y.0;
        if y - ground < floor {
            if lock {
                self.cam.pitch_down_locked = true;
            }
            eye[1] = (ground + floor).0;
        } else if lock {
            self.cam.pitch_down_locked = false;
        }
    }

    /// The lags, as this class runs them: [`Follow::filter`] with this class's slot
    /// ([`Race::keep_off_the_ground`]) called where the base class's filter calls its own — on the raw
    /// position first, which is what the lags then chase (and what a snap loads them with), and again
    /// on the lagged position when the lags ran, which is what the frame hands the viewport. The lags
    /// are not told about the second lift.
    #[allow(clippy::too_many_arguments)]
    pub fn filter(
        &mut self,
        dt: F,
        near_clip: F,
        position: &mut Vf,
        look_at: &mut Vf,
        fov: &mut F,
        spare: &mut F,
        scene: &mut impl Surface,
    ) {
        self.keep_off_the_ground(position, near_clip, true, scene);
        let lags = self.cam.preset().filter_this_preset && !self.cam.snap;
        self.cam.filter(dt, position, look_at, fov, spare);
        if lags {
            self.keep_off_the_ground(position, near_clip, false, scene);
        }
    }

    /// Yaw and pitch, this class's way. Not a vtable slot at all, unlike the base class's: the
    /// derived frame is its one caller, where the base reaches [`Follow::aim`] through slot `+0x118`.
    ///
    /// The yaw is the base class's exactly. The pitch is not: there is no reverse ramp in it, and
    /// after the limit two trims can take it down.
    ///
    /// The first trim is the low-speed one, and it is the only reader this port has found for the
    /// preset's `LowSpeedAngleXVelCutoff` and `LowSpeedAngleXScaleFactor` — S7 recorded both as
    /// having no known reader. Below the cutoff the pitch is scaled towards the scale factor as the
    /// speed goes to zero, and then the factor times the rider's bare forward tilt is subtracted: a
    /// bike stopped on a slope looks level-ish rather than into the hill. A cutoff of zero turns the
    /// whole thing off even for a rider stopped dead, which is how the original tests it.
    ///
    /// The second is `short_viewport` (see [`Switches`]), which takes a third of the preset's pitch
    /// off.
    ///
    /// `forward_y` is the rider's forward axis' `y` — the actor's own row, not the flattened heading.
    pub fn aim(&mut self, swing: F, speed: F, forward_y: F, short_viewport: bool) {
        let c = &mut self.cam;
        let (angle_y, angle_x) = (c.preset().angle_y, c.preset().angle_x);
        c.yaw = angle_y - c.user_yaw;
        c.yaw = c.yaw - c.camera_swing * swing;
        c.yaw = F(libm::fmodf(c.yaw.0, TWO_PI));
        if c.yaw > PI {
            c.yaw = c.yaw - TWO_PI;
        } else if c.yaw < -PI {
            c.yaw = c.yaw + TWO_PI;
        }
        let cutoff = c.preset().low_speed_angle_x_vel_cutoff;
        c.pitch = c.user_pitch + angle_x;
        if c.pitch >= -PITCH_LIMIT {
            if c.pitch > PITCH_LIMIT {
                c.pitch = F(PITCH_LIMIT);
                c.user_pitch = F(PITCH_LIMIT) - angle_x;
            }
        } else {
            c.pitch = F(-PITCH_LIMIT);
            c.user_pitch = F(-PITCH_LIMIT) - angle_x;
        }
        if speed < cutoff && cutoff != 0.0 {
            let k = c.preset().low_speed_angle_x_scale_factor;
            c.pitch = c.pitch * (k + (speed / cutoff) * (F::ONE - k));
            c.pitch = c.pitch - k * forward_y.abs();
        }
        if short_viewport {
            c.pitch = c.pitch - F(SHORT_VIEWPORT_PITCH) * angle_x;
        }
    }

    /// The follow distance and its limits — [`Follow::limit_distance`] with [`NEAR_CLIP_FLOOR`] times
    /// the near clip for a floor instead of the near clip itself. Everything else about it, down to
    /// rewriting the player's zoom on a clamp, is the same.
    pub fn limit_distance(&mut self, near_clip: F) {
        let p = self.cam.preset().clone();
        let floor = p.min_following_distance.max(F(NEAR_CLIP_FLOOR) * near_clip);
        let want = self.cam.reverse_following + (self.cam.user_zoom + p.following_distance);
        self.cam.distance = want;
        if want < floor {
            self.cam.distance = floor;
            self.cam.user_zoom = self.cam.distance - p.following_distance;
        } else if p.max_following_distance < want {
            self.cam.distance = p.max_following_distance;
            self.cam.user_zoom = self.cam.distance - p.following_distance;
        }
    }

    /// Where the camera looks. Called straight from the frame, not through the vtable.
    ///
    /// A rewrite of [`Follow::look_at`], and the differences are all in the lead:
    ///
    /// * the lead is the plain speed over [`Follow::speed_reference`], where the base takes the
    ///   velocity's component along the heading and caps it at one — so this one does not shorten
    ///   when the rider slides sideways, and does not stop growing above the reference speed;
    /// * only a fall faster than [`FALL_SPEED`] corrects it, and the correction is divided by 1.8
    ///   where the base's is not;
    /// * the result is floored at zero after the preset's `LookAheadDistance` is applied, so a
    ///   correction that went negative does not push the target *behind* the rider;
    /// * the look-back cam takes zero lead and then runs the rest of the routine — including the
    ///   ground probe, which the base class's early return skips.
    ///
    /// The swung vector is the *direction basis* rotated by the lean swing, and unlike the base's it
    /// is not normalised afterwards: the basis went in normalised and a rotation does not change
    /// that, so the base's extra `rsqrt` was only ever rounding. It is used twice, as the base's is —
    /// once for the lead and once for the ground probe's reach — and the probe is
    /// [`Follow::ground_probe_point`] and [`Follow::probe_ground`] unchanged.
    ///
    /// `target_point` is the led point, not the plain one. The returned `w` is the led point's.
    #[allow(clippy::too_many_arguments)]
    pub fn look_at(
        &mut self,
        speed: F,
        fov_ratio: F,
        swing: F,
        target_point: Vf,
        direction_base: Vf,
        vel: Vf,
        curve: &CurveTable,
        scene: &mut impl Surface,
    ) -> Vf {
        let pull = (self.cam.distance * fov_ratio) / 90.0 + 1.0;
        let mut lead = (speed / self.cam.speed_reference)
            * (pull * ((F(1.8) * (F::ONE - self.cam.distance / 600.0)) * pull));
        if F(vel.y()) < FALL_SPEED {
            lead = lead * (F::ONE + (F(vel.y()) / speed) / 1.8);
        }
        let p = self.cam.preset();
        let ahead = if p.is_look_back_cam {
            F::ZERO
        } else {
            F::ZERO.max(lead * p.look_ahead_distance)
        };
        let swung = rotate(
            wrap_half_turn(swing * (F(-0.7) * self.cam.look_swing)),
            UP,
            direction_base,
            curve,
        );
        let point = add3(target_point, scale(swung, ahead));
        // The base class writes this as `(1 - ratio) * vel.y * 0.25`, left to right; this one
        // multiplies the quarter in first. Different rounding, same intent.
        let rise = F(point.y()) + (F::ONE - fov_ratio) * (F(vel.y()) * 0.25);
        let lift = self.cam.probe_ground(self.cam.ground_probe_point(swung, target_point), scene);
        let y = rise + (self.cam.height_bias + lift);
        Vf::new(point.x(), y.0, point.z(), point.w())
    }

    /// Where the camera is. Called straight from the frame, not through the vtable.
    ///
    /// The target point plus the direction times the follow distance, as the base class has it — and
    /// then, for a rider standing still, [`STILL_POSITION_LEAD`] steps of velocity on top. That arm
    /// is the mirror of the look-at's: below [`STILL_SPEED`] the look-at stops leading the target and
    /// the position starts leading instead, so the view drifts rather than freezing. The velocity it
    /// leads with is the rider's own, not the copy the frame may have zeroed for reverse.
    ///
    /// The original's routine also takes the heading, the look-at and the sideways axis and does
    /// nothing with any of them; they are not arguments here. The base class's dead yaw swing is not
    /// in this one at all.
    pub fn position(&self, dt: F, speed: F, direction: Vf, target_point: Vf, vel: Vf) -> Vf {
        let at = add3(target_point, scale(direction, self.cam.distance));
        if speed < STILL_SPEED {
            add3(at, scale(scale(vel, dt), F(STILL_POSITION_LEAD)))
        } else {
            at
        }
    }

    /// One frame of the race camera, in the original's order. Vtable slot `+0x38` of the derived
    /// class, which is slot `+0x98` of the secondary vtable the base class's frame sits in.
    ///
    /// `near_clip` is the viewport's near clip distance (see [`Race::limit_distance`] and
    /// [`Race::keep_off_the_ground`]); `switches` are the two engine bits the frame reads; `scene`
    /// answers the ground probe and the eye's floor. There is no obstacle argument because the
    /// floor's segment probe never runs on the disc (see [`Race::keep_off_the_ground`]).
    ///
    /// A camera with no actor runs none of this in the original and falls through to the base
    /// class's *other* update, the one that only ages the nudges; a caller here simply does not call
    /// it.
    pub fn update(
        &mut self,
        dt: F,
        rider: &Ridden,
        near_clip: F,
        curve: &CurveTable,
        switches: Switches,
        scene: &mut impl Surface,
    ) -> View {
        let t = &rider.target;
        let mut fov = self.cam.field_of_view();

        // The target point, exactly as the base class builds it.
        let mut offset = add3(
            scale(t.forward, F(self.cam.offset.z())),
            scale(t.up, F(self.cam.offset.y())),
        );
        if self.cam.offset.x() != 0.0 {
            offset = add3(offset, scale(cross(t.up, t.forward), F(self.cam.offset.x())));
        }
        let target_point = add3(t.body_point, offset);

        // The heading: the velocity once the rider is airborne and fast, the forward axis otherwise;
        // flattened and normalised either way. `y` is zeroed with an integer store and `w` comes
        // through from whichever row was picked.
        let from = if rider.airborne && rider.speed > HEADING_FROM_VEL_SPEED { t.vel } else { t.forward };
        let heading = normalize(Vf::new(from.x(), 0.0, from.z(), from.w()));

        let mut vel = t.vel;
        let mut speed = rider.speed;
        let fov_ratio = fov / self.cam.preset().max_fov;
        // No `CameraSwing` here — the base class folds it in at this point and this frame does not.
        // The aim still applies it to the yaw, so the preset is not ignored, only the look-at's
        // share of it.
        let swing = (F(0.21) * -t.lean) * fov_ratio;

        // Reverse, this frame's way: if the rider is moving against the heading at any speed worth
        // testing, the velocity and the speed are thrown away. Written as the original's test is, so
        // that a NaN speed takes the same branch.
        if !(speed <= REVERSE_TEST_SPEED) && !self.cam.preset().is_look_back_cam {
            let along = Acc::mula(F(heading.x()), F(vel.x()))
                .madda(F(heading.y()), F(vel.y()))
                .madd(F(heading.z()), F(vel.z()));
            if along < 0.0 {
                vel = Vf::new(0.0, 0.0, 0.0, 1.0);
                speed = F::ZERO;
            }
        }

        self.aim(swing, speed, F(t.forward.y()), switches.short_viewport);

        // The direction basis: the velocity again, but over a lower speed and only while it has a
        // horizontal part worth flattening. `w` is a literal one here, where the heading's came
        // through from the row.
        let steer = rider.airborne
            && !(speed <= BASIS_FROM_VEL_SPEED)
            && !((speed - F(vel.y()).abs()).abs() <= BASIS_DEGENERATE);
        let from = if steer { vel } else { heading };
        let basis = normalize(Vf::new(from.x(), 0.0, from.z(), 1.0));

        // Pitch about the sideways axis first, then yaw about up — with both angles wrapped into
        // `-pi..pi` first, which the base class's frame does not bother to do.
        let sideways = cross(basis, UP);
        let direction = rotate(
            wrap_half_turn(self.cam.yaw),
            UP,
            rotate(wrap_half_turn(self.cam.pitch), sideways, basis, curve),
            curve,
        );

        self.limit_distance(near_clip);

        let (mut look_at, lead_point) = if switches.no_look_ahead || speed < STILL_SPEED {
            let still = Vf::new(
                target_point.x(),
                (F(target_point.y()) + self.cam.height_bias).0,
                target_point.z(),
                target_point.w(),
            );
            (still, target_point)
        } else {
            let lead_point = add3(target_point, scale(vel, F(LEAD_STEP_LIMIT).min(dt)));
            let look_at =
                self.look_at(speed, fov_ratio, swing, lead_point, basis, vel, curve, scene);
            (look_at, lead_point)
        };

        let mut position = self.position(dt, speed, direction, lead_point, t.vel);
        let mut spare = F::ZERO;
        // The filters are the base class's, unoverridden, but the slot they call on the position is
        // this class's floor. The original also hands them the plain target point, which only the
        // floor's segment probe would read.
        self.filter(dt, near_clip, &mut position, &mut look_at, &mut fov, &mut spare, scene);
        View { position, look_at, fov }
    }
}
