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
