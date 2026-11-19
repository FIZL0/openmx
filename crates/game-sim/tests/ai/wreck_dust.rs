// ---------------------------------------------------------------------------------------------------
// S422 — the collision models' and the foot dust's draws, on a wreck
// ---------------------------------------------------------------------------------------------------

use super::*;
use game_sim::dust::{Ball, Cull, Emitter, Entry, Feed, Kind, Object, Place, Touches, View};
use ps2_oracle::{Ee, Kit};
use std::collections::HashSet;

/// The two entries' vtables: `CollisionModel` and `ParticleCollisionConstraint`.
const CM: u32 = 0x0045_e690;
const PCC: u32 = 0x0045_e660;

fn vf(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|o| ee.f32_at(a + o)))
}

/// A collision entry, its emitter and what its update reads, out of the image: the entry's
/// `Disable` `+0xc`, zones `+0x18` (a collision model) or mask `+0x18` and zones `+0x1c` (a bone
/// collision entry); the emitter `+0x14`'s switch `+0x240` (on 4, just on 2), `MinimumEmissionSpeed`
/// `+0x3c4`, points `+0x40` / `+0x60` and view cull (`+0x244` on, distances `+0x24c` / `+0x250`, the
/// last test `+0x248`); the feed (manager `+0x30`)'s wreck flag `+0x24`. A
/// collision model's object is `**(entry + 0x28)`: ground `+0x48`, object `+0x4c`, the collider's
/// relative velocity `+0x1e0`, contacts `+0x100` from `+0x108` (`0x80` each: point `+0x40`, surface
/// `+0x70`). The bone collisions are the feed's, `+0x10` from `+0x14`: ground `+0x18`, scene `+0x1c`,
/// surface `+0x3c`, point `+0x40`, the bone's body `+0x80` (velocity `+0x70`).
fn seed(ee: &Ee, e: u32) -> (Entry, Feed, Touches) {
    let vt = ee.u32_at(e + 0x10);
    let zones = |at: u32| core::array::from_fn(|i| ee.u8_at(at + i as u32));
    let kind = if vt == CM { Kind::Collision { zones: zones(e + 0x18) } } else { Kind::Spheres { mask: ee.u32_at(e + 0x18), zones: zones(e + 0x1c) } };
    let em = ee.u32_at(e + 0x14);
    let switch = ee.u8_at(em + 0x240);
    let flag = ee.u8_at(e + 8);
    let entry = Entry {
        kind,
        tyre: None,
        disable: ee.u32_at(e + 0xc) as u8,
        live: flag & 1 != 0,
        zone_off: flag & 2 != 0,
        emitter: Emitter {
            on: switch & 4 != 0,
            just_on: switch & 2 != 0,
            min_speed: F(ee.f32_at(em + 0x3c4)),
            place: Place::Chassis(Vf([0.0; 4])),
            pos: vf(ee, em + 0x40),
            prev: vf(ee, em + 0x60),
            cull: (ee.u32_at(em + 0x244) != 0).then(|| Cull { far: F(ee.f32_at(em + 0x24c)), directional: F(ee.f32_at(em + 0x250)) }),
            culled: ee.u32_at(em + 0x248) != 0,
        },
        draws: 0,
    };
    let fed = ee.u32_at(ee.u32_at(e) + 0x30);
    let feed = Feed { wrecking: ee.u32_at(ee.u32_at(fed + 0x24)) != 0, ..Feed::default() };
    let mut touches = Touches::default();
    if vt == CM {
        let obj = ee.u32_at(ee.u32_at(e + 0x28));
        let at = ee.u32_at(obj + 0x108);
        touches.object = Some(Object {
            ground: ee.u32_at(obj + 0x48) != 0,
            object: ee.u32_at(obj + 0x4c) != 0,
            relative: vf(ee, obj + 0x1e0),
            contacts: (0..ee.u32_at(obj + 0x100)).map(|i| (vf(ee, at + 0x80 * i + 0x40), ee.i32_at(at + 0x80 * i + 0x70))).collect(),
        });
    } else {
        let at = ee.u32_at(fed + 0x14);
        touches.balls = (0..ee.u32_at(fed + 0x10))
            .map(|k| ee.u32_at(at + 4 * k))
            .map(|c| {
                (c != 0).then(|| Ball {
                    ground: ee.u32_at(c + 0x18) != 0,
                    object: ee.u32_at(c + 0x1c) != 0,
                    surface: ee.u8_at(c + 0x3c),
                    point: vf(ee, c + 0x40),
                    velocity: vf(ee, ee.u32_at(c + 0x80) + 0x70),
                })
            })
            .collect();
    }
    (entry, feed, touches)
}

