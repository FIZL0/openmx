//! What moves on the biplane: the pilot sitting in it, and the plane's own moving parts.
//!
//! **The pilot.** The pilot is a rider's body like any other — the vehicle-animation object
//! [`vehicle_anim`](crate::vehicle_anim) ports the step of — and nothing in its state machine knows
//! about planes. Three things turn it into a pilot:
//!
//! 1. The rider build adds one record to every body's library for each biplane or helicopter in the
//!    machine list ([`register`]): the plane's type as the key, `biplane_roll_bck` and
//!    `biplane_roll_fwd` as the two clips.
//! 2. Mounting the plane points the body's request at the plane's own request block and its added
//!    record at the one whose key is the plane's type ([`Driver::mount`]). From then on the
//!    selection plays that record and nothing else: both clips, the back roll at the phase axis and the
//!    two mixed by the mix axis ([`Blend::Two`](crate::vehicle_anim::Blend::Two)).
//! 3. The plane's measure step writes that request block every frame ([`Pilot::ask`]): the aileron
//!    stick into the phase axis, the elevator stick into the mix axis, both centred on a half. The
//!    body's own two lags ease them in, which is the whole of the pilot's lean.
//!
//! The state machine still runs underneath — the chooser still asks for the airborne row in the air
//! and the riding grid on the ground, and still draws its fidget timer off the shared generator — but
//! with an added record mounted none of it reaches the clips.
//!
//! **The moving parts** ([`surfaces`]) are bones of the plane's model, each posed every frame from
//! the controls: the four wing flaps by the aileron, the two rear flaps by the elevator, the rudder by
//! the rudder, and the prop dummy by the engine's prop angle, which also carries one of the two prop
//! cones — the spinning blur, or the bent one once the plane is wrecked. None of this reaches the
//! simulation: the port answers with angles and which cone, and the drawing side builds the matrices.
//!
//! What the flight port (S34) has to call, and where — the port has no plane object to hang this on
//! yet — is in the S35 journal entry.
#![deny(clippy::float_arithmetic)]

use crate::motion::Animations;
use crate::vehicle_anim::{Asked, Driver, Library, NO_GESTURE, Request};
use ps2_float::{F, Vf};

// ---------------------------------------------------------------------------------------------------
// the pilot's clips
// ---------------------------------------------------------------------------------------------------

/// The clip the added record plays at the phase axis: the aileron's, and the one a centred stick
/// sits half-way through.
pub const ROLL_BCK: &str = "biplane_roll_bck";
/// The clip the mix axis mixes towards: the elevator's.
pub const ROLL_FWD: &str = "biplane_roll_fwd";

/// The biplane's actor type: the first word of the actor, and the key its pilot's record is filed
/// under. The live plane in the human's recording has a 1 there on every tick.
pub const PLANE: i32 = 1;

/// Which kinds of machine-list entry get the pair: 1 the biplane and 2 the helicopter. The other three
/// kinds have steering clips of their own, which are not ported.
pub fn flies(kind: i32) -> bool {
    kind == 1 || kind == 2
}

/// The rider build's half: one record per flying machine in the list, keyed by the machine actor's
/// type, the back roll as the record's first clip and the forward roll as its second.
///
/// `enabled` is the body's own "takes added records" bit. Every body in the human's recording has it
/// set; its setter is not located, so the caller says.
pub fn register(lib: &mut Library, enabled: bool, key: i32, anim: &mut impl Animations) {
    lib.add(enabled, key, ROLL_BCK, ROLL_FWD, anim);
}

// ---------------------------------------------------------------------------------------------------
// the plane's request block
// ---------------------------------------------------------------------------------------------------

/// The plane's reset flag, and the frame step that lets go of it.
///
/// The reset raises it. The frame step lowers it only when it was already up the last time the step
/// looked, so it stays up for the step that sees it first, and the request's holding flag — a copy
/// of it made by the measure step — is what snaps the pilot's two axes onto the stick after a reset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reset {
    /// Just reset: the plane's holding flag.
    pub just: bool,
    /// [`Reset::just`] as the last frame step left it.
    pub seen: bool,
}

impl Reset {
    /// The plane is put back on its spawn transform.
    pub fn reset(&mut self) {
        self.just = true;
    }

    /// The plane's frame step's own two lines.
    pub fn frame(&mut self) {
        if self.just && self.seen {
            self.just = false;
        }
        self.seen = self.just;
    }
}

/// What the plane's measure step hands its pilot: the inputs of the request writer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pilot {
    /// The elevator stick as the controls left it: up positive, `-1..1`.
    pub elevator: F,
    /// The aileron stick: left positive, `-1..1`.
    pub aileron: F,
    /// The plane's reset flag ([`Reset::just`]).
    pub held: bool,
    /// Any of the three tyres touches.
    pub on_ground: bool,
    /// The plane's airspeed, `|velocity|`.
    pub speed: F,
    /// Where the plane's body is, as its rigid body holds it.
    pub base: Vf,
    /// Which way it faces, the rigid body's quaternion.
    pub rotation: Vf,
    /// The pilot's seat: the plane's `PelvisBoxPosition`.
    pub seat: Vf,
}

impl Pilot {
    /// The request block as the plane's constructor leaves it: taking the position from the base, no
    /// gesture, and nothing else asked. The first measure overwrites everything the plane ever writes.
    pub fn block() -> Asked {
        Asked {
            request: Request { at_base: true, ..Request::default() },
            gesture: NO_GESTURE,
            ..Asked::default()
        }
    }

