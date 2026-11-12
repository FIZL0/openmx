// ---------------------------------------------------------------------------------------------------
// S282c: a track's two path nets off the disc, against a race image's, node for node.
// ---------------------------------------------------------------------------------------------------

use super::*;

/// sx01's race files out of `Tracks\sx01_2.res`, inside MX.PSR, by their names in the archive.
fn sx01_files(names: &[&str]) -> Option<Vec<Vec<u8>>> {
    let kit = ps2_oracle::Kit::load().map_err(|why| skip(&why)).ok()?;
    let at = kit.ctx("iso").join("MX.PSR");
    let Ok(mut f) = std::fs::File::open(&at) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
        return None;
    };
    use std::io::{Read, Seek, SeekFrom};
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = game_sim::archive::Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("Tracks\\sx01_2.res").expect("sx01's second track archive");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = game_sim::archive::Res::parse(&bytes).expect("the archive's layout");
    let out = names
        .iter()
        .map(|n| {
            let e = res.entries.iter().find(|e| e.name.eq_ignore_ascii_case(n)).unwrap_or_else(|| panic!("{n} in sx01_2.res"));
            res.read(&bytes, e).expect("inflate")
        })
        .collect();
    Some(out)
}

/// The sx01 race image: research/c31's race start run on to frame 604, three AI riders on the
/// path follower.
pub(crate) const S282C_STATE: &str = "s282c-sx01-race.ee.bin";
/// The three AI riders in that image.
pub(crate) const S282C_RIDERS: [u32; 3] = [0x008c_9af0, 0x0090_3050, 0x0111_de50];

/// A net in RAM against the port's: every path's flags, length, first node and branches, every
/// node's centre and edges (`x`, `z`, `w`: the heights are the ground's), its length and its next.
/// Hands back the RAM addresses of the paths and nodes, in the port's order.
fn same_net(ee: &ps2_oracle::Ee, track: u32, net: &Net, what: &str) -> (Vec<u32>, Vec<u32>) {
    let count = ee.u32_at(track + 0x40) as usize;
    assert_eq!(count, net.paths.len(), "{what}: the path count");
    let paths: Vec<u32> = (0..count as u32).map(|k| ee.u32_at(ee.u32_at(track + 0x50) + 4 * k)).collect();
    let mut nodes = Vec::new();
    for (k, &p) in paths.iter().enumerate() {
        assert_eq!(net.paths[k].first, nodes.len(), "{what}: path {k}'s first node");
        let mut n = ee.u32_at(p + 0x10);
        while n != 0 {
            nodes.push(n);
            n = ee.u32_at(n + 0x54);
        }
    }
    assert_eq!(nodes.len(), net.nodes.len(), "{what}: the node count");
    let path_index = |a: u32| paths.iter().position(|&p| p == a);
    let node_index = |a: u32| nodes.iter().position(|&n| n == a);
    for (k, (&p, ours)) in paths.iter().zip(&net.paths).enumerate() {
        let list = ee.u32_at(p + 0x2c);
        let branches: Vec<Option<usize>> =
            (0..ee.u32_at(p + 0x18)).map(|b| path_index(ee.u32_at(list + 4 * b))).collect();
        assert_eq!(
            (ee.u32_at(p), ee.u32_at(p + 8), branches),
            (ours.flags, ours.length.0.to_bits(), ours.branches.iter().map(|&b| Some(b)).collect()),
            "{what}: path {k}'s flags, length and branches"
        );
    }
    for (i, (&a, n)) in nodes.iter().zip(&net.nodes).enumerate() {
        let flat = |v: [u32; 4]| [v[0], v[2], v[3]];
        let mine = |v: Vf| {
            let b = v.to_bits();
            [b[0], b[2], b[3]]
        };
        assert_eq!(
            (flat(got(ee, a + 0x10)), flat(got(ee, a + 0x20)), flat(got(ee, a + 0x30)), ee.u32_at(a + 0x40), node_index(ee.u32_at(a + 0x54))),
            (mine(n.centre), mine(n.edges[0]), mine(n.edges[1]), n.length.0.to_bits(), n.next),
            "{what}: node {i}"
        );
    }
    (paths, nodes)
}