/// **S422: the collision entries' draws on a wreck, against the original.** S325's nose dive
/// (`capture/s138/s325-nose76-2604`) run as the original's whole frames, 45 of them: the rider lands
/// on the nose at 2615 and is thrown. Before every collision model's and bone collision entry's
/// update the port's [`Entry::touch_update`] is seeded from the image and run; the original's update
/// then runs, and the rands its emits make and its emitter's switch and points after must be the
/// port's. No other code may emit on those emitters. After each frame every such emitter's view test
/// ([`Emitter::view_test`]) on the frame's camera (the display's: point `+0x140`, forward `+0x150`)
/// must answer the image's.
#[test]
fn s422_collision_entries_draw_as_the_original() {
    use super::draws::{neutral_pad, render, render_setup, through, whole_frame};
    let kit = Kit::load().unwrap();
    let mut ee = Ee::from_ram(&kit.ctx("capture/s138/s325-nose76-2604.ee.bin")).unwrap();
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let text = std::fs::read_to_string(kit.ctx("capture/s138/s325-nose76.csv")).unwrap();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
    let (fc, dtc) = (col("frame_counter"), col("dt"));
    let rows: Vec<(u64, F)> = lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .map(|c| (c[fc].parse().unwrap(), F(c[dtc].parse::<f64>().unwrap() as f32)))
        .filter(|r| r.0 >= 2604)
        .collect();
    let rand_state = addr("rand_state").unwrap();
    let draws: Rc<RefCell<u32>> = Rc::default();
    let d = Rc::clone(&draws);
    ee.hooks.insert(
        addr("rand").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let n = super::sim::s12e_u64(ee, rand_state).wrapping_mul(0x5851_f42d_4c95_7f2d).wrapping_add(1);
            ee.write(rand_state, 8, u128::from(n));
            ee.set(2, (n >> 32) & 0x7fff_ffff);
            *d.borrow_mut() += 1;
        }),
    );
    let mut ours = HashSet::new();
    let mut emitters = Vec::new();
    for at in (0x0010_0000u32..0x0200_0000).step_by(4) {
        let vt = ee.u32_at(at);
        if vt == CM || vt == PCC {
            ours.insert(ee.u32_at(at - 0x10 + 0x14));
            emitters.push(at - 0x10);
        }
    }
    let inside: Rc<RefCell<bool>> = Rc::default();
    let wrong: Rc<RefCell<Vec<String>>> = Rc::default();
    let (i, w) = (Rc::clone(&inside), Rc::clone(&wrong));
    ee.hooks.insert(
        addr("emit").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let (a0, ra) = (ee.r(4) as u32, ee.r(31) as u32);
            if ours.contains(&a0) && !*i.borrow() {
                w.borrow_mut().push(format!("emitter {a0:#x} emitted from {ra:#x}, outside its entry's update"));
            }
            through(ee);
        }),
    );
    // Per kind: updates, updates that emitted, and their draws.
    let tally: Rc<RefCell<[[u32; 3]; 2]>> = Rc::default();
    let tick: Rc<RefCell<u64>> = Rc::default();
    for (name, k) in [("vehicle_fx_collision", 0), ("vehicle_fx_pcc_update", 1)] {
        let (d, i, w, t, now) = (Rc::clone(&draws), Rc::clone(&inside), Rc::clone(&wrong), Rc::clone(&tally), Rc::clone(&tick));
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee: &mut Ee| {
                let e = ee.r(4) as u32;
                let dt = F(ee.f(12));
                let (mut entry, feed, touches) = seed(ee, e);
                let want = entry.touch_update(&feed, &touches, dt);
                let before = *d.borrow();
                *i.borrow_mut() = true;
                through(ee);
                *i.borrow_mut() = false;
                let got = *d.borrow() - before;
                let em = ee.u32_at(e + 0x14);
                let switch = ee.u8_at(em + 0x240);
                let after = (switch & 4 != 0, switch & 2 != 0, vf(ee, em + 0x40).to_bits(), vf(ee, em + 0x60).to_bits());
                let port = &entry.emitter;
                let ours = (port.on, port.just_on, port.pos.to_bits(), port.prev.to_bits());
                if got != want || after != ours {
                    w.borrow_mut().push(format!("tick {} entry {e:#x}: draws {got} vs the port's {want}, emitter {after:x?} vs {ours:x?}", *now.borrow()));
                }
                let mut t = t.borrow_mut();
                t[k][0] += 1;
                t[k][1] += u32::from(got > 0);
                t[k][2] += got;
            }),
        );
    }
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    render_setup(&mut ee);
    let mut culls = [0; 2];
    for r in rows.windows(2).take(45) {
        *tick.borrow_mut() = r[1].0;
        neutral_pad(&mut ee);
        whole_frame(&mut ee, r[0].1, &mut clock);
        render(&mut ee);
        let cam = ee.u32_at(ee.u32_at(addr("display_object").unwrap()));
        let view = View { at: vf(&ee, cam + 0x140), forward: vf(&ee, cam + 0x150) };
        for &e in &emitters {
            let mut entry = seed(&ee, e).0;
            let em = ee.u32_at(e + 0x14);
            entry.emitter.culled = false;
            entry.emitter.view_test(&view);
            if entry.emitter.cull.is_some() {
                culls[usize::from(entry.emitter.culled)] += 1;
            }
            if entry.emitter.culled != (ee.u32_at(em + 0x248) != 0) {
                wrong.borrow_mut().push(format!("tick {}: emitter {em:#x}'s view test {} vs the image's {}", r[1].0, entry.emitter.culled, ee.u32_at(em + 0x248)));
            }
        }
    }
    let wrong = wrong.borrow();
    assert!(wrong.is_empty(), "{} differences, the first:\n{}", wrong.len(), wrong.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    let t = *tally.borrow();
    println!("collision models {:?}, bone collision entries {:?} (updates, emitting, draws); view tests seen, culled {culls:?}", t[0], t[1]);
    assert!(t[0][1] > 0 && t[1][1] > 0, "both kinds drew in the wreck: {t:?}");
    assert!(culls[0] > 0 && culls[1] > 0, "the view tests answered both ways: {culls:?}");
}

/// The rider body actors in `ee` (the original's class, by its vtable at `+8`, with eighteen parts)
/// and, for each, a body's bone: the part (`+0x2f4`, `0xb0` each, body `+0xa4`) it is.
fn actors(ee: &Ee) -> Vec<u32> {
    (0x0010_0000..0x0200_0000u32).step_by(16).filter(|&a| ee.u32_at(a + 8) == 0x0045_8eb0 && ee.u32_at(a + 0x2e0) == 18).collect()
}

fn bone_of(ee: &Ee, actor: u32, body: u32) -> Option<usize> {
    (0..18).find(|&i| ee.u32_at(ee.u32_at(actor + 0x2f4) + 0xb0 * i as u32 + 0xa4) == body)
}

/// The bone collision entries' feeds in `ee`: each bone collision entry's manager's `+0x30`.
fn feeds(ee: &Ee) -> Vec<u32> {
    let mut feeds: Vec<u32> = (0x0010_0000u32..0x0200_0000)
        .step_by(4)
        .filter(|&at| ee.u32_at(at) == PCC)
        .map(|at| ee.u32_at(ee.u32_at(at - 0x10) + 0x30))
        .collect();
    feeds.sort_unstable();
    feeds.dedup();
    feeds
}

