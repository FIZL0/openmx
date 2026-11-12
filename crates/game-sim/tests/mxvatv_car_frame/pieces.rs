use super::*;

#[test]
fn controls_are_mx_unleasheds() {
    pieces(0x0126_0001, &[CONTROLS], &["controls"], 22);
}

#[test]
fn all_six_pieces_are_mx_unleasheds() {
    pieces_over(
        6000,
        0x0126_0002,
        &[CONTROLS, TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO],
        &["controls", "travel", "anti-slide", "preload", "launch", "aero"],
        48,
    );
}

/// Each piece unhooked alone, and all five, under the frame's oracle: the original's own code for
/// the piece against [`Pieces`]'s port, every word and call equal; each of the piece's branches seen.
fn pieces(seed: u64, unhooked: &[u32], prefixes: &[&str], want: usize) {
    pieces_over(3000, seed, unhooked, prefixes, want);
}

/// [`pieces`] over `cases` frames. The six- and eight-piece runs need 6,000: "preload: fired, degenerate" is
/// rare (it averaged about 17 per 3,000 frames once MD126's controls joined the stream), too close
/// to the floor of 20 for any seed to stay above it (B36).
fn pieces_over(cases: usize, seed: u64, unhooked: &[u32], prefixes: &[&str], want: usize) {
    let Some((_, seen)) = run(seed, cases, unhooked) else { return };
    let mine: Vec<(&&str, &u32)> = seen.iter().filter(|(w, _)| prefixes.iter().any(|p| w.starts_with(p))).collect();
    for &(what, &k) in &mine {
        assert!(k >= 20, "{what} seen only {k} times");
    }
    assert_eq!(mine.len(), want, "a branch was never seen: {seen:?}");
}

#[test]
fn engine_is_mx_unleasheds() {
    pieces(0x0125_0001, &[ENGINE_STEP, SHIFT], &["engine", "shift"], 24);
}

#[test]
fn engine_with_the_controls_is_mx_unleasheds() {
    pieces(0x0125_0002, &[CONTROLS, ENGINE_STEP, SHIFT], &["controls", "engine", "shift"], 22 + 24);
}

#[test]
fn travel_cofg_is_mx_unleasheds() {
    pieces(0x0106_a001, &[TRAVEL_COFG], &["travel"], 4);
}

#[test]
fn anti_slide_is_mx_unleasheds() {
    pieces(0x0106_a002, &[ANTI_SLIDE], &["anti-slide"], 3);
}

#[test]
fn preload_is_mx_unleasheds() {
    pieces(0x0106_a003, &[PRELOAD], &["preload"], 10);
}

#[test]
fn launch_is_mx_unleasheds() {
    pieces(0x0106_a004, &[LAUNCH], &["launch"], 5);
}

#[test]
fn aero_is_mx_unleasheds() {
    pieces(0x0106_a005, &[AERO], &["aero"], 4);
}

#[test]
fn tire_step_is_mx_unleasheds() {
    pieces(0x0124_a001, &[TIRE_STEP], &["tire"], 20);
}

#[test]
fn contact_sort_is_mx_unleasheds() {
    pieces(0x0127_0001, &[SORT_CONTACTS], &["sort"], 10);
}

#[test]
fn sort_and_probes_are_mx_unleasheds() {
    pieces(0x0127_0002, &[SORT_CONTACTS, PROBE], &["sort", "probe"], 10 + 13);
}

#[test]
fn all_five_pieces_are_mx_unleasheds() {
    pieces(0x0106_a006, &[TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO], &["travel", "anti-slide", "preload", "launch", "aero"], 26);
}

#[test]
fn all_six_pieces_with_the_tire_step_are_mx_unleasheds() {
    let all = [TRAVEL_COFG, ANTI_SLIDE, PRELOAD, LAUNCH, AERO, TIRE_STEP];
    pieces_over(6000, 0x0124_a002, &all, &["travel", "anti-slide", "preload", "launch", "aero", "tire"], 46);
}

#[test]
fn all_eight_pieces_are_mx_unleasheds() {
    // 6,000 frames, as the six-piece run: the degenerate preload fell to 19 in 3,000 (F31b5l).
    pieces_over(
        6000,
        0x0127_0013,
        &[TRAVEL_COFG, SORT_CONTACTS, PROBE, ANTI_SLIDE, PRELOAD, LAUNCH, AERO, TIRE_STEP],
        &["travel", "sort", "probe", "anti-slide", "preload", "launch", "aero", "tire"],
        // Every tag but the sort's struck object and kept mover.
        46 + 10 + 13 - 2,
    );
}