    /// The axis targets: the aileron stick's `0.5 * (1 - x)` on the phase axis, so a stick held right
    /// plays the back roll further in, and the elevator's `0.5 * (1 + y)` on the mix axis.
    pub fn axes(&self) -> (F, F) {
        (F(0.5) * (F::ONE - self.aileron), F(0.5) * (F::ONE + self.elevator))
    }

    /// The measure step's writes into the request block, and only those.
    ///
    /// The measure comes before the controls in a tick, so the sticks it writes from are the ones the
    /// previous tick's controls left, and the pilot leans a tick behind the plane's surfaces. It comes
    /// after the tick's reset and before the reset flag's latch ([`Reset::frame`]).
    ///
    /// Everything the plane does not write keeps what the constructor put there ([`Pilot::block`]):
    /// no gesture, not wrecked, nothing commanded. A wrecked plane does not tell its pilot so.
    pub fn ask(&self, a: &mut Asked) {
        let (phase, mix) = self.axes();
        a.request.at_base = false;
        a.request.base = self.base;
        a.request.rotation = self.rotation;
        a.request.band = self.speed;
        a.request.offset = self.seat;
        a.airborne = !self.on_ground;
        a.request.held = self.held;
        a.request.phase_axis = phase;
        a.request.mix_axis = mix;
    }
}

// ---------------------------------------------------------------------------------------------------
// the moving parts
// ---------------------------------------------------------------------------------------------------

/// The four wing flaps, in the order the plane finds and poses them.
pub const FLAPS: [&str; 4] = ["BottomLeftFlap", "TopLeftFlap", "BottomRightFlap", "TopRightFlap"];
/// The two rear flaps: the elevator.
pub const REAR: [&str; 2] = ["RearLeftFlap", "RearRightFlap"];
/// The rudder's bone.
pub const RUDDER: &str = "Rudder";
/// The bone the prop turns on and the prop cone is stood at.
pub const PROP: &str = "PropDummy";
/// The prop cone shown while the plane flies.
pub const CONE_BLUR: &str = "propconeblur";
/// The one shown once it is wrecked.
pub const CONE_BENT: &str = "propconebent";

/// One frame's pose of the plane's moving parts, as angles in radians.
///
/// Two kinds of turn, and the drawing side has to keep them apart. In the PS2's row-vector order
/// (`v' = v · M`), with `R(axis, a)` the rotation by `a` about a unit axis:
/// - each wing flap is put back on the rest matrix the plane kept when it found the bone, and then
///   turned about its own hinge — the rest matrix's first row, kept beside it — by its angle:
///   `R(hinge, a) · rest`, the position the rest matrix's. A zero angle is not turned at all.
/// - the rear flaps, the rudder and the prop have their rotation **replaced** by `R(axis, a)` about a
///   fixed axis — `x` for the rear flaps, `y` for the rudder, `z` for the prop — and keep their
///   position.
///
/// The human's recording agrees with both to a thousandth on the wing flaps and a few millionths on
/// the rest (`tests/biplane_anim.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surfaces {
    /// Per [`FLAPS`] entry: the aileron on the left two, the aileron negated on the right two.
    pub flaps: [F; 4],
    /// Both [`REAR`] flaps: the elevator as a fraction, times `MaxElevator`, negated.
    pub rear: F,
    /// [`RUDDER`]: the rudder, negated.
    pub rudder: F,
    /// [`PROP`]: the engine's prop angle, or `None` once the plane is wrecked, when it is not turned.
    pub prop: Option<F>,
    /// Which prop cone is stood at the prop's world matrix and shown; the other is hidden.
    pub cone: &'static str,
}

/// The plane's surface step: the controls' angles onto the bones, in the original's order.
///
/// `elevator` is the elevator as a fraction of `max_elevator`, `aileron` and `rudder` are radians,
/// `prop_angle` is the engine's, and `wrecked` is a crash reason other than none — as it stands when
/// the step runs, which is after the tick's reset and before its wreck test, so the tick a plane is
/// wrecked on still turns the prop, and so does the tick it is put back on.
pub fn surfaces(elevator: F, max_elevator: F, aileron: F, rudder: F, prop_angle: F, wrecked: bool) -> Surfaces {
    Surfaces {
        flaps: [aileron, aileron, -aileron, -aileron],
        rear: -(elevator * max_elevator),
        rudder: -rudder,
        prop: (!wrecked).then_some(prop_angle),
        cone: if wrecked { CONE_BENT } else { CONE_BLUR },
    }
}

// ---------------------------------------------------------------------------------------------------
// the body's half
// ---------------------------------------------------------------------------------------------------

impl Driver {
    /// The body is put on a machine: the added record whose key is the machine's type becomes the one
    /// the selection plays, or none if the library has no record for it.
    ///
    /// The first match wins. In the original this is the tail of the body's "mount" method, behind two
    /// gates the port leaves to the caller: a body that already has a request is not remounted, and
    /// the call's mask has to share a bit with the body's own.
    pub fn mount(&mut self, lib: &Library, key: i32) {
        self.extra = lib
            .extra
            .iter()
            .position(|rec| rec.key == key)
            .and_then(|i| i32::try_from(i).ok())
            .unwrap_or(-1);
    }
}