/// **S476: the feed's spheres are in the body's slot order, not the rider file's.** In S325's nose
/// dive image every rider's feed (count `+0x10`, list `+0x14`) holds nine bone collisions, each one
/// of its body actor's collisions (count `+0x620`, list `+0x624`, file order; a collision's body
/// `+0x7c`). Read as bones, the actor's list is the rider file's order ([`Records::collisions`],
/// the disc's `mrider.phy`, where the disc is here) and the feed's is [`FEED_BONES`]. The port's
/// balls come out in the feed's order. Then the original runs 45 whole frames: each frame the body
/// rewrites the same nine pointers, held foot or not, and a foot that is off (`+0x04`) is fed with
/// its ground (`+0x18`) and scene (`+0x1c`) flags clear, so it gives the foot dust nothing.
#[test]
fn s476_feed_spheres_in_slot_order() {
    use super::draws::{neutral_pad, render, render_setup, whole_frame};
    use game_sim::dust::{FEED_BONES, balls_of};
    let kit = Kit::load().unwrap();
    let mut ee = Ee::from_ram(&kit.ctx("capture/s138/s325-nose76-2604.ee.bin")).unwrap();
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let actors = actors(&ee);
    let feeds = feeds(&ee);
    assert_eq!(feeds.len(), 4, "the four riders' feeds: {feeds:x?}");
    let list = |ee: &Ee, f: u32| -> Vec<u32> {
        assert_eq!(ee.u32_at(f + 0x10), 9, "feed {f:#x}'s count");
        (0..9).map(|k| ee.u32_at(ee.u32_at(f + 0x14) + 4 * k)).collect()
    };
    let mut file_order = None;
    for &f in &feeds {
        let spheres = list(&ee, f);
        let a = *actors
            .iter()
            .find(|&&a| (0..ee.u32_at(a + 0x620)).any(|k| ee.u32_at(ee.u32_at(a + 0x624) + 4 * k) == spheres[0]))
            .unwrap_or_else(|| panic!("feed {f:#x}'s actor"));
        let bones = |cs: &[u32]| -> Vec<usize> { cs.iter().map(|&c| bone_of(&ee, a, ee.u32_at(c + 0x7c)).expect("a collision's bone")).collect() };
        let actor_list: Vec<u32> = (0..ee.u32_at(a + 0x620)).map(|k| ee.u32_at(ee.u32_at(a + 0x624) + 4 * k)).collect();
        assert_eq!(bones(&spheres), FEED_BONES, "feed {f:#x}'s spheres' bones");
        let file = bones(&actor_list);
        assert_eq!(*file_order.get_or_insert_with(|| file.clone()), file, "actor {a:#x}'s collisions' bones");
    }
    let file_order = file_order.unwrap();
    match game_render::disc::Disc::open_configured() {
        Ok(mut disc) => {
            let name = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len());
            let bytes = disc.res(&name.expect("the rider archive")).unwrap().read_suffix("mrider.phy").unwrap();
            assert_eq!(game_sim::ragdoll::Records::read_bytes(&bytes).collisions, file_order, "the rider file's collisions against the actor's");
        }
        Err(why) => println!("no disc ({why}): the actor's order {file_order:?} not held against the rider file's"),
    }
    // The port's balls: each contact marked with its file index by its surface.
    let records = game_sim::ragdoll::Records { parts: vec![Default::default(); 18], joints: Vec::new(), collisions: file_order.clone() };
    let doll = game_sim::ragdoll::Ragdoll::new(records);
    let contacts: Vec<_> = (0..doll.collisions.len()).map(|k| game_sim::ragdoll_contact::Contact { surface: k as u8, ..Default::default() }).collect();
    let got: Vec<Option<usize>> = balls_of(&doll, &contacts).iter().map(|b| b.map(|b| file_order[b.surface as usize])).collect();
    assert_eq!(got, FEED_BONES.map(Some), "the port's balls' bones");
    // The frames: the same nine pointers each frame, and what an off foot hands over.
    let first: Vec<Vec<u32>> = feeds.iter().map(|&f| list(&ee, f)).collect();
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    render_setup(&mut ee);
    let (mut off, mut off_touching, mut wrecks) = (0, 0, 0);
    let mut seen = std::collections::HashMap::new();
    for t in 0..45 {
        neutral_pad(&mut ee);
        whole_frame(&mut ee, F(1.0 / 30.0), &mut clock);
        render(&mut ee);
        for (n, &f) in feeds.iter().enumerate() {
            assert_eq!(list(&ee, f), first[n], "frame {t}: feed {f:#x}'s spheres moved");
            wrecks += u32::from(ee.u32_at(ee.u32_at(f + 0x24)) != 0);
            for &c in &first[n][..2] {
                let now = (ee.u32_at(c + 4), ee.u32_at(c + 0x18), ee.u32_at(c + 0x1c), ee.u32_at(c + 0x14));
                let was = seen.insert(c, now);
                if was != Some(now) {
                    println!("s476 t{t} feed {f:#x} foot {c:#x}: on/ground/scene/touching {now:?} from {was:?}");
                }
                if ee.u32_at(c + 4) == 0 {
                    off += 1;
                    off_touching += u32::from(ee.u32_at(c + 0x18) != 0 || ee.u32_at(c + 0x1c) != 0);
                }
            }
        }
    }
    println!("s476: feet off and fed {off} times, {off_touching} of them still touching; wrecking feed-frames {wrecks}");
    // Measured: 283 off, none touching (the port clears every contact's flags each tick, on or off);
    // the wrecking rider's feet come on at frame 10 and touch at 11 to 12 and 30 to 42.
    assert!(off > 0 && wrecks > 0, "no foot off ({off}) or no feed wrecking ({wrecks}) in 45 frames");
    assert_eq!(off_touching, 0, "an off foot handed over a touch");
}