/// **The car's frame is the original's**: SLUS-21104's world step over the buggy image's world, with
/// every call of the car's three listeners hooked, against [`atv_car::frame`] with a [`Steps`] that
/// plays the same script, over 3,000 generated states: every word of the car, its tires and its body
/// equal, the same calls in the same order with the same arguments, and nothing else written in those
/// blocks. Each branch of the listeners is seen; then a word of the port's result is changed, to see
/// the comparison go red there.
#[test]
fn the_cars_frame_is_the_originals() {
    let Some((seen, _)) = run(0x0106_ca7f_0000_0001, 3000, &[]) else { return };
    for (what, &k) in &seen {
        assert!(k >= 30, "{what} seen only {k} times");
    }
    assert_eq!(seen.len(), 19, "a branch was never seen: {seen:?}");
}

pub(crate) type Seen = std::collections::BTreeMap<&'static str, u32>;

/// The world step's frame, `cases` generated states from `seed`, with the calls at `unhooked` left to
/// SLUS-21104's own code and the port's answered by MX Unleashed's [`Pieces`]: the listeners' branches
/// seen, and the pieces'.
fn run(seed: u64, cases: usize, unhooked: &[u32]) -> Option<(Seen, Seen)> {
    let pristine = image()?;
    let mut ee = image()?;
    let p = ptrs(&ee);
    let image_car = read_car(&ee, &p);
    let image_body = seed::body(&|a| ee.u32_at(a), p.body);
    let image_mx = read_mx(&ee, &p);
    assert!(image_body.enabled, "the image's body is enabled");
    let log: Shared<Vec<Call>> = Rc::default();
    let script: Shared<Script> = Rc::default();
    hook(&mut ee, p, log.clone(), script.clone());
    hook_cue(&mut ee, log.clone());
    for a in unhooked {
        assert!(ee.hooks.remove(a).is_some(), "{a:#x} was not hooked");
    }
    // The engine's shift and reverse switch go with its step (MD125).
    assert_eq!(unhooked.contains(&ENGINE_STEP), unhooked.contains(&SHIFT), "the engine's step and shift unhooked together");
    if unhooked.contains(&ENGINE_STEP) {
        assert!(ee.hooks.remove(&REVERSE).is_some(), "the reverse switch was not hooked");
    }
    let world = World { iterations: 1, rate: F(115.0) };
    let table = AcosTable::build();
    let mut r = Rng(seed);
    let mut r2 = Rng(seed ^ 0x0127_5eed_0127_5eed);
    let mut r3 = Rng(seed ^ 0x0125_e061_0125_e061);
    let engine = unhooked.contains(&ENGINE_STEP);
    let mut seen = Seen::new();
    let mut pieces_seen = Seen::new();
    let mut see = |what: &'static str, yes: bool| {
        if yes {
            *seen.entry(what).or_insert(0) += 1;
        }
    };
    let blocks: Vec<(u32, u32)> = [(CAR, 0x1500), (p.body, 0x230), (p.record + at::TIPPED, 4), (p.engine, engine::SIZE)]
        .into_iter()
        .chain(p.tires.map(|t| (t, tire::SIZE)))
        .collect();
    for case in 0..cases {
        let (mut car, mut body, dt) = generate(&mut r, &image_car, &image_body, &p);
        let mut m = generate_mx(&mut r, &image_mx, &mut car, &mut body);
        generate_tires(&mut r, &mut m, &mut car, &mut body);
        // The rears' friction scales (MD126) are their tires' (MD124): one word each.
        m.state.rear_friction = [2, 3].map(|t| m.keys.tires[t].friction_scale);
        let mut s = Script::new(&mut r);
        if unhooked.contains(&SORT_CONTACTS) || unhooked.contains(&PROBE) {
            generate_probe(&mut r2, &mut car, &body);
            generate_sort(&mut r2, &mut s, &body);
            // With the tire step unhooked too, what the port's step leaves to the rest or does
            // not prove (MD124) stays out: a tire standing on a struck object or a kept mover, and
            // a surface byte past the sixteen tables.
            if unhooked.contains(&TIRE_STEP) {
                for h in &mut s.sort.hits {
                    h.object = 0;
                    if h.mover_kind == 2 {
                        h.mover_kind = 3;
                    }
                }
                for g in s.ground.iter_mut().flatten() {
                    g.surface &= 15;
                }
                for t in &mut car.tires {
                    t.surface &= 15;
                }
            }
        }
        if engine {
            generate_engine(&mut r3, &mut m, &mut car);
        }
        let before = [car_words(&car, &p), seed::body_words(&body, p.body), mx_words(&m, &p)].concat();

        // The original.
        poke(&mut ee, &before);
        *script.borrow_mut() = s.clone();
        log.borrow_mut().clear();
        ee.set(28, GP);
        ee.call(WORLD_STEP, &[u64::from(WORLD)], &[dt.0], 20_000_000);
        let written = ee.written_ranges();

        // The port.
        let (mut pc, mut pb) = (car, body);
        let port = Port { log: Vec::new(), script: s };
        let keys = Keys { engine: if engine { m.keys.engine.clone() } else { None }, ..m.keys() };
        let mut port = Unhooked { pieces: Pieces { keys, state: m.state, rest: port }, unhooked, seen: Seen::new() };
        atv_car::frame(&world, &mut pc, &mut pb, dt, &table, &mut port);
        for (what, k) in std::mem::take(&mut port.seen) {
            *pieces_seen.entry(what).or_insert(0) += k;
        }
        let pm = Mx { keys: Keys { tires: port.pieces.keys.tires, ..m.keys.clone() }, state: port.pieces.state, ..m };
        let port = port.pieces.rest;
        let after = [car_words(&pc, &p), seed::body_words(&pb, p.body), mx_words(&pm, &p)].concat();

        let bad: Vec<String> = after
            .iter()
            .filter(|&&(a, x)| ee.u32_at(a) != x)
            .map(|&(a, x)| format!("  {a:#x}: port {x:#010x} ({}), original {:#010x} ({})", f32::from_bits(x), ee.u32_at(a), ee.f32_at(a)))
            .collect();
        assert!(bad.is_empty(), "case {case} (dt {dt:?}): {} words differ\n{}", bad.len(), bad.join("\n"));
        let theirs = log.borrow().iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        let ours = port.log.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>();
        assert_eq!(ours, theirs, "case {case}: the calls");
        let compared: std::collections::BTreeSet<u32> = after.iter().map(|&(a, _)| a).collect();
        for &(from, to) in &written {
            for a in (from & !3..to).step_by(4) {
                let ours = blocks.iter().any(|&(b, n)| (b..b + n).contains(&a));
                assert!(!ours || compared.contains(&a), "case {case}: the original wrote {a:#x}, which is not compared");
            }
        }

        let n = ours.len();
        see("riding", car.state == atv_car::RIDING);
        see("wrecked", car.state != atv_car::RIDING);
        see("lone front", pc.tires[0].lone_front);
        see("no front alone", !pc.tires[0].lone_front && (s_any(&pc, |t| t.solid || t.sorted)));
        see("handed by the left", pc.tires[2].handed && pc.tires[0].grounded);
        see("handed by the right", pc.tires[2].handed && !pc.tires[0].grounded);
        see("nothing handed", !pc.tires[2].handed);
        let landed = ours.iter().any(|c| c.starts_with("Landed"));
        let shock = pb.correction_peak.to_bits() != [0, 0, 0, 1.0f32.to_bits()];
        see("landed on the shock", landed && shock);
        see("landed on the velocity", landed && !shock);
        see("one side", pc.one_side && pc.grounded);
        see("airborne", !pc.grounded);
        see("yaw held", pb.rates.y().abs() == pc.yaw_rate_limit.0);
        see("tipped", pc.tipped);
        see("on the tipped edge", pc.up.to_bits()[..3] == [0, 1.0f32.to_bits(), 0] && port.script.normal.y().to_bits() == EDGE);
        see("tied on signed zeros", pc.tires[2].handed && pc.tires[0].load == 0.0 && pc.tires[1].load == 0.0 && pc.tires[0].load.to_bits() != pc.tires[1].load.to_bits());
        see("upright", !pc.tipped);
        see("latch let go", car.latch && car.latched);
        see("ground velocity of a tire", pc.grounded);
        see("more substeps", n > 40);

        if case == 0 {
            // One flipped bit of the port's result goes red in exactly that word.
            let (a, _) = after[after.len() / 2];
            let red: Vec<u32> = after
                .iter()
                .map(|&(b, y)| if b == a { (b, y ^ 1) } else { (b, y) })
                .filter(|&(b, y)| ee.u32_at(b) != y)
                .map(|(b, _)| b)
                .collect();
            assert_eq!(red, [a], "a flipped bit");
        }

        // Put the image back.
        for (from, to) in written {
            for a in from..to {
                ee.write(a, 1, u128::from(pristine.u8_at(a)));
            }
        }
    }
    eprintln!("MD106 (unhooked {unhooked:x?}): {cases} frames, seen {seen:?}, pieces {pieces_seen:?}");
    Some((seen, pieces_seen))
}

fn s_any(car: &Car, f: impl Fn(&Tire) -> bool) -> bool {
    car.tires.iter().any(f)
}
