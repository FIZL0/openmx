//! MD75e — **the partner borrow on a moving ground, inside the quad**: the Force body with the wheel
//! integrate, its surface drag and its spin unhooked (as [`super::the_quads_wheel_spin_is_the_originals`]),
//! a wheel off the ground whose partner is down on a collision object (on a rigid body or on a
//! vehicle's chassis) or on a mover. The wheel takes its partner's ground and runs the contact
//! velocity on it; [`atv_wheel::spin_integrate`] copies the partner's [`Ground`] where the original
//! copies its two pointers, so the original's one object is the port's two copies. The harness lays
//! the object once and points every wheel standing on it there, as the port's copies say.

use super::*;
use game_sim::atv_wheel::{self, Chassis, Ground, Holder, Mover};
use game_sim::contact::Node;

/// Where the ground is laid: the collision object (`+0x80` its centre of mass, `+0x90` the contact
/// velocity's world copy of it, `+0xe0` a chassis, `+0xe4` a body, `+0xe8` its transform), the body
/// or chassis it is on, the transform, the mover (`+4` its vtable, `+0x180` its origin, `+0x244` its
/// transform) and the mover's two virtuals.
const GROUND_OBJECT: u32 = 0x01c3_0000;
const GROUND_BODY: u32 = 0x01c3_0400;
const GROUND_CHASSIS: u32 = 0x01c3_0800;
const GROUND_NODE: u32 = 0x01c3_0c00;
const GROUND_MOVER: u32 = 0x01c3_1000;
const MOVER_VT: u32 = 0x01c3_1400;
const MOVER_VELOCITY_FN: u32 = 0x01cf_0a00;
const MOVER_SPIN_FN: u32 = 0x01cf_0a10;

impl Gen {
    fn ground_node(&mut self) -> Node {
        Node { rows: [self.normal(), self.normal(), self.normal()], origin: self.vec(-50.0, 50.0) }
    }

    /// Something moving under a wheel.
    fn moving_ground(&mut self) -> Ground {
        match self.bits() % 3 {
            0 => Ground::Object {
                cofg: self.vec(-1.0, 1.0),
                cofg_world: self.vec(-1.0, 1.0),
                on: Holder::Body(atv_wheel::Body {
                    position: self.vec(-50.0, 50.0),
                    velocity: self.vec(-10.0, 10.0),
                    orientation: self.quaternion(),
                    spin: self.vec(-3.0, 3.0),
                    ..Default::default()
                }),
            },
            1 => Ground::Object {
                cofg: self.vec(-1.0, 1.0),
                cofg_world: self.vec(-1.0, 1.0),
                on: Holder::Chassis(Chassis {
                    cofg: self.vec(-1.0, 1.0),
                    velocity: self.vec(-10.0, 10.0),
                    spin: self.vec(-3.0, 3.0),
                    node: self.ground_node(),
                }),
            },
            _ => Ground::Mover(Mover {
                node: self.ground_node(),
                origin: self.vec(-1.0, 1.0),
                velocity: self.vec(-10.0, 10.0),
                spin: self.vec(-3.0, 3.0),
            }),
        }
    }
}

fn lay_node(ee: &mut Ee, n: &Node) {
    for (k, r) in n.rows.iter().enumerate() {
        put_v(ee, GROUND_NODE + 0x70 + 0x10 * k as u32, *r);
    }
    put_v(ee, GROUND_NODE + 0xa0, n.origin);
    ee.set_u32(GROUND_NODE + 0xf0, 0); // no parent
    ee.set_u32(GROUND_NODE + 0x100, 1); // up to date
}

