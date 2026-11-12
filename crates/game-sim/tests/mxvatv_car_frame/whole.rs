// --- MD106c: the whole frame off the image -----------------------------------------------------------

use super::*;
use game_sim::atv_ground;
use game_sim::noise::SmoothNoise;
use std::collections::BTreeSet;

/// Desert Oasis's terrain file, out of `mc02.res` (MD37a's copy; MD39a's buggy spawns there).
fn terrain_file() -> Option<Vec<u8>> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let res = kit.ctx("mxvatv-ps2/md37a").join("mc02.res");
    if !res.exists() {
        skip(&format!("{} is not there: Desert Oasis's mc02.res off MX vs. ATV's disc (Env/MC02)", res.display()));
        return None;
    }
    let file = std::fs::read(&res).unwrap();
    let archive = game_sim::archive::Res::parse(&file).unwrap();
    Some(archive.read(&file, archive.find("Env\\mc02\\MC02.ps2.trn").expect("mc02.res holds MC02.ps2.trn")).unwrap())
}

/// The shared smooth noise's tables, the word that says they are not built yet, and the knot
/// each sample leaves (MD125 faked the noise; here it is [`SmoothNoise::at_accumulated`] on the
/// image's tables, as `mxvatv_bike_ride`'s).
const NOISE_PERM: u32 = 0x003f_3540;
const NOISE_G1: u32 = 0x003f_6580;
const NOISE_UNBUILT: u32 = 0x0038_f708;
const NOISE_KNOT: u32 = 0x003f_6d88;
/// The sort's ray probe's flags byte and its struck bit (MD127's [`sort::FLAGS`]).
const STRUCK: u8 = 0x20;

/// The port's world round the car: both ground queries answered by [`atv_ground::query`] on
/// Desert Oasis, the noise by [`SmoothNoise::at_accumulated`] (keeping the knot the original
/// leaves), and every other call it is handed listed.
struct Oasis<'t> {
    terrain: &'t atv_ground::Terrain<'t>,
    noise: SmoothNoise,
    knot: Option<u32>,
    /// Each query, by tire (none for the one under the body), and the point asked.
    asked: Vec<(Option<usize>, [u32; 4])>,
    rest: Vec<Call>,
}

impl Steps for Oasis<'_> {
    fn call(&mut self, c: Call, car: &mut Car, _body: &mut Body) {
        match c {
            Call::GroundUnder(at) => {
                let mut point = at;
                atv_ground::query(self.terrain, &mut point, Some(&mut car.ground_normal), None);
                self.asked.push((None, at.to_bits()));
            }
            Call::TireGround(k, at) => {
                let t = &mut car.tires[k];
                let (mut point, mut normal) = (at, Vf::ZERO);
                atv_ground::query(self.terrain, &mut point, Some(&mut normal), Some(&mut t.surface));
                t.query = (point, normal);
                self.asked.push((Some(k), at.to_bits()));
            }
            Call::Noise(at) => {
                car.noise = self.noise.at_accumulated(at);
                self.knot = Some((F(4096.0) + at).to_i32() as u32);
            }
            c => self.rest.push(c),
        }
    }
}

/// One ground query the original made: its return address, world, record and the point asked.
type Asked = (u32, u32, u32, [u32; 4]);

/// The ground query hooked on `ee`, answered by [`atv_ground::query`] on `terrain` (with the
/// caller's normal buffer, and its record byte, none when the record is null), each logged.
fn hook_ground(ee: &mut Ee, terrain: &'static atv_ground::Terrain<'static>, asked: Shared<Vec<Asked>>) {
    ee.hooks.insert(GROUND_QUERY, Box::new(move |ee| {
        let (w, pt, n, rec) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32, ee.r(8) as u32);
        assert_eq!(ee.r(7), 0, "the ground query's fourth argument");
        let (mut point, mut normal) = (get_v(ee, pt), get_v(ee, n));
        asked.borrow_mut().push((ee.r(31) as u32, w, rec, point.to_bits()));
        let mut record = if rec != 0 { ee.u8_at(rec) } else { 0 };
        atv_ground::query(terrain, &mut point, Some(&mut normal), (rec != 0).then_some(&mut record));
        put_v(ee, pt, point);
        put_v(ee, n, normal);
        if rec != 0 {
            ee.write(rec, 1, u128::from(record));
        }
    }));
}

