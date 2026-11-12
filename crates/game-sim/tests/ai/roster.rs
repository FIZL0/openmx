use super::*;

#[test]
fn the_roster_matches_over_ten_thousand_spawns() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&[
        "spawn_racers",
        "operator_new",
        "player_construct",
        "app_pointer",
        "all_ai",
        "rand_seeded",
        "rand_state",
    ]) else {
        return;
    };
    let [spawn, new, construct, app_pointer, all_ai_knob, seeded, rand_state] = at[..] else {
        unreachable!()
    };

    let next = Rc::new(RefCell::new(PLAYERS));
    let n = next.clone();
    ee.hooks.insert(
        new,
        Box::new(move |ee| {
            let at = *n.borrow();
            *n.borrow_mut() += PLAYER_SIZE;
            for k in (0..PLAYER_SIZE).step_by(4) {
                ee.set_u32(at + k, 0);
            }
            ee.set(2, u64::from(at));
        }),
    );
    // The constructor's own fields, as it leaves them, and the vtable the two calls go through.
    ee.hooks.insert(
        construct,
        Box::new(|ee| {
            let p = ee.r(4) as u32;
            ee.set_u32(p + 8, VTABLE);
            ee.set_u32(p + 0x6b0, u32::MAX);
            ee.set_u32(p + 0x6b4, 0);
            ee.set_u32(p + 0x6b8, 0);
            ee.set_u32(p + 0x6bc, 1);
            ee.set(2, u64::from(p));
        }),
    );
    let kinds = Rc::new(RefCell::new(Vec::<(u32, u32)>::new()));
    let added = Rc::new(RefCell::new(Vec::<u32>::new()));
    let k = kinds.clone();
    ee.hooks.insert(
        HOOK_SET_UP,
        Box::new(move |ee| k.borrow_mut().push((ee.r(4) as u32, ee.r(5) as u32))),
    );
    let a = added.clone();
    ee.hooks.insert(HOOK_ADD, Box::new(move |ee| a.borrow_mut().push(ee.r(5) as u32)));

    ee.set_u32(VTABLE + 0xb8, HOOK_SET_UP);
    ee.set_u32(VTABLE + 0x74, HOOK_ADD);
    ee.set_u32(MANAGER + 8, VTABLE);
    ee.set_u32(CALLER + 0x30, LINK);
    ee.set_u32(LINK + 0x30, MANAGER);
    ee.set_u32(app_pointer, APP);
    ee.set_u32(APP + 0x3d4, SETTINGS);
    ee.set_u32(APP + 0x3d8, DATABASE);
    ee.set_u32(SETTINGS + 0xc, EVENT);
    ee.set_u32(SETTINGS + 0x10, HUMAN_IDS);
    ee.set_u32(DATABASE + 0x14, ID_HOLDER);
    ee.set_u32(ID_HOLDER, ID_LIST);
    ee.set_u32(ID_LIST + 8, ID_ENTRIES);
    for i in 0..64 {
        ee.set_u32(ID_ENTRIES + i * 4, entry(i as i32));
        ee.set_u32(HUMAN_IDS + i * 4, 0x4800_0000 + i);
    }
    // Seeded already, so the spawn never asks the clock.
    ee.set_u32(seeded, 1);

    let mut pick = Pick(0x51a3_9e37_c0de_0012);
    let (mut ghosts, mut all, mut full) = (0, 0, 0);
    for case in 0..10_000 {
        let racers = pick.below(13);
        let humans = pick.below(5);
        let all_ai = pick.below(8) == 0 && racers >= 1;
        // The knob makes it one human whatever was asked.
        let ais = (racers - if all_ai { 1 } else { humans }).max(0);
        // Just enough identities to tell the riders apart, up to a database of sixty.
        let identities = FIRST_AI_IDENTITY + ais.max(1) + pick.below(60 - FIRST_AI_IDENTITY as u32 - 12);
        let input = RosterIn {
            racers,
            humans,
            difficulty: if case % 3 == 0 { [0, 69, 70, 79, 80, 89, 90, 99, 100, 255][case % 10] } else { pick.below(256) as u8 },
            identities,
            all_ai,
            ghost_race: pick.below(16) == 0,
        };
        let state = pick.next();

        ee.write(rand_state, 8, u128::from(state));
        ee.set_u32(all_ai_knob, u32::from(input.all_ai));
        ee.set_u32(SETTINGS, input.racers as u32);
        ee.set_u32(EVENT + 4, if input.ghost_race { 3 } else { 1 });
        ee.write(EVENT + 0x116, 1, 0);
        ee.write(EVENT + 0x117, 1, u128::from(input.difficulty));
        ee.set_u32(ID_LIST, input.identities as u32);
        ee.set_u32(CALLER + 0xe8, 0);
        ee.set_u32(CALLER + 0xf0, 0);
        *next.borrow_mut() = PLAYERS;
        kinds.borrow_mut().clear();
        added.borrow_mut().clear();

        ee.call(spawn, &[u64::from(CALLER), input.humans as u64], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        let roster = spawn_roster(&mut rng, input);
        let what = format!("case {case}: {input:?} from {state:#018x}");

        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        let got: Vec<Seen> = added
            .borrow()
            .iter()
            .map(|&p| {
                let kind = kinds.borrow().iter().find(|&&(o, _)| o == p).map_or(0, |&(_, k)| k);
                (kind, ee.i32_at(p + 0x6b0), ee.u32_at(p + 0x6b8), ee.i32_at(p + 0x6bc))
            })
            .collect();
        let mut want: Vec<Seen> = Vec::new();
        if roster.ghost {
            want.push((3, 1, 0, 1));
            want.push((1, 0, 0, 1));
        } else {
            for h in 0..roster.humans {
                want.push((1, h, 0, 1));
            }
            for (j, r) in roster.ai.iter().enumerate() {
                want.push((2, roster.humans + j as i32, entry(r.identity), r.skill));
            }
        }
        assert_eq!(got, want, "{what}: the players differ");
        match roster.all_ai {
            Some(d) => {
                all += 1;
                assert_eq!(ee.i32_at(CALLER + 0xe8), d.racer, "{what}: the AllAI racer");
                assert_eq!(ee.f32_at(CALLER + 0xf0).to_bits(), d.seconds.to_f32().to_bits(), "{what}: the AllAI time");
            }
            None => assert_eq!(ee.u32_at(CALLER + 0xe8), 0, "{what}: an AllAI draw with the knob off"),
        }
        ghosts += usize::from(roster.ghost);
        full += usize::from(roster.ai.len() >= 8);
    }
    eprintln!("ai roster: {ghosts} ghost races, {all} AllAI, {full} grids of 8+ AI");
    // The branches the cases were meant to reach, reached.
    assert!(ghosts > 300 && all > 600 && full > 100, "ghosts {ghosts}, AllAI {all}, full grids {full}");
}