#[test]
fn sx01s_path_nets_off_the_disc_are_the_race_images() {
    use game_sim::race::{TrackFile, TrackPaths};
    let Some(files) = sx01_files(&["tracks\\sx01\\sx01.tdf", "tracks\\sx01\\sx01ai.tdf"]) else { return };
    let kit = match ps2_oracle::Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let image = kit.ctx("states").join(S282C_STATE);
    if !image.exists() {
        return skip(&format!(
            "{} is not there: run research/c31's start state on to frame ~600, save, `./kit state ram`",
            image.display()
        ));
    }
    let Some(at) = named(&["activity_manager", "boot_gp", "path_locate"]) else { return };
    let [manager, gp, locate] = at[..] else { unreachable!() };
    let mut ee = ps2_oracle::Ee::from_ram(&image).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    let act = ee.u32_at(ee.u32_at(ee.u32_at(manager) + 0x58));
    let (race_track, ai_track) = (ee.u32_at(act + 0xb8), ee.u32_at(act + 0xbc));
    let net = |bytes: &[u8]| {
        let file = TrackFile::parse(bytes).expect("the race file reads");
        Net::from_track(&TrackPaths::new(&file).expect("its paths walk"))
    };
    let (race, ai) = (net(&files[0]), net(&files[1]));
    assert_eq!((race.paths.len(), race.nodes.len()), (2, 96), "sx01.tdf: a start run and the lap");
    assert_eq!((ai.paths.len(), ai.nodes.len()), (38, 444), "sx01AI.tdf: the AI's web");
    let (race_paths, race_nodes) = same_net(&ee, race_track, &race, "sx01.tdf");
    let (ai_paths, ai_nodes) = same_net(&ee, ai_track, &ai, "sx01AI.tdf");

    // Which net is whose: the follower's (+0xdc) is the AI file's; the avoidance's edge asks the
    // race's (+0xd8's +0x68) at the card's path and node (+0xd4's +0x88, +0x8c).
    let mut on_ai = 0;
    for r in S282C_RIDERS {
        assert_eq!(ee.u32_at(r + 0xdc), ai_track, "rider {r:#x}'s follower net");
        assert_eq!(ee.u32_at(ee.u32_at(r + 0xd8) + 0x68), race_track, "rider {r:#x}'s race net");
        let card = ee.u32_at(r + 0xd4);
        assert!(race_paths.contains(&ee.u32_at(card + 0x88)), "rider {r:#x}'s card path is the race net's");
        assert!(race_nodes.contains(&ee.u32_at(card + 0x8c)), "rider {r:#x}'s card node is the race net's");
        // The follower's spot (+0x220: path, node, along), once it has one, is on the AI net.
        if ee.u32_at(r + 0x220) != 0 {
            assert!(ai_paths.contains(&ee.u32_at(r + 0x220)), "rider {r:#x}'s spot path");
            assert!(ai_nodes.contains(&ee.u32_at(r + 0x224)), "rider {r:#x}'s spot node");
            on_ai += 1;
        }
    }
    assert!(on_ai > 0, "no rider has a spot on the AI net");

    // The original's locate on the image's own nets against the port's on the nets off the disc.
    let mut pick = Pick(0x5282_c0de_7a11_0001);
    let mut placed = 0;
    for (name, net, paths, nodes) in [("sx01.tdf", &race, &race_paths, &race_nodes), ("sx01AI.tdf", &ai, &ai_paths, &ai_nodes)] {
        for case in 0..5_000 {
            let k = pick.below(net.paths.len() as u32) as usize;
            let n = &net.nodes[net.paths[k].first + pick.below(paths_len(net, k) as u32) as usize];
            let side = pick.below(3) as usize;
            let base = if side == 0 { n.centre } else { n.edges[side - 1] };
            let point = Vf([base.x() + pick.range(-60.0, 60.0), base.y(), base.z() + pick.range(-60.0, 60.0), 1.0]);
            let limit = if pick.chance(3) { pick.range(0.0, 300.0) } else { -1.0 };
            let holding_only = pick.chance(3);
            put(&mut ee, PATH_POINT, point);
            for o in [0, 4, 8] {
                ee.set_u32(PATH_OUT + o, UNTOUCHED);
            }
            ee.set_u32(PATH_DIST, UNTOUCHED);
            ee.call(
                locate,
                &[0, 0, u64::from(PATH_POINT), u64::from(paths[k]), u64::from(PATH_OUT), u64::from(PATH_DIST), 0, u64::from(holding_only)],
                &[limit],
                BUDGET,
            );
            let ours = path::locate(net, net.paths[k].first, point, F(limit), holding_only);
            let w = format!("{name} case {case}: path {k}, point {point:?}, limit {limit}, holding only {holding_only}: {ours:?}");
            assert_eq!(ee.v0(), 1, "{w}: locate's answer");
            match ours.place {
                Some(pl) => {
                    assert_eq!(ee.u32_at(PATH_OUT + 4), nodes[pl.node], "{w}: the node");
                    assert_eq!(ee.u32_at(PATH_OUT + 8), pl.along.to_bits(), "{w}: how far along");
                    placed += 1;
                }
                None => assert_eq!(ee.u32_at(PATH_OUT + 4), UNTOUCHED, "{w}: a node written"),
            }
            assert_eq!(ee.u32_at(PATH_DIST), ours.distance.to_bits(), "{w}: the distance");
        }
    }
    assert!(placed > 5_000, "only {placed} points placed");
}

/// How many nodes path `k` of `net` has.
fn paths_len(net: &Net, k: usize) -> usize {
    let mut n = 1;
    let mut at = net.paths[k].first;
    while let Some(j) = net.nodes[at].next {
        n += 1;
        at = j;
    }
    n
}