// ---------------------------------------------------------------------------------------------------
// S483 — the Sim's own wreck's collision dust, beside the original's
// ---------------------------------------------------------------------------------------------------

/// The ragdoll's kit as the shell builds it (tests/replay.rs's `the_ragdoll_kit`): the rider's body
/// and model off the disc, the rider file's parts and pins, the wreck bike's sphere and mesh; with
/// them the wreck bike's part and the rider's wreck set.
fn s483_ragdoll_kit() -> Option<(game_sim::ragdoll_ride::Kit, game_sim::phy::Part, game_sim::phy::Wrecks)> {
    use game_render::disc::Disc;
    let mut disc = Disc::open_configured().map_err(|why| skip(&why)).ok()?;
    let names: Vec<String> = disc.entries().iter().filter(|e| e.name.to_ascii_lowercase().ends_with(".res")).map(|e| e.name.clone()).collect();
    let rider_name = names.iter().filter(|n| n.to_ascii_lowercase().ends_with("thq1.res")).min_by_key(|n| n.len())?.clone();
    let res = disc.res(&rider_name).expect("the rider archive opens");
    let rider_file = res.read_suffix("mrider.phy").expect("the rider's archive carries mrider.phy");
    let mut body = game_render::rider::Body::load(
        vec![disc.res(&rider_name).expect("the rider archive opens")],
        "thq1",
        game_render::rider::Sex::Male,
        "",
        &[],
        game_sim::motion::Params::default(),
    )
    .expect("the rider's body loads");
    let model = game_render::psg::Model::parse(&res.read_suffix(&format!("{}.psg", body.geometry())).expect("its model")).expect("the model parses");
    let doll = body.ragdoll_rig(&model, &rider_file).expect("every part names a node");
    let wreck_bike = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbike.phy").ok())).expect("an archive carries wreckbike.phy");
    let mut kit = game_sim::ragdoll_ride::Kit::new(&rider_file, doll.rig);
    let col = names.iter().find_map(|n| disc.res(n).ok().and_then(|r| r.read_suffix("wreckbikef.col").ok()));
    kit.bike = col.as_deref().and_then(game_sim::ragdoll_ride::bike_sphere);
    kit.bike_mesh = col.as_deref().and_then(game_sim::ragdoll_scene::Bike::parse).map(std::sync::Arc::new);
    Some((kit, game_sim::phy::Part::read_bytes(&wreck_bike, "PhysicsPart1"), game_sim::phy::Wrecks::read_bytes(&rider_file)))
}

/// A rigid body as an image holds it (tests/replay.rs's `s252a_body`).
fn s483_body(ee: &Ee, b: u32) -> game_sim::biplane::Body {
    let q = |a: u32| Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(b + a + k)));
    game_sim::biplane::Body {
        enabled: ee.u32_at(b) != 0,
        inv_mass: F(ee.f32_at(b + 0x10)),
        damping: q(0x20),
        last_position: q(0x50),
        position: q(0x60),
        velocity: q(0x70),
        last_force: q(0x80),
        force: q(0x90),
        accel: q(0xa0),
        shock_peak: q(0xb0),
        shock: q(0xc0),
        step: q(0xf0),
        inv_inertia: q(0x120),
        ang_damping: q(0x130),
        last_quat: q(0x150),
        quat: q(0x170),
        cofg: q(0x180),
        moment: q(0x190),
        last_moment: q(0x1a0),
        rates: q(0x1b0),
        ang_accel: q(0x1c0),
        twist_peak: q(0x1d0),
        twist: q(0x1e0),
    }
}

