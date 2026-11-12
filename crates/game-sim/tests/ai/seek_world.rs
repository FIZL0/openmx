// --- S313: a map's seeker world, read off the disc ------------------------------------------------------

use super::*;

/// A member of the disc's container.
pub(crate) fn s313_member(f: &mut std::fs::File, psr: &game_sim::archive::Psr, name: &str) -> Vec<u8> {
    use std::io::{Read, Seek, SeekFrom};
    let e = psr.find(name).unwrap_or_else(|| panic!("no {name}"));
    f.seek(SeekFrom::Start(e.at)).unwrap();
    let mut b = vec![0u8; e.size as usize];
    f.read_exact(&mut b).unwrap();
    b
}

/// A file of one of the container's archives, by its full name.
pub(crate) fn s313_file(file: &[u8], name: &str) -> Vec<u8> {
    let res = game_sim::archive::Res::parse(file).unwrap();
    let want = name.to_ascii_lowercase();
    let e = res.entries.iter().find(|e| e.name.to_ascii_lowercase() == want).unwrap_or_else(|| panic!("no {name}"));
    res.read(file, e).unwrap()
}

/// A quad as RAM holds it, bit for bit.
fn s313_bits(ee: &ps2_oracle::Ee, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| ee.u32_at(at + k))
}

