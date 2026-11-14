// --- S315: a start zone's open flags from the save profile ----------------------------------------------

use super::*;

/// What the opening reads in one of the free-ride images: the rider manager, its zone list, its zone
/// records, the save profile's record of the event's track, and the race activity.
struct S315Image {
    ee: ps2_oracle::Ee,
    manager: u32,
    zones: u32,
    records: u32,
    count: u32,
    profile: u32,
    race: u32,
}

fn s315_image(name: &str) -> Option<S315Image> {
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(w) => {
            skip(&w);
            return None;
        }
    };
    let image = kit.ctx("states").join(format!("{name}.p2m2_SaveState.eeMemory.bin"));
    if !image.exists() {
        skip(&format!("{} is not there: `./kit state ram` of {name}.p2m2_SaveState.p2s", image.display()));
        return None;
    }
    let at = named(&["ai_seek_sequence_root", "app_pointer", "activity_manager"])?;
    let ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    let manager = ee.u32_at(at[0]);
    // The profile's map records, found by the event's track name as the original's lookup does.
    let app = ee.u32_at(at[1]);
    let list = ee.u32_at(ee.u32_at(app + 0x3f4) + 0xb0);
    let track = s315_text(&ee, ee.u32_at(ee.u32_at(app + 0x3d4) + 0xc) + 8);
    assert!(track.starts_with("Free"), "{name}: the event's track {track}");
    let profile = (0..ee.u32_at(list + 0x58))
        .map(|k| ee.u32_at(list + 0x40 + 4 * k))
        .find(|&r| s315_text(&ee, ee.u32_at(ee.u32_at(r + 4))).eq_ignore_ascii_case(&track))
        .unwrap_or_else(|| panic!("{name}: no profile record for {track}"));
    let race = ee.u32_at(ee.u32_at(ee.u32_at(at[2]) + 0x58) + 0x20);
    Some(S315Image {
        zones: ee.u32_at(manager + 0x44),
        records: ee.u32_at(manager + 0x40),
        count: ee.u32_at(manager + 0x3c),
        manager,
        profile,
        race,
        ee,
    })
}

fn s315_text(ee: &ps2_oracle::Ee, at: u32) -> String {
    (0..).map(|k| ee.u8_at(at + k)).take_while(|&b| b != 0).map(char::from).collect()
}

/// A zone record as the port's [`sk::Zone`]: its type and place.
fn s315_zone(ee: &ps2_oracle::Ee, record: u32) -> sk::Zone {
    let activity = match ee.u32_at(record + 8) {
        1 => Some(sk::Activity::Hit),
        2 => Some(sk::Activity::Run),
        3 => Some(sk::Activity::Race),
        4 => Some(sk::Activity::Stunt),
        _ => None,
    };
    let place = ee.u32_at(record) as i32;
    sk::Zone { activity, number: ee.u32_at(record + 4) as i32, place: usize::try_from(place).ok() }
}

/// The profile record's bytes the opening reads (through +0x30).
fn s315_progress(ee: &ps2_oracle::Ee, profile: u32) -> sk::Progress {
    sk::Progress((0..0x30).map(|k| ee.u8_at(profile + k)).collect())
}