/// **The human's body actor in `ee` as the port's riding body**, and the holds of its last riding
/// request carried into the frame of `m`, the bike's transform now: tests/replay.rs's `b19_riding`
/// (B19), word for word, on an [`Ee`] rather than a save state.
fn s483_riding(ee: &Ee, kit: &game_sim::ragdoll_ride::Kit, m: &game_sim::math::Mat4) -> (game_sim::ragdoll_riding::Riding, game_sim::ragdoll_riding::Holds) {
    use game_sim::math::{self, Mat4};
    use game_sim::rider_anim::Mode;
    let f = |at: u32| F(ee.f32_at(at));
    let quad = |at: u32| Vf::from_bits([0, 4, 8, 12].map(|k| ee.u32_at(at + k)));
    let mat = |at: u32| Mat4([0, 0x10, 0x20, 0x30].map(|k| quad(at + k)));
    let body_of = |a: u32, i: usize| ee.u32_at(ee.u32_at(a + 0x2f4) + 0xb0 * i as u32 + 0xa4);
    let bike = m.0[3];
    let near = |a: u32| {
        let p = quad(body_of(a, 0) + 0x60);
        (0..3).map(|l| (p[l] - bike[l]).powi(2)).sum::<f32>()
    };
    let a = actors(ee).into_iter().min_by(|&x, &y| near(x).total_cmp(&near(y))).expect("a rider body actor");
    assert!(near(a) < 100.0, "the nearest body actor's main bone is {} ft from the bike", near(a).sqrt());
    let mut riding = game_sim::ragdoll_riding::Riding::new(kit);
    let actor = &mut riding.actor;
    let n = actor.ragdoll.bones.len();
    let bone_of = |b: u32| (0..n).find(|&i| body_of(a, i) == b).expect("a bone's body");
    let r = &mut actor.ragdoll;
    for (i, bone) in r.bones.iter_mut().enumerate() {
        let b = body_of(a, i);
        bone.body = s483_body(ee, b);
        bone.mass = f(b + 0x0c);
        bone.inertia = quad(b + 0x110);
        bone.reference = quad(b + 0x160);
    }
    let joint_pin = |i: usize| ee.u32_at(ee.u32_at(a + 0x4c0) + 0x50 * i as u32 + 8);
    for (i, j) in r.joints.iter_mut().enumerate() {
        let pin = joint_pin(i);
        j.wrecked = ee.u32_at(pin + 0x70) != 0;
        j.max_angle = f(pin + 0xc4);
        j.spring = quad(pin + 0xd0);
        j.damping = quad(pin + 0xe0);
        j.critical = quad(pin + 0xf0);
    }
    let holds = [0x5d0, 0x5d4, 0x5d8, 0x5dc];
    let pins: Vec<u32> = holds.iter().map(|&h| ee.u32_at(a + h)).chain((0..r.joints.len()).map(joint_pin)).collect();
    assert_eq!(pins.len(), r.pins.len(), "the holds' and joints' pins");
    for (p, &at) in r.pins.iter_mut().zip(&pins) {
        let second = ee.u32_at(at + 0x6c);
        *p = game_sim::ragdoll::Pin {
            first: bone_of(ee.u32_at(at + 0x68)),
            second: (second != 0).then(|| bone_of(second)),
            anchors: [quad(at + 0x20), quad(at + 0x30)],
            target: quad(at + 0x40),
            target_velocity: quad(at + 0x50),
            lower: quad(at + 0x80),
            upper: quad(at + 0x90),
            hit: quad(at + 0xa0),
            sticky: quad(at + 0xb0),
            gains: quad(at + 0x100),
        };
    }
    for (i, c) in r.collisions.iter_mut().enumerate() {
        let k = ee.u32_at(ee.u32_at(a + 0x624) + 4 * i as u32);
        assert_eq!(c.bone, bone_of(ee.u32_at(k + 0x7c)), "collision {i}'s bone");
        c.enabled = ee.u32_at(k + 0x04) != 0;
        c.touching = ee.u32_at(k + 0x14) != 0;
    }
    r.holds = holds.map(|h| ee.u32_at(ee.u32_at(a + h) + 0x04) != 0);
    r.throwing = ee.u32_at(ee.u32_at(a + 0x62c)) != 0;
    r.entered = ee.u32_at(a + 0x49c) != 0;
    r.stiff = ee.u32_at(a + 0x2b4) != 0;
    r.riding = ee.u32_at(a + 0x2a4) != 0;
    r.off_bike = ee.u32_at(a + 0x6b0) != 0;
    r.flung = ee.u32_at(a + 0x6b8) != 0;
    r.settled = ee.u32_at(a + 0x6b4) != 0;
    r.roll = f(a + 0x490);
    r.pull = quad(a + 0x6e0);
    let mode = |at: u32| match ee.i32_at(at) {
        0 => Mode::Held,
        1 => Mode::Riding,
        2 => Mode::Wrecked,
        3 => Mode::Posed,
        4 => Mode::Gate,
        m => panic!("mode {m}"),
    };
    actor.mode = mode(a + 0x47c);
    actor.prev = mode(a + 0x480);
    actor.idle = ee.u32_at(a + 0xa2c) != 0;
    actor.look_pose = ee.i32_at(a + 0x9f0);
    actor.clip = ee.i32_at(a + 0x8f4);
    actor.touching = ee.u32_at(a + 0x48c) != 0;
    actor.fade = game_sim::ragdoll_body::Fade { length: f(a + 0x644), left: f(a + 0x648) };
    actor.foot_fade = [f(a + 0x638), f(a + 0x63c)];
    actor.pose_damping = quad(a + 0x660);
    actor.hold_targets = std::array::from_fn(|k| quad(a + 0x4d0 + 0x10 * k as u32));
    actor.hold_velocity = quad(a + 0x530);
    actor.bike_frame = mat(a + 0x580);
    actor.placed = ee.u32_at(a + 0x2ac) != 0;
    actor.active = ee.u32_at(a + 0x2b0) != 0;
    actor.repose = ee.u32_at(a + 0x2a8) != 0;
    actor.torques = ee.u32_at(a + 0x2b8) != 0;
    actor.gravity_scale = f(a + 0x2d0);
    actor.impact = quad(a + 0x6f0);
    let req = ee.u32_at(a + 0x488);
    let inv = math::inverse(m).expect("the bike's transform");
    let local = |at: u32| math::transform_point(&inv, quad(at));
    let holds = game_sim::ragdoll_riding::Holds {
        root: math::mul(&mat(req + 0x50), &inv),
        hands: [local(req + 0xb0), local(req + 0xc0)],
        feet: [local(req + 0xd0), local(req + 0xe0)],
    };
    (riding, holds)
}

/// The ticks the Sim's wreck is held for: S325's 40 (tests/replay.rs's `S325_DROPS`).
const S483_TICKS: usize = 40;

/// The last tick every collision entry's draws agree with the original's, on S325's nose dive:
/// the bone collisions' first draws (2618), before the ragdoll drifts (R24) and they part (2619).
const S483_BONES_AGREE: u64 = 2618;

