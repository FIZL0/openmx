//! The career rounds' rosters (S12f), proved against the original's own spawn with the event's career
//! byte set: the same harness as `roster`, plus the chain the spawn walks to the career's current round
//! (the first player's profile, its career, the round) and both of the round's lists.

use super::*;
use game_sim::ai::{AiRider, CareerRoster, MAX_CHAMPIONSHIP_AI, MAX_SAVED_AI, spawn_career_roster};

/// The career chain, after the players' block so the allocator never reaches it.
const PROFILE: u32 = 0x0182_1000; // +0xb0 its career
const CAREER: u32 = 0x0182_2000; // +0x5c the current round
const ROUND: u32 = 0x0182_3000; // +0 saved (0) or not, +4 the championship, +0x4c riders, +0x68 count
const CHAMPIONSHIP: u32 = 0x0182_4000; // +0x2c identities, +0x38 count
const SAVED: u32 = 0x0182_5000; // the saved riders, 0x100 apart: +0x74 identity entry, +0x78 skill

#[test]
fn career_rosters_match_over_ten_thousand_spawns() {
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
    ee.hooks.insert(HOOK_SET_UP, Box::new(move |ee| k.borrow_mut().push((ee.r(4) as u32, ee.r(5) as u32))));
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
    // The app's per-player profile pointers start at +0x3f4: the first player's is that word.
    ee.set_u32(APP + 0x3f4, PROFILE);
    ee.set_u32(PROFILE + 0xb0, CAREER);
    ee.set_u32(CAREER + 0x5c, ROUND);
    ee.set_u32(ROUND + 4, CHAMPIONSHIP);
    ee.set_u32(SETTINGS + 0xc, EVENT);
    ee.set_u32(SETTINGS + 0x10, HUMAN_IDS);
    ee.set_u32(DATABASE + 0x14, ID_HOLDER);
    ee.set_u32(ID_HOLDER, ID_LIST);
    ee.set_u32(ID_LIST + 8, ID_ENTRIES);
    for i in 0..64 {
        ee.set_u32(ID_ENTRIES + i * 4, entry(i as i32));
        ee.set_u32(HUMAN_IDS + i * 4, 0x4800_0000 + i);
    }
    ee.set_u32(seeded, 1);

    let mut pick = Pick(0x7c3a_11d5_e0f1_9a27);
    let (mut saved, mut champ, mut ghosts, mut all, mut drawn) = (0, 0, 0, 0, 0);
    for case in 0..10_000 {
        let racers = pick.below(13);
        let all_ai = pick.below(8) == 0 && racers >= 1;
        let input = RosterIn {
            racers,
            humans: pick.below(5),
            difficulty: if case % 3 == 0 { [0, 69, 70, 79, 80, 89, 90, 99, 100, 255][case % 10] } else { pick.below(256) as u8 },
            // The career lists bypass the database: the spawn must not read it.
            identities: 0,
            all_ai,
            ghost_race: pick.below(16) == 0,
        };
        let career = if pick.below(2) == 0 {
            let count = pick.below(MAX_SAVED_AI as u32 + 2) as u8;
            let riders = (0..MAX_SAVED_AI)
                .map(|_| AiRider { identity: pick.below(64) as i32, skill: pick.next() as i32 })
                .collect();
            CareerRoster::Saved { count, riders }
        } else {
            CareerRoster::Championship {
                count: pick.below(MAX_CHAMPIONSHIP_AI as u32 + 3) as i32 - 2,
                identities: [0; 3].map(|_| pick.below(64) as i32),
            }
        };
        let state = pick.next();

        ee.write(rand_state, 8, u128::from(state));
        ee.set_u32(all_ai_knob, u32::from(input.all_ai));
        ee.set_u32(SETTINGS, input.racers as u32);
        ee.set_u32(EVENT + 4, if input.ghost_race { 3 } else { 1 });
        ee.write(EVENT + 0x116, 1, 1);
        ee.write(EVENT + 0x117, 1, u128::from(input.difficulty));
        // A database the spawn would draw nothing sane from, were it read.
        ee.set_u32(ID_LIST, 0);
        match &career {
            CareerRoster::Saved { count, riders } => {
                ee.set_u32(ROUND, 0);
                // The count byte shares its word with bytes the spawn must not read.
                ee.set_u32(ROUND + 0x68, 0xa5c3_e100 | u32::from(*count));
                for (i, r) in riders.iter().enumerate() {
                    let at = SAVED + i as u32 * 0x100;
                    ee.set_u32(ROUND + 0x4c + i as u32 * 4, at);
                    ee.set_u32(at + 0x74, entry(r.identity));
                    ee.set_u32(at + 0x78, r.skill as u32);
                }
            }
            CareerRoster::Championship { count, identities } => {
                ee.set_u32(ROUND, 1 + pick.below(0x7fff_ffff) as u32);
                ee.set_u32(CHAMPIONSHIP + 0x38, *count as u32);
                for (i, &id) in identities.iter().enumerate() {
                    ee.set_u32(CHAMPIONSHIP + 0x2c + i as u32 * 4, entry(id));
                }
            }
        }
        ee.set_u32(CALLER + 0xe8, 0);
        ee.set_u32(CALLER + 0xf0, 0);
        *next.borrow_mut() = PLAYERS;
        kinds.borrow_mut().clear();
        added.borrow_mut().clear();

        ee.call(spawn, &[u64::from(CALLER), input.humans as u64], &[], BUDGET);

        let mut rng = Rng::from_state(state);
        let out = spawn_career_roster(&mut rng, input, &career);
        let roster = &out.roster;
        let what = format!("case {case}: {input:?} {career:?} from {state:#018x}");

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
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "{what}: the generator ends elsewhere");
        assert_eq!(ee.i32_at(SETTINGS), out.racers, "{what}: the racer count written back");
        match roster.all_ai {
            Some(d) => {
                all += 1;
                assert_eq!(ee.i32_at(CALLER + 0xe8), d.racer, "{what}: the AllAI racer");
                assert_eq!(ee.f32_at(CALLER + 0xf0).to_bits(), d.seconds.to_f32().to_bits(), "{what}: the AllAI time");
            }
            None => assert_eq!(ee.u32_at(CALLER + 0xe8), 0, "{what}: an AllAI draw with the knob off"),
        }
        if roster.ghost {
            ghosts += 1;
        } else if !roster.ai.is_empty() {
            match career {
                CareerRoster::Saved { .. } => saved += 1,
                CareerRoster::Championship { .. } => {
                    champ += 1;
                    // A skill range wider than one level, so the draw shows.
                    drawn += usize::from(roster.ai.iter().any(|r| r.skill != roster.ai[0].skill));
                }
            }
        }
    }
    eprintln!("career roster: {saved} saved, {champ} championship ({drawn} with mixed skills), {ghosts} ghost races, {all} AllAI");
    assert!(saved > 3000 && champ > 1500 && drawn > 300 && ghosts > 300 && all > 600, "saved {saved}, championship {champ}, mixed {drawn}, ghosts {ghosts}, AllAI {all}");
}