/// Every byte `ee`'s last call wrote, outside the interpreter's stack.
fn wrote(ee: &Ee) -> BTreeSet<u32> {
    let stack = ee.stack - 0x40000..ee.stack + 0x1000;
    ee.written_ranges().into_iter().filter(|(f, _)| !stack.contains(f)).flat_map(|(f, t)| f..t).collect()
}

/// Bytes as `(start, end)` runs, for a message.
fn runs(bytes: impl IntoIterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for x in bytes {
        match out.last_mut() {
            Some(r) if r.1 == x => r.1 = x + 1,
            _ => out.push((x, x + 1)),
        }
    }
    out
}

/// **The buggy's whole frame is the original's** (MD106c): SLUS-21104's world step over MD39a's
/// buggy image at 1/30 s with nothing hooked but the ground query, which is answered on both
/// sides by [`atv_ground`] on Desert Oasis, against [`atv_car::frame`] on the image read whole
/// (the car, its body, its engine and tires, the pieces' keys and state, the vehicle's words) with
/// MX Unleashed's [`Pieces`] and SLUS-21104's [`atv_car::Own`] answering every call they port.
/// Every word of the car, body, engine and tires (and the vehicle's, the world's and the scene
/// node's words [`atv_car::Vehicle`] holds) equals the original's, and so does the noise's knot.
///
/// Then every byte the original's frame wrote is accounted for, from three more runs: the car's
/// listeners alone (the preforce list cut to its first entry), the same with the calls the port
/// hands out hooked to do nothing, and the other three preforce listeners (the idle buggy, two
/// other vehicles) called alone in the list's order. The whole frame's RAM is the car's run with
/// the other three's laid over it; the other three write nothing the comparison reads; with the
/// hand-outs doing nothing the car writes only words the comparison reads (and the noise's knot),
/// and the same values; and what the hand-outs write is none of the car's.
#[test]
fn the_buggys_whole_frame_is_the_originals() {
    let Some(pre) = image() else { return };
    let Some(trn) = terrain_file() else { return };
    let trn: &'static [u8] = Box::leak(trn.into_boxed_slice());
    let map: &'static game_sim::track::Terrain<'static> = Box::leak(Box::new(game_sim::track::Terrain::parse(trn).unwrap()));
    let terrain: &'static atv_ground::Terrain<'static> = Box::leak(Box::new(atv_ground::Terrain::of(map)));
    let p = ptrs(&pre);
    let o = own_ptrs(&pre, &p);
    // The terrain object every query names is the one [`atv_ground::Terrain::of`] builds.
    assert!(p.tire_worlds.iter().all(|&w| w == p.ground_world), "the tires' grounds are the car's");
    let g = p.ground_world;
    let t = terrain;
    assert_eq!(
        [get_v(&pre, g + 0x30).to_bits()[0], get_v(&pre, g + 0x30).to_bits()[2], get_v(&pre, g + 0x40).to_bits()[0], get_v(&pre, g + 0x40).to_bits()[2]],
        [t.min.to_bits()[0], t.min.to_bits()[2], t.max.to_bits()[0], t.max.to_bits()[2]],
        "the terrain's box"
    );
    assert_eq!(
        [pre.u32_at(g + 0x50), pre.u32_at(g + 0x90), pre.u32_at(g + 0x788), pre.u32_at(g + 0x8b8), pre.u32_at(g + 0x770)],
        [t.margin.to_bits(), t.height_scale.to_bits(), t.per_sample.to_bits(), u32::from(t.coarse), u32::from(t.split_one_way)],
        "the terrain object's words"
    );
    assert_eq!(pre.u32_at(NOISE_UNBUILT), 0, "the noise's tables are built");
    // The preforce list: the car, the idle buggy (same class), two other vehicles.
    let list = pre.u32_at(WORLD + 0xa0);
    let listeners: Vec<u32> = (0..pre.u32_at(WORLD + 0x98)).map(|k| pre.u32_at(list + 4 * k)).collect();
    assert_eq!(listeners.len(), 4, "the preforce list's length");
    assert_eq!(listeners[0], CAR + 0x30, "the car's PreProcess leads the list");
    let dt = F(1.0 / 30.0);
    let record_tell = pre.u32_at(pre.u32_at(p.record + 4) + 0x88);
    let handed_out = [SORT_PLACE, SORT_SEARCH, record_tell, SUBTREE_OUT_OF_DATE, SPATIAL_UPDATE, NODE_BOUNDS, COLLISION_SYNC, EDGE_TEST];
    let step = |car_alone: bool, nothing_handed: bool| {
        let mut ee = image().unwrap();
        let asked: Shared<Vec<Asked>> = Rc::default();
        hook_ground(&mut ee, terrain, asked.clone());
        if car_alone {
            ee.set_u32(WORLD + 0x98, 1);
        }
        if nothing_handed {
            for f in handed_out {
                ee.hooks.insert(f, Box::new(|ee| ee.set(2, 0)));
            }
        }
        ee.set(28, GP);
        ee.call(WORLD_STEP, &[u64::from(WORLD)], &[dt.0], 50_000_000);
        let w = wrote(&ee);
        let a = asked.borrow().clone();
        (ee, w, a)
    };

    // The original: the whole frame; the car's listeners alone; and without what they hand out.
    let (ee, wa, asked) = step(false, false);
    let (alone, wb, asked_alone) = step(true, false);
    let (bare, wbare, _) = step(true, true);
    // The other three preforce listeners alone, in the list's order.
    let mut others = image().unwrap();
    let asked_others: Shared<Vec<Asked>> = Rc::default();
    hook_ground(&mut others, terrain, asked_others.clone());
    let mut wo = BTreeSet::new();
    for &e in &listeners[1..] {
        others.set(28, GP);
        others.call(pre.u32_at(pre.u32_at(e) + 8), &[u64::from(e)], &[dt.0], 50_000_000);
        wo.extend(wrote(&others));
    }

    // The port, on the image read whole.
    let mut car = read_car(&pre, &p);
    let mut body = seed::body(&|a| pre.u32_at(a), p.body);
    let mut v = read_vehicle(&pre, &p, &o);
    let m = read_mx(&pre, &p);
    assert!(m.keys.engine.is_some(), "the image's engine keys");
    let noise = SmoothNoise::from_tables(&std::array::from_fn(|k| pre.u32_at(NOISE_PERM + 4 * k as u32)), &std::array::from_fn(|k| F(pre.f32_at(NOISE_G1 + 4 * k as u32))));
    let world = World { iterations: 1, rate: F(115.0) };
    let table = AcosTable::build();
    let oasis = Oasis { terrain, noise, knot: None, asked: Vec::new(), rest: Vec::new() };
    let mut pieces = Pieces { keys: m.keys(), state: m.state, rest: oasis };
    atv_car::frame(&world, &mut car, &mut body, dt, &table, &mut atv_car::Own { v: &mut v, rest: &mut pieces });
    let pm = Mx { keys: Keys { tires: pieces.keys.tires.clone(), ..m.keys.clone() }, state: pieces.state, ..m };
    let oasis = pieces.rest;
    let knot = (NOISE_KNOT, oasis.knot.expect("the engine asked the noise"));
    let after: Vec<(u32, u32)> =
        [car_words(&car, &p), seed::body_words(&body, p.body), mx_words(&pm, &p), vehicle_words(&pre, &v, &p, &o), vec![knot]].concat();
    let show = |ee: &Ee, words: &[(u32, u32)]| -> Vec<String> {
        words
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x} ({}), original {:#010x} ({}), was {:#010x}", f32::from_bits(x), ee.u32_at(a), ee.f32_at(a), pre.u32_at(a)))
            .collect()
    };
    let bad = show(&ee, &after);
    assert!(bad.is_empty(), "{} of {} words differ from the original's frame:\n{}", bad.len(), after.len(), bad.join("\n"));

    // The same ground queries, in the same order, at the same points.
    let tire_of = |rec: u32| p.tires.iter().position(|&t| t + at::SURFACE == rec);
    let theirs: Vec<(Option<usize>, [u32; 4])> = asked
        .iter()
        .filter(|&&(_, _, rec, _)| rec == 0 || tire_of(rec).is_some())
        .map(|&(ra, w, rec, at)| {
            assert_eq!(w, p.ground_world, "the car's ground query's world");
            assert_eq!(ra, if rec == 0 { POST_GROUND } else { PROBE_GROUND }, "a car's ground query from elsewhere");
            (tire_of(rec), at)
        })
        .collect();
    assert_eq!(oasis.asked, theirs, "the car's ground queries");
    assert_eq!(theirs.len(), 5, "the four probes and the one under the body");
    assert_eq!(asked_alone.len(), theirs.len(), "the car's listeners alone ask the car's queries");
    assert_eq!(asked.len() - theirs.len(), asked_others.borrow().len(), "the other listeners' queries");

    // What the port handed out, and what the original's own code answered: the sort's search
    // struck nothing (the port's `SortSearch` leaves [`Car::sort`] empty); the map-edge test
    // said inside (the port's [`atv_car::Vehicle::out_of_bounds`] stays false), so the car rides.
    let named: Vec<String> = oasis.rest.iter().map(|c| format!("{c:?}")).collect();
    assert_eq!(
        named,
        ["SortPlace", "SortSearch", "GravityRecord", "GravityRecord", "GravityRecord", "GravityRecord", "SubtreeOutOfDate", "SpatialUpdate", "NodeBounds", "CollisionSync", "OutOfBounds"],
        "handed out"
    );
    assert_eq!(ee.u8_at(p.sort_probe + sort::FLAGS) & STRUCK, 0, "the sort's search struck something");
    assert_eq!(ee.u32_at(CAR + at::STATE), atv_car::RIDING, "the car wrecked");

    // Every byte the frame wrote is the car's run's or the other three's, with its value; the
    // bytes both write are the search's and the database's shared words.
    let mut both = Vec::new();
    for &x in wa.iter().chain(&wb).chain(&wo) {
        let (got, b, c) = (ee.u8_at(x), alone.u8_at(x), others.u8_at(x));
        match (wb.contains(&x), wo.contains(&x)) {
            (true, false) => assert_eq!(got, b, "{x:#x}: the frame's byte is not the car's"),
            (false, true) => assert_eq!(got, c, "{x:#x}: the frame's byte is not the other listeners'"),
            (true, true) => {
                assert!(got == b || got == c, "{x:#x}: written by both, the frame's byte is neither's");
                both.push(x);
            }
            (false, false) => panic!("{x:#x}: written by neither alone"),
        }
    }
    // The other three touch nothing the comparison reads, nor the car's blocks.
    let compared: BTreeSet<u32> = after.iter().flat_map(|&(a, _)| a..a + 4).collect();
    let mut blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record, 0x200), (p.engine, engine::SIZE), (o.world, 0xb0)]
        .into_iter()
        .chain(p.tires.map(|t| (t, tire::SIZE)))
        .collect();
    blocks.extend(o.nodes.iter().map(|&n| (n, 0x110)));
    let in_blocks = |x: u32| blocks.iter().any(|&(b, n)| (b..b + n).contains(&x));
    let touched: Vec<u32> = wo.iter().copied().filter(|&x| compared.contains(&x) || in_blocks(x)).collect();
    assert!(touched.is_empty(), "the other listeners wrote the car's: {:x?}", runs(touched));
    // The car with nothing handed out writes only what the comparison reads, and the same.
    let unread: Vec<u32> = wbare.iter().copied().filter(|x| !compared.contains(x)).collect();
    assert!(unread.is_empty(), "the car's own code wrote words not compared: {:x?}", runs(unread));
    let bad = show(&bare, &after);
    assert!(bad.is_empty(), "with nothing handed out, {} words differ:\n{}", bad.len(), bad.join("\n"));
    assert!(wbare.is_subset(&wb), "the hooked hand-outs made the car write more");
    // What the hand-outs write is none of the compared words; in the car's blocks, only the
    // scene node's first halfword.
    let handed: Vec<u32> = wb.difference(&wbare).copied().collect();
    let theirs_read: Vec<u32> = handed.iter().copied().filter(|x| compared.contains(x)).collect();
    assert!(theirs_read.is_empty(), "the hand-outs wrote compared words: {:x?}", runs(theirs_read));
    let in_car: Vec<(u32, u32)> = runs(handed.iter().copied().filter(|&x| in_blocks(x)));
    assert_eq!(in_car, [(p.node, p.node + 2)], "the hand-outs wrote in the car's blocks");
    eprintln!(
        "MD106c: {} words equal; the frame wrote {} bytes: the car's own code {}, its hand-outs {} more, the other three listeners {} ({} both); {} instructions",
        after.len(),
        wa.len(),
        wbare.len(),
        handed.len(),
        wo.len(),
        both.len(),
        ee.steps
    );

    // A word changed in the port's result goes red at its place, and only there.
    let (a, x) = after[after.len() / 3];
    let wrong: Vec<(u32, u32)> = after.iter().map(|&(b, y)| if b == a { (b, y ^ 1) } else { (b, y) }).collect();
    assert_eq!(show(&ee, &wrong).len(), after.iter().filter(|&&(b, _)| b == a).count(), "a flipped bit at {a:#x} ({x:#x})");
}