/// `g` laid once, and each wheel standing on something pointed at it.
fn lay_ground(ee: &mut Ee, g: &Ground, under: &[Ground; 4]) {
    for a in (GROUND_OBJECT..MOVER_VT + 0x100).step_by(4) {
        ee.set_u32(a, 0);
    }
    match g {
        Ground::Still => {}
        Ground::Object { cofg, cofg_world, on } => {
            put_v(ee, GROUND_OBJECT + 0x80, *cofg);
            put_v(ee, GROUND_OBJECT + 0x90, *cofg_world);
            match on {
                Holder::Body(o) => {
                    ee.set_u32(GROUND_OBJECT + 0xe4, GROUND_BODY);
                    // A chassis and transform there too, which the body hides.
                    ee.set_u32(GROUND_OBJECT + 0xe0, GROUND_CHASSIS);
                    ee.set_u32(GROUND_OBJECT + 0xe8, GROUND_NODE);
                    put_v(ee, GROUND_BODY + 0x70, o.position);
                    put_v(ee, GROUND_BODY + 0x80, o.velocity);
                    put_v(ee, GROUND_BODY + 0x190, o.orientation);
                    put_v(ee, GROUND_BODY + 0x1d0, o.spin);
                }
                Holder::Chassis(c) => {
                    ee.set_u32(GROUND_OBJECT + 0xe0, GROUND_CHASSIS);
                    ee.set_u32(GROUND_OBJECT + 0xe8, GROUND_NODE);
                    put_v(ee, GROUND_CHASSIS + 0x180, c.cofg);
                    put_v(ee, GROUND_CHASSIS + 0x250, c.velocity);
                    put_v(ee, GROUND_CHASSIS + 0x280, c.spin);
                    lay_node(ee, &c.node);
                }
            }
        }
        Ground::Mover(m) => {
            ee.set_u32(GROUND_MOVER + 4, MOVER_VT);
            ee.set_u32(GROUND_MOVER + 0x244, GROUND_NODE);
            put_v(ee, GROUND_MOVER + 0x180, m.origin);
            lay_node(ee, &m.node);
            ee.set_u32(MOVER_VT + 0xc0, MOVER_VELOCITY_FN);
            ee.set_u32(MOVER_VT + 0xc4, MOVER_SPIN_FN);
        }
    }
    for (u, &at) in under.iter().zip(&WHEEL) {
        let (object, mover) = match u {
            Ground::Still => (0, 0),
            Ground::Object { .. } => (GROUND_OBJECT, 0),
            Ground::Mover(_) => (0, GROUND_MOVER),
        };
        ee.set_u32(at + 0x1f0, object);
        ee.set_u32(at + 0x1f4, mover);
    }
}

/// The object's world centre of mass as the contact velocity leaves it.
fn cofg_world(g: &Ground) -> Option<Vf> {
    match g {
        Ground::Object { cofg_world, .. } => Some(*cofg_world),
        _ => None,
    }
}