/// **A start zone's flag bytes at the map's load are the original's opening, over 10,000 profiles.**
/// The original's per-player opening runs in the interpreter on recording2's image, from the rider
/// manager's held test to the end of its zone loop, for one human owner: each case writes a random
/// map record into the save profile's free09 record (the race and stunt words, and every hit and run
/// byte a place from -1 to 4 reaches), random zone records (place -1..4, type 0..5), the held word,
/// the race activity's count, which player the owner is, and the hit-progression answer (the check
/// itself hooked), and now and then takes the profile away. Every zone's two bytes, from 0, against
/// [`sk::open_flags`] for the owner's player and 0 for the other.
#[test]
fn s315_zone_open_flags_are_the_originals() {
    let Some(at) = named(&["ai_zone_open_stretch", "ai_zone_open_stretch_end", "ai_zone_open_stretch_skip", "ai_zone_hit_running"])
    else {
        return;
    };
    let [start, end, next, hit_check] = at[..] else { unreachable!() };
    let Some(S315Image { mut ee, manager, zones, records, count, profile, race }) = s315_image("recording2") else { return };
    assert_eq!(count, 12);
    let owner = {
        let items = ee.u32_at(manager + 0x38);
        (0..ee.u32_at(manager + 0x30))
            .map(|i| ee.u32_at(ee.u32_at(items + 4 * i) + 0x54))
            .find(|&o| ee.u32_at(o + 0x34) == 1)
            .expect("a human owner")
    };
    let answer = Rc::new(RefCell::new((0u64, 0u32)));
    for a in [end, next] {
        ee.hooks.insert(a, Box::new(|ee| ee.set(31, u64::from(ps2_oracle::DONE) | 0xffff_ffff_0000_0000)));
    }
    let a = Rc::clone(&answer);
    ee.hooks.insert(hit_check, Box::new(move |ee| {
        let mut a = a.borrow_mut();
        a.1 += 1;
        ee.set(2, a.0);
    }));
    let items = ee.u32_at(zones + 0x38);
    let bytes: Vec<u32> = (0..count).map(|i| ee.u32_at(ee.u32_at(items + 4 * i) + 0x44)).collect();

    let mut pick = Pick(0x5315_0f1a_95ee_d001);
    let (mut opened, mut shown, mut held, mut bare, mut checked) = (0, 0, 0, 0, 0);
    for case in 0..10_000 {
        for k in 0x14..0x30 {
            // Mostly the low two bits, which are what the bytes say; now and then any byte.
            let v = if pick.below(4) == 0 { pick.below(256) } else { pick.below(4) };
            ee.write(profile + k, 1, u128::from(v as u8));
        }
        for z in 0..count {
            let r = records + 12 * z;
            ee.set_u32(r, (pick.below(6) - 1) as u32);
            ee.set_u32(r + 8, pick.below(6) as u32);
            ee.write(bytes[z as usize], 1, 0);
            ee.write(bytes[z as usize] + 1, 1, 0);
        }
        let opening = sk::Opening { held: pick.below(8) == 0, hit_running: pick.below(4) == 0, race_live: pick.below(2) == 0 };
        ee.set_u32(manager + 0x58, if opening.held { 1 + pick.below(1000) as u32 } else { 0 });
        ee.set_u32(race + 0x14, if opening.race_live { 1 + pick.below(3) as u32 } else { 0 });
        answer.borrow_mut().0 = u64::from(opening.hit_running);
        let player = pick.below(2) as usize;
        ee.set_u32(owner + 0x30, player as u32);
        let with = pick.below(16) != 0;

        ee.set(19, u64::from(if with { profile } else { 0 }));
        ee.set(21, u64::from(owner));
        ee.set(23, u64::from(zones));
        ee.call(start, &[], &[], BUDGET);

        let progress = s315_progress(&ee, profile);
        for z in 0..count {
            let zone = s315_zone(&ee, records + 12 * z);
            let port = sk::open_flags(zone, player, false, with.then_some(&progress), opening);
            for p in 0..sk::OPEN_PLAYERS {
                let want = if p == player { port } else { 0 };
                let got = ee.u8_at(bytes[z as usize] + p as u32);
                assert_eq!(got, want, "case {case}: zone {z} {zone:?}, player {p} (owner {player}), {opening:?}, profile {with}: {:02x?}", progress.0);
                if p == player && port & sk::ZONE_OPEN != 0 {
                    opened += 1;
                    shown += usize::from(port & sk::ZONE_SHOWN != 0);
                }
                checked += 1;
            }
        }
        held += usize::from(opening.held);
        bare += usize::from(!with);
    }
    // Every branch reached: opened and shown zones, the held and no-profile exits, and the check.
    assert!(opened > 10_000 && shown > 3_000 && held > 1_000 && bare > 300, "{opened} {shown} {held} {bare}");
    assert!(answer.borrow().1 > 5_000, "the hit check ran {} times", answer.borrow().1);
    assert_eq!(checked, 10_000 * 12 * 2);
}

/// **Three free-ride images' zone bytes are the port's opening of their own save.** recording2 and
/// first_recording (free09) and biplane (free06): each zone record and the profile's record of the
/// event's track off the image, the
/// held word and the race activity's count as the image has them, no rider in a hit progression, and
/// the session's human owners (kind 1) as the players; every zone's two bytes against
/// [`sk::open_flags`], and their open bits against [`sk::opener`], the answer the seeker world's load
/// takes. (biplane's race zone is closed: the player is in the plane and the race activity's list is
/// empty.)
#[test]
fn s315_free_ride_images_hold_the_ports_zone_bytes() {
    for name in ["recording2", "first_recording", "biplane"] {
        let Some(S315Image { ee, manager, zones, records, count, profile, race }) = s315_image(name) else { return };
        let progress = s315_progress(&ee, profile);
        let opening = sk::Opening {
            held: ee.u32_at(manager + 0x58) != 0,
            hit_running: false,
            race_live: ee.u32_at(race + 0x14) != 0,
        };
        let items = ee.u32_at(manager + 0x38);
        let owners: Vec<u32> = (0..ee.u32_at(manager + 0x30)).map(|i| ee.u32_at(ee.u32_at(items + 4 * i) + 0x54)).collect();
        let humans: Vec<usize> =
            owners.iter().filter(|&&o| ee.u32_at(o + 0x34) != 2).map(|&o| ee.u32_at(o + 0x30) as usize).collect();
        assert_eq!(humans, [0], "{name}: one human, player 0");
        let open = sk::opener(&humans, Some(&progress), opening);
        let group_items = ee.u32_at(zones + 0x38);
        let mut seen = Vec::new();
        for z in 0..count {
            let zone = s315_zone(&ee, records + 12 * z);
            let at = ee.u32_at(group_items + 4 * z);
            assert_eq!((zone.number, zone.place.map_or(-1, |p| p as i32)), (ee.u32_at(at + 0x74) as i32, ee.u32_at(at + 0x70) as i32));
            let bytes = ee.u32_at(at + 0x44);
            for p in 0..sk::OPEN_PLAYERS {
                let want = if humans.contains(&p) { sk::open_flags(zone, p, false, Some(&progress), opening) } else { 0 };
                let got = ee.u8_at(bytes + p as u32);
                assert_eq!(got, want, "{name}: zone {z} {zone:?}, player {p}");
                assert_eq!(open(p, zone), got & 1 != 0, "{name}: zone {z}, player {p}: the load's answer");
            }
            seen.push(ee.u8_at(bytes));
        }
        let race_byte = if name == "biplane" { 0 } else { 5 };
        assert_eq!(seen, [5, 0, 0, 0, 0, 5, 0, 0, 0, 0, race_byte, 5], "{name}: the first of each opened");
    }
}