/// **S483: the Sim's own wreck draws the collision dust as the original's does, by the tick.** S325's
/// nose dive (`capture/s138/s325-nose76-2604`) twice:
///
/// - **The original**, as S422 runs it: its whole frames in the interpreter, the human's collision
///   models and bone collision entries (its manager, rider `+0x1490`) each counted per tick by the
///   rands its update makes, and the frame's camera kept.
/// - **The Sim**: the human's bike built and seeded off the image as C33's riders are (race4.rs),
///   on race125p with the bike's body rays, the wreck's part and set and the ragdoll's kit off the
///   disc, its riding body the image's ([`s483_riding`], replay.rs's B19 seed); free09 off the disc
///   as the world (S323). Its dust is the human's collision entries seeded off the image (S422's
///   seed, the body's bone collisions as the image left them), and each tick the shell's camera is
///   the original's for the tick.
///
/// Held every tick: the wreck's reason (both 1 from 2614) and the snapshot's wreck flag the manager
/// reads (from 2615, [`RiderDust::wrecked`]), and the collision models' draws, entry by entry —
/// they read the wreck bike, which keeps with the original's. The bone collisions read the ragdoll,
/// which drifts from the original's (R24): every entry agrees through [`S483_BONES_AGREE`], the
/// bone collisions' first draws among them, and after it only the counts are printed.
///
/// Measured: collision models 54 draws on both sides, every tick equal; bone collisions 156 against
/// the Sim's 114, equal through 2618 and on 2621, 2630 and 2640 after it.
#[test]
fn s483_the_sims_wreck_draws_collision_dust_as_the_original() {
    use super::draws::{neutral_pad, render, render_setup, through, whole_frame};
    use super::lane::s323_free09;
    use super::race4::{Bikes, c33_bike};
    use game_sim::Sim;
    use game_sim::dust::{Dust, RiderDust};
    use game_sim::eco::Wooded;
    use game_sim::rider::Tables;
    use game_sim::track::{self, Terrain};
    use game_sim::world::{Ground, Place as WorldPlace};
    use std::sync::Arc;
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let state = kit.ctx("capture/s138/s325-nose76-2604.ee.bin");
    if !state.exists() {
        return skip(&format!("no {}: S325's capture", state.display()));
    }
    let mut ee = Ee::from_ram(&state).unwrap();
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let text = std::fs::read_to_string(kit.ctx("capture/s138/s325-nose76.csv")).unwrap();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
    let (fc, dtc) = (col("frame_counter"), col("dt"));
    let rows: Vec<(u64, F)> = lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .map(|c| (c[fc].parse().unwrap(), F(c[dtc].parse::<f64>().unwrap() as f32)))
        .filter(|r| r.0 >= 2604)
        .collect();
    // The human's rider, down the game manager's list as tests/replay.rs's save states find it.
    let mut human = ee.u32_at(addr("game_manager").unwrap());
    for hop in [0x34, 0x34, 0x8, 0x8, 0x0] {
        human = ee.u32_at(human + hop);
    }
    assert_eq!(ee.u32_at(human), 2, "the chain lands on a rider");
    let m = ee.u32_at(human + 0x1490);
    let entries: Vec<u32> = (0..ee.u32_at(m + 0x34)).map(|i| ee.u32_at(ee.u32_at(m + 0x3c) + 4 * i)).filter(|&e| matches!(ee.u32_at(e + 0x10), CM | PCC)).collect();
    let kinds: Vec<usize> = entries.iter().map(|&e| usize::from(ee.u32_at(e + 0x10) == PCC)).collect();
    assert!(kinds.contains(&0) && kinds.contains(&1), "the human's manager holds both kinds: {entries:x?}");
    // The Sim's seed, taken before the original moves the image on.
    let seeded: Vec<(Entry, Feed, Touches)> = entries.iter().map(|&e| seed(&ee, e)).collect();

    // The original: each tick's draws per human entry, and the tick's camera.
    let rand_state = addr("rand_state").unwrap();
    let draws: Rc<RefCell<u32>> = Rc::default();
    let d = Rc::clone(&draws);
    ee.hooks.insert(
        addr("rand").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let n = super::sim::s12e_u64(ee, rand_state).wrapping_mul(0x5851_f42d_4c95_7f2d).wrapping_add(1);
            ee.write(rand_state, 8, u128::from(n));
            ee.set(2, (n >> 32) & 0x7fff_ffff);
            *d.borrow_mut() += 1;
        }),
    );
    let now: Rc<RefCell<Vec<u32>>> = Rc::new(RefCell::new(vec![0; entries.len()]));
    for name in ["vehicle_fx_collision", "vehicle_fx_pcc_update"] {
        let (d, now, entries) = (Rc::clone(&draws), Rc::clone(&now), entries.clone());
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee: &mut Ee| {
                let e = ee.r(4) as u32;
                let before = *d.borrow();
                through(ee);
                if let Some(k) = entries.iter().position(|&x| x == e) {
                    now.borrow_mut()[k] += *d.borrow() - before;
                }
            }),
        );
    }
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    render_setup(&mut ee);
    let mut theirs: Vec<Vec<u32>> = Vec::new();
    let mut views = Vec::new();
    let mut wrecking = Vec::new();
    let mut reasons = Vec::new();
    let fed = ee.u32_at(m + 0x30);
    for r in rows.windows(2).take(S483_TICKS) {
        now.borrow_mut().fill(0);
        neutral_pad(&mut ee);
        whole_frame(&mut ee, r[0].1, &mut clock);
        render(&mut ee);
        let cam = ee.u32_at(ee.u32_at(addr("display_object").unwrap()));
        views.push(View { at: vf(&ee, cam + 0x140), forward: vf(&ee, cam + 0x150) });
        theirs.push(now.borrow().clone());
        wrecking.push(ee.u32_at(ee.u32_at(fed + 0x24)) != 0);
        reasons.push(ee.u32_at(human + 0x748));
    }

    // The Sim: the human's bike off the image, its wreck and ragdoll off the disc, on free09.
    let ee = Ee::from_ram(&state).unwrap();
    let Some((terrain_bytes, scene, props, trees)) = s323_free09(&kit, &ee) else { return };
    let terrain = Terrain::parse(&terrain_bytes).expect("free09's terrain");
    let mut world = Wooded { world: WorldPlace { ground: Ground::new(&terrain, &scene.terrain), props }, trees };
    let tables = Arc::new(Tables::build());
    let Some(mut bikes) = Bikes::open() else { return };
    let Some((doll_kit, part, wrecks)) = s483_ragdoll_kit() else { return };
    let name = track::physics_file(track::Engine::Cc125, false, track::Tuning::P);
    let phy = bikes.phy(&name, &tables);
    let mut bike = c33_bike(&ee, human, &phy, &name, &bikes.col);
    bike.carries_wreck(part, wrecks, game_sim::ride::WRECK_INERTIA);
    bike.carries_ragdoll(Arc::new(doll_kit));
    let mut sim = Sim::with_rider(0, Arc::clone(&tables), phy, bike);
    let doll_kit = sim.rider.ragdoll_kit.clone().expect("the kit");
    let (riding, holds) = s483_riding(&ee, &doll_kit, &sim.rider.ride.transform());
    // The vehicle animation's clip as the actor last read it (B22), as replay.rs's B19 ride holds it.
    sim.rider.body.veh.state = riding.actor.clip;
    sim.rider.riding = Some(Box::new(riding));
    sim.rider.holds = Some(Arc::new(holds));
    let balls = seeded.iter().find(|s| matches!(s.0.kind, game_sim::dust::Kind::Spheres { .. })).map(|s| s.2.balls.clone()).unwrap_or_default();
    sim.dust = Some(Box::new(Dust {
        riders: vec![RiderDust { entries: seeded.iter().map(|s| s.0).collect(), feed: seeded[0].1, balls, wrecked: ee.u32_at(human + 0x748) != 0, ..RiderDust::default() }],
        view: None,
    }));
    let mut ours: Vec<Vec<u32>> = Vec::new();
    let mut down = Vec::new();
    for (t, r) in rows.windows(2).take(S483_TICKS).enumerate() {
        sim.dust.as_mut().unwrap().view = Some(views[t]);
        sim.frame(r[0].1, &Sim::reports(game_sim::Input::default()), &mut world);
        ours.push(sim.dust.as_ref().unwrap().riders[0].entries.iter().map(|e| u32::from(e.draws)).collect());
        down.push((sim.dust.as_ref().unwrap().riders[0].feed.wrecking, sim.rider.ragdoll.is_some(), sim.rider.state.wreck.map_or(0, |w| u32::from(w.0))));
    }

    // Per tick and kind: the original's draws and the Sim's.
    let by_kind = |row: &[u32], k: usize| row.iter().zip(&kinds).filter(|(_, x)| **x == k).map(|(d, _)| d).sum::<u32>();
    let (mut agree, mut part_ways) = (Vec::new(), Vec::new());
    let mut totals = [[0u32; 2]; 2];
    for t in 0..theirs.len() {
        let tick = rows[t + 1].0;
        let pair = [0, 1].map(|k| (by_kind(&theirs[t], k), by_kind(&ours[t], k)));
        for k in 0..2 {
            totals[k][0] += pair[k].0;
            totals[k][1] += pair[k].1;
        }
        println!(
            "s483 tick {tick}: reason {} / {}, snapshot wrecking {} / {}, ragdoll {}: collision models {} vs {}, bone collisions {} vs {}; per entry {:?} vs {:?}",
            reasons[t], down[t].2, wrecking[t], down[t].0, down[t].1, pair[0].0, pair[0].1, pair[1].0, pair[1].1, theirs[t], ours[t]
        );
        if theirs[t] == ours[t] { agree.push(tick) } else { part_ways.push(tick) }
        assert_eq!((reasons[t], wrecking[t]), (down[t].2, down[t].0), "tick {tick}: the wreck's reason and the snapshot's wreck flag");
        let models = |row: &[u32]| row.iter().zip(&kinds).filter(|(_, k)| **k == 0).map(|(d, _)| *d).collect::<Vec<_>>();
        assert_eq!(models(&ours[t]), models(&theirs[t]), "tick {tick}: the collision models' draws");
    }
    println!("s483: draws (original, Sim): collision models {:?}, bone collisions {:?}", totals[0], totals[1]);
    println!("s483: ticks agreeing on every entry {agree:?}; parting {part_ways:?}");
    // Every entry agrees until the ragdoll has drifted (R24): through S483_BONES_AGREE, which
    // takes in the bone collisions' first draws.
    let through = rows.iter().skip(1).position(|r| r.0 == S483_BONES_AGREE).unwrap();
    assert!(theirs[..=through] == ours[..=through], "the ticks through {S483_BONES_AGREE} part ways");
    assert!(by_kind(&theirs[through], 1) > 0, "the bone collisions draw by {S483_BONES_AGREE}");
    assert!(totals[1][1] > 0, "the Sim's bone collisions drew: {totals:?}");
}