/// **free09's seeker world, loaded from the disc, is the one recording2's image holds, field for
/// field.** `hit.dat`, the hit database and the hit-set database it names come out of the disc's
/// `Tracks\free09_2.res`, and the points are put on free09's terrain; the riders (kind, player, the
/// model root's position) and the slot count are the image's, and a zone is open to player 0 when
/// it is the first of its progression (what this save's progress has unlocked) and to nobody else.
/// Against the image: every jump's three legs (velocity, aim, point, facing, learnt; the scale the
/// seeker never reads is skipped), every target's jump and per-slot done flags, every group's list
/// (by target), point, in-play word and two open bytes (bit 0), and every set's owner, with no pick
/// list yet, no course and no sequence.
#[test]
fn s313_free09_world_loads_as_the_image_holds_it() {
    use game_sim::phy::Ini;
    use game_sim::track::{Scene, Terrain};
    use game_sim::world::{Ground, Place, Props};
    use std::io::Read;

    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let image = kit.ctx("states").join("recording2.p2m2_SaveState.eeMemory.bin");
    if !image.exists() {
        return skip(&format!("{} is not there: `./kit state ram` of recording2.p2m2_SaveState.p2s", image.display()));
    }
    let mut f = match std::fs::File::open(kit.ctx("iso").join("MX.PSR")) {
        Ok(f) => f,
        Err(e) => return skip(&format!("no disc container: {e}")),
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).unwrap();
    let psr = game_sim::archive::Psr::parse(&front).unwrap();
    let track = s313_member(&mut f, &psr, "Tracks\\free09.res");
    let scene = Scene::parse(&s313_file(&track, "tracks\\free09\\free09.scn")).unwrap();
    let trn = s313_file(&track, "tracks\\free09\\free09.ps2.trn");
    let terrain = Terrain::parse(&trn).unwrap();
    let mut place = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };
    let data = s313_member(&mut f, &psr, "Tracks\\free09_2.res");
    let hit_dat = Ini::parse_bytes(&s313_file(&data, "tracks\\free09\\hit.dat"));
    let (hits, sets) = sk::databases(&hit_dat);
    let (hits, sets) = (hits.unwrap(), sets.unwrap());
    assert_eq!((hits, sets), ("free.hit", "free.set"));
    let hits = Ini::parse_bytes(&s313_file(&data, &format!("tracks\\free09\\{hits}")));
    let hit_sets = Ini::parse_bytes(&s313_file(&data, &format!("tracks\\free09\\{sets}")));

    let ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    let root = ee.u32_at(addr("ai_seek_sequence_root").unwrap());
    let set_items = ee.u32_at(root + 0x38);
    let owners: Vec<sk::Owner> = (0..ee.u32_at(root + 0x30))
        .map(|i| {
            let o = ee.u32_at(ee.u32_at(set_items + 4 * i) + 0x54);
            let node = ee.u32_at(ee.u32_at(ee.u32_at(o + 0x3c)) + 0x18);
            sk::Owner {
                kind: ee.u32_at(o + 0x34) as i32,
                player: ee.u32_at(o + 0x30) as usize,
                position: Vf::from_bits(s313_bits(&ee, node + 0x70)),
                hit_freeride: false,
            }
        })
        .collect();
    let zones = ee.u32_at(root + 0x44);
    let manager = ee.u32_at(ee.u32_at(zones + 0x50) + 0x48);
    let slots = ee.u32_at(manager + 0x48) as usize;
    let open = |player: usize, zone: sk::Zone| player == 0 && zone.place == Some(0);
    let world = sk::load(&hit_dat, &hits, &hit_sets, &owners, slots, &open, &mut place);

    // The sets: one per rider, as passed, and nothing built yet.
    assert_eq!(world.sets.len(), 4);
    for (i, set) in world.sets.iter().enumerate() {
        let s = ee.u32_at(set_items + 4 * i as u32);
        assert_eq!(ee.u32_at(s + 0x114), 0, "set {i}: the image's has a course");
        assert_eq!((set.picks, &set.course), (None, &None), "set {i}");
        let o = ee.u32_at(s + 0x54);
        assert_eq!((set.owner.kind, set.owner.player), (ee.u32_at(o + 0x34) as i32, ee.u32_at(o + 0x30) as usize));
    }
    assert_eq!(world.sequence, None);

    // The targets and their jumps, in the database's order.
    let target_items = ee.u32_at(manager + 0x38);
    let targets: Vec<u32> = (0..ee.u32_at(manager + 0x30)).map(|i| ee.u32_at(target_items + 4 * i)).collect();
    assert_eq!((world.targets.len(), world.jumps.len(), targets.len()), (106, 106, 106));
    let leg = |at: u32, l: &sk::Leg, what: &str| {
        for (name, off, port) in
            [("velocity", 0, l.velocity), ("aim", 0x10, l.aim), ("point", 0x20, l.point), ("facing", 0x30, l.facing)]
        {
            assert_eq!(port.to_bits(), s313_bits(&ee, at + off), "{what}: {name} {port:?}");
        }
        assert_eq!(l.learnt.0.to_bits(), ee.u32_at(at + 0x4c), "{what}: learnt {:?}", l.learnt);
    };
    for (i, (t, &at)) in world.targets.iter().zip(&targets).enumerate() {
        let hit = ee.u32_at(at + 0x3c);
        assert_eq!(ee.u16_at(hit + 0x120) as usize, i + 1, "target {i}'s hit id");
        let records = ee.u32_at(at + 0x40);
        let done: Vec<bool> = (0..slots as u32).map(|p| ee.u16_at(records + 0x30 * p + 0x20) & 2 != 0).collect();
        assert_eq!((t.jump, &t.done), (Some(i), &done), "target {i}");
        let j = &world.jumps[i];
        leg(hit, &j.take_off, &format!("jump {i} take-off"));
        leg(hit + 0x60, &j.landing, &format!("jump {i} landing"));
        leg(hit + 0xc0, &j.setup, &format!("jump {i} setup"));
    }

    // The groups: the start zones.
    let group_items = ee.u32_at(zones + 0x38);
    assert_eq!(world.groups.len() as u32, ee.u32_at(zones + 0x30));
    assert_eq!(world.groups.len(), 12);
    for (i, g) in world.groups.iter().enumerate() {
        let at = ee.u32_at(group_items + 4 * i as u32);
        let list = ee.u32_at(at + 0x48);
        let want = (list != 0).then(|| {
            let items = ee.u32_at(list + 0xc);
            (0..ee.u32_at(list + 4))
                .map(|k| {
                    let t = ee.u32_at(items + 4 * k);
                    targets.iter().position(|&a| a == t)
                })
                .collect::<Vec<_>>()
        });
        assert_eq!(g.targets, want, "group {i}'s list");
        assert_eq!(g.point.to_bits(), s313_bits(&ee, at + 0x50), "group {i}'s point {:?}", g.point);
        assert_eq!(g.active, ee.u32_at(at + 0x78) == 1, "group {i} in play");
        let bytes = ee.u32_at(at + 0x44);
        let open: Vec<bool> = (0..2).map(|p| ee.u8_at(bytes + p) & 1 != 0).collect();
        assert_eq!(g.open, open, "group {i}'s open bytes");
    }
    // A hit zone in play, a run zone's list, the race and stunt zones with none, and the image's
    // repeat inside a list: what the loop above covered.
    assert!(world.groups[0].active && !world.groups[5].active);
    assert_eq!(world.groups[8].targets.as_ref().map(Vec::len), Some(4));
    assert_eq!((&world.groups[10].targets, &world.groups[11].targets), (&None, &None));
}
