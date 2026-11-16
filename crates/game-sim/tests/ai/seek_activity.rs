// --- S331a: which loads build a seeker world, and its slot count -----------------------------------------

use super::*;

/// One save state's answer: its event's activity type, whether its activity manager holds a seeker
/// world, and, when it does, the sets the riders took and the slots the load made.
struct S331aLoad {
    activity: i32,
    world: bool,
    used: usize,
    slots: usize,
}

fn s331a_read(ee: &ps2_oracle::Ee) -> S331aLoad {
    let event = ee.u32_at(ee.u32_at(addr("settings").unwrap()) + 0xc);
    let manager = ee.u32_at(addr("activity_manager").unwrap());
    let root = ee.u32_at(addr("ai_seek_sequence_root").unwrap());
    // The manager keeps the root it made at +0x54; the root keeps its sets used at +0x30 and the
    // slots it was loaded with at +0x34.
    let world = root != 0 && ee.u32_at(manager + 0x54) == root;
    let (used, slots) = if world { (ee.u32_at(root + 0x30) as usize, ee.u32_at(root + 0x34) as usize) } else { (0, 0) };
    S331aLoad { activity: ee.u32_at(event + 4) as i32, world, used, slots }
}

/// **Only a freestyle or fast-lap load builds a seeker world, with a slot per rider** — against
/// every save state of a loaded world there is: two single races on sx01 (four racers) and nat03,
/// two practices (sx01, Nat11) and five freestyles (free09 with four racers three times, free10 and
/// free11 alone). A race or practice holds no seeker root ([`sk::loads_world`] false); a freestyle
/// holds one whose slots are its riders' sets ([`sk::slots`]) — except recording2's session
/// (recording2 and first_recording), whose activity holds one more player, a static one that took no
/// set (S316). What that player is was not traced, so those two are held to riders + 1 here.
#[test]
fn s331a_only_freestyle_loads_build_a_seeker_world() {
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let images = [
        ("states/c31-sx01-ai4.ee.bin", false),
        ("states/s282c-sx01-race.ee.bin", false),
        ("states/s296a-nat03.ee.bin", false),
        ("states/s196-sx01.ee.bin", false),
        ("states/s133-nat11.ee.bin", false),
        ("states/s196-free11.ee.bin", false),
        ("states/s133-free10.ee.bin", false),
        ("capture/c33/free09-ai4.ee.bin", false),
        ("states/recording2.p2m2_SaveState.eeMemory.bin", true),
        ("states/first_recording.p2m2_SaveState.eeMemory.bin", true),
    ];
    let (mut races, mut worlds) = (0, 0);
    for (rel, static_player) in images {
        let path = kit.ctx(rel);
        if !path.exists() {
            return skip(&format!("{} is not there", path.display()));
        }
        let ee = ps2_oracle::Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
        let load = s331a_read(&ee);
        assert_eq!(load.world, sk::loads_world(load.activity), "{rel}: activity {} and a seeker world {}", load.activity, load.world);
        if !load.world {
            races += usize::from(load.activity == sk::ACTIVITY_SINGLE_RACE);
            continue;
        }
        worlds += 1;
        let want = sk::slots(load.used) + usize::from(static_player);
        assert_eq!(load.slots, want, "{rel}: {} riders' sets", load.used);
    }
    assert_eq!((races, worlds), (3, 5), "the images' activities");
}