/// **S483: the wreck word the dust's manager reads is the reason as the frame before's verdict left
/// it.** The manager's feed (`+0x24`) points at the rider's `+0x868`, the state pack's copy of the
/// wreck reason `+0x748`. The pack runs before the frame's wreck verdict, so across a verdict the
/// copy holds the frame before's reason; and the manager reads the copy the frame before packed. S325's
/// nose dive (reason 1 on the ground) and its flat drop (reason 5), each run as the original's whole
/// frames from its image, and its dive past the airborne limit, already down (reason 8) in its
/// image: after every frame `+0x868` is the reason after the frame before, and each drop is down
/// within its frames.
#[test]
fn s483_the_feeds_wreck_word_is_the_frame_befores_reason() {
    use super::draws::{neutral_pad, render, render_setup, whole_frame};
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    for (image, frames) in [("s325-nose76-2604", 20), ("s325-dive-2515", 10), ("s325-flat-2643", 30)] {
        let at = kit.ctx(&format!("capture/s138/{image}.ee.bin"));
        if !at.exists() {
            println!("no {}: S325's capture", at.display());
            continue;
        }
        let mut ee = Ee::from_ram(&at).unwrap();
        ee.set(28, u64::from(addr("boot_gp").unwrap()));
        let mut human = ee.u32_at(addr("game_manager").unwrap());
        for hop in [0x34, 0x34, 0x8, 0x8, 0x0] {
            human = ee.u32_at(human + hop);
        }
        let fed = ee.u32_at(ee.u32_at(human + 0x1490) + 0x30);
        assert_eq!(ee.u32_at(fed + 0x24), human + 0x868, "{image}: the feed's wreck word");
        let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
        render_setup(&mut ee);
        let mut was = ee.u32_at(human + 0x748);
        let mut seen = Vec::new();
        for t in 0..frames {
            neutral_pad(&mut ee);
            whole_frame(&mut ee, F(1.0 / 30.0), &mut clock);
            render(&mut ee);
            let (reason, copy) = (ee.u32_at(human + 0x748), ee.u32_at(human + 0x868));
            seen.push((reason, copy));
            assert_eq!(copy, was, "{image} frame {t}: the copy against the reason before the frame");
            was = reason;
        }
        println!("s483 {image}: (reason, copy) {seen:?}");
        assert!(seen.iter().any(|s| s.0 != 0 && s.1 != 0), "{image} wrecks within {frames} frames");
    }
}