/// **The partner borrow on objects and movers is the original's**: generated rides with one wheel
/// off the ground, borrowing (the rear-pair flag), its partner down on something moving; the rest
/// of the quad's wheels on still ground. Every word of the harness's regions equal after the Force
/// body, every wheel's ground pointers the same place, and the object's world centre of mass the
/// one the port's copies computed.
#[test]
fn the_quads_partner_borrow_on_moving_ground_is_the_originals() {
    let Some(mut ee) = image() else { return };
    let mask = ee.u32_at(STRUCK_MASK);
    let picks = [WHEEL_INTEGRATE, SURFACE_DRAG, SPIN_INTEGRATE];
    let answers: Answers = Rc::default();
    let echo = Rc::new(Cell::new(false));
    let log = hook_steps(&mut ee, &picks, &answers, &echo);
    let mover = Rc::new(Cell::new((Vf::ZERO, Vf::ZERO)));
    for (at, pick) in [(MOVER_VELOCITY_FN, 0), (MOVER_SPIN_FN, 1)] {
        let m = mover.clone();
        ee.hooks.insert(at, Box::new(move |ee| {
            assert_eq!(ee.r(5) as u32, GROUND_MOVER, "the mover's virtual's object");
            let out = ee.r(4);
            let (v, s) = m.get();
            put_v(ee, out as u32, if pick == 0 { v } else { s });
            ee.set(2, out);
        }));
    }
    let mut g = Gen(0x0075_e001);
    let mut t = BTreeMap::new();
    for k in 0..4000 {
        let mut r = g.ride(mask);
        r.s.vstate = Some(g.bits() % 3);
        r.s.q.active = true;
        let i = (g.bits() % 4) as usize;
        let j = PARTNER[i];
        let under = g.moving_ground();
        r.bike.ground = [Ground::Still; 4];
        r.bike.ground[j] = under;
        // The borrower's own ground, which the borrow overwrites: still, the partner's, or the other
        // kind (its pointer laid at nothing: only the borrow's run is compared).
        let own = g.bits() % 3;
        r.bike.ground[i] = match own {
            0 => Ground::Still,
            1 => under,
            _ => loop {
                let o = g.moving_ground();
                if matches!(o, Ground::Mover(_)) != matches!(under, Ground::Mover(_)) {
                    break o;
                }
            },
        };
        (r.s.q.wheels[i].touching, r.s.q.wheels[i].rear_pair, r.s.q.wheels[j].touching) = (false, true, true);
        if g.one_in(6) {
            r.bike.more[i].ground_known = 1;
            tally(&mut t, "ground already known");
        } else {
            r.bike.more[i].ground_known = 0;
        }
        tally(&mut t, match under {
            Ground::Object { on: Holder::Body(_), .. } => "an object on a body",
            Ground::Object { on: Holder::Chassis(_), .. } => "an object on a chassis",
            _ => "a mover",
        });
        if let Ground::Mover(m) = under {
            mover.set((m.velocity, m.spin));
        }

        // The original, the borrower's own ground (if any) where the object is too: it is never read.
        log.borrow_mut().clear();
        *answers.borrow_mut() = r.answers.iter().copied().collect();
        echo.set(r.echo);
        reset_all(&mut ee);
        poke_ride(&mut ee, &r);
        lay_ground(&mut ee, &under, &r.bike.ground);
        ee.call(GROUND_STEP, &[u64::from(OBJ)], &[r.s.dt.0], 2_000_000);
        let was = snap_all(&ee);
        let was_cofg = get_v(&ee, GROUND_OBJECT + 0x90);
        let pointers: Vec<[u32; 2]> = WHEEL.iter().map(|&at| [ee.u32_at(at + 0x1f0), ee.u32_at(at + 0x1f4)]).collect();

        // The port.
        let mut u = r.clone();
        let mut queue: std::collections::VecDeque<(Vf, Vf, Option<u8>)> = r.answers.iter().copied().collect();
        let echo_now = r.echo;
        let ground = move |q: Vf, record: &mut u8| {
            let (p, n, r) = queue.pop_front().expect("more ground queries than answers");
            if let Some(r) = r {
                *record = r;
            }
            (if echo_now { q } else { p }, n)
        };
        let mut m = Only { picks: &picks, steps: quad::Steps { bike: u.bike.clone(), rest: Vec::new(), ground, sweep: () } };
        quad::ground_step(&u.s.p, &mut u.s.q, &mut u.s.body, u.s.dt, &mut m);
        u.bike = m.steps.bike;
        let borrowed = !u.s.q.wheels[i].touching;
        if borrowed {
            tally(&mut t, "borrowed");
            if own == 2 {
                tally(&mut t, "own ground replaced");
            }
        } else if own == 2 {
            continue;
        }
        reset_all(&mut ee);
        poke_ride(&mut ee, &u);
        lay_ground(&mut ee, &under, &u.bike.ground);
        let now = snap_all(&ee);
        if let Some(n) = was.iter().zip(&now).position(|(a, b)| a != b) {
            panic!(
                "state {k}: {} is {:#010x} ({}) in the original and {:#010x} ({}) in the port\n{r:#?}",
                where_in_all(n),
                was[n],
                f32::from_bits(was[n]),
                now[n],
                f32::from_bits(now[n]),
            );
        }
        let ported: Vec<[u32; 2]> = WHEEL.iter().map(|&at| [ee.u32_at(at + 0x1f0), ee.u32_at(at + 0x1f4)]).collect();
        assert_eq!(pointers, ported, "state {k}: the wheels' ground pointers (object, mover)\n{r:#?}");
        // The object's world centre of mass: the copies the port's contact velocities wrote, each the
        // original's one word; none written, the laid one.
        let laid = cofg_world(&under);
        let wrote: Vec<Vf> = u.bike.ground.iter().filter_map(cofg_world).filter(|&c| Some(c.to_bits()) != laid.map(Vf::to_bits)).collect();
        if let Some(c) = laid {
            for c in wrote.iter().copied().chain(wrote.is_empty().then_some(c)) {
                assert_eq!(c.to_bits(), was_cofg.to_bits(), "state {k}: the object's world centre of mass\n{r:#?}");
            }
        }
        assert_eq!(*log.borrow(), m.steps.rest, "state {k}: the calls\n{r:#?}");
    }
    each_path_seen(
        &t,
        &["an object on a body", "an object on a chassis", "a mover", "own ground replaced", "ground already known", "borrowed"],
        100,
    );
}