/// The emitter pass's entries' vtables: `Exhaust`, `Generic` and `Spray`.
const PASS: [u32; 3] = [0x0045_e780, 0x0045_e6f0, 0x0045_e720];

/// **S477: the emitter pass's emits skip a culled emitter.** The S422 run of `s325-nose76` (four
/// riders, 45 frames from 2604), the original interpreted whole. Each emit (`a0` the emitter, `f12`
/// the frame) on an exhaust, generic or spray entry's emitter is seeded into the port's emitter just
/// before it runs (as `seed` reads one); the port's emit on the point the original's node put it at
/// ([`Emitter::emit_on`]) must make the original's rands and leave its switch and points. The bike's
/// `.emt`s all set `UseViewDependentCull`, and some emits must find their emitter switched on and
/// culled.
#[test]
fn s477_the_emitter_pass_skips_a_culled_emitter() {
    use super::draws::{neutral_pad, render, render_setup, through, whole_frame};
    let kit = Kit::load().unwrap();
    let mut ee = Ee::from_ram(&kit.ctx("capture/s138/s325-nose76-2604.ee.bin")).unwrap();
    ee.set(28, u64::from(addr("boot_gp").unwrap()));
    let text = std::fs::read_to_string(kit.ctx("capture/s138/s325-nose76.csv")).unwrap();
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).unwrap();
    let (fc, dtc) = (col("frame_counter"), col("dt"));
    let rows: Vec<(u64, F)> = lines
        .map(|l| l.split(',').collect::<Vec<_>>())
        .map(|c| (c[fc].parse().unwrap(), F(c[dtc].parse::<f64>().unwrap() as f32)))
        .filter(|r| r.0 >= 2604)
        .collect();
    let rand_state = addr("rand_state").unwrap();
    let draws: Rc<RefCell<u32>> = Rc::default();
    let d = Rc::clone(&draws);
    ee.hooks.insert(
        addr("rand").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let n = super::sim::s12e_u64(ee, rand_state).wrapping_mul(0x5851_f42d_4c95_7f2d).wrapping_add(1);
            ee.write(rand_state, 8, u128::from(n));
            ee.set(2, (n >> 32) & 0x7fff_ffff);
            *d.borrow_mut() += 1;
        }),
    );
    let mut pass = HashSet::new();
    let mut culls = HashSet::new();
    for at in (0x0010_0000u32..0x0200_0000).step_by(4) {
        if PASS.contains(&ee.u32_at(at)) {
            let em = ee.u32_at(at + 4);
            pass.insert(em);
            if ee.u32_at(em + 0x244) != 0 {
                culls.insert(em);
            }
        }
    }
    assert!(!pass.is_empty() && culls == pass, "every pass emitter has a view cull: {} of {}", culls.len(), pass.len());
    let wrong: Rc<RefCell<Vec<String>>> = Rc::default();
    // Emits seen; with the emitter on and culled; that drew.
    let tally: Rc<RefCell<[u32; 3]>> = Rc::default();
    let tick: Rc<RefCell<u64>> = Rc::default();
    let (w, t, now, dr) = (Rc::clone(&wrong), Rc::clone(&tally), Rc::clone(&tick), Rc::clone(&draws));
    ee.hooks.insert(
        addr("emit").unwrap(),
        Box::new(move |ee: &mut Ee| {
            let em = ee.r(4) as u32;
            if !pass.contains(&em) {
                through(ee);
                return;
            }
            let dt = F(ee.f(12));
            let switch = ee.u8_at(em + 0x240);
            let mut port = Emitter {
                on: switch & 4 != 0,
                just_on: switch & 2 != 0,
                min_speed: F(ee.f32_at(em + 0x3c4)),
                place: Place::Chassis(Vf([0.0; 4])),
                pos: vf(ee, em + 0x40),
                prev: vf(ee, em + 0x60),
                cull: Some(Cull { far: F(ee.f32_at(em + 0x24c)), directional: F(ee.f32_at(em + 0x250)) }),
                culled: ee.u32_at(em + 0x248) != 0 && ee.u32_at(0x0041_f788) != 0,
            };
            let node = ee.u32_at(em + 0xa4) != 0;
            let before = *dr.borrow();
            through(ee);
            let got = *dr.borrow() - before;
            let on_culled = port.on && !port.just_on && port.culled;
            let want = u32::from(port.emit_on(node.then(|| vf(ee, em + 0x40)), dt));
            let switch = ee.u8_at(em + 0x240);
            let after = (switch & 4 != 0, switch & 2 != 0, vf(ee, em + 0x40).to_bits(), vf(ee, em + 0x60).to_bits());
            let ours = (port.on, port.just_on, port.pos.to_bits(), port.prev.to_bits());
            if got != want || after != ours {
                w.borrow_mut().push(format!("tick {} emitter {em:#x}: draws {got} vs the port's {want}, emitter {after:x?} vs {ours:x?}", *now.borrow()));
            }
            let mut t = t.borrow_mut();
            t[0] += 1;
            t[1] += u32::from(on_culled);
            t[2] += u32::from(got > 0);
        }),
    );
    let mut clock = f64::from(ee.u32_at(addr("wall_ticks").unwrap())) * 100.0;
    render_setup(&mut ee);
    for r in rows.windows(2).take(45) {
        *tick.borrow_mut() = r[1].0;
        neutral_pad(&mut ee);
        whole_frame(&mut ee, r[0].1, &mut clock);
        render(&mut ee);
    }
    let wrong = wrong.borrow();
    assert!(wrong.is_empty(), "{} differences, the first:\n{}", wrong.len(), wrong.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    let t = *tally.borrow();
    println!("pass emits {}, on and culled {}, drew {}", t[0], t[1], t[2]);
    assert!(t[1] > 0 && t[2] > 0, "emits found their emitter on and culled, and others drew: {t:?}");
}
