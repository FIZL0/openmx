//! V70: the surface byte each wheel leaves is the zone the tire tracks read.
//!
//! The original's tire records (the particle manager's, which the `TireTrack` entries gate on) point
//! at the rider's snapshot, and the snapshot's two zone bytes are copied straight out of each wheel's
//! surface byte, the one the heightfield query writes at the end of the suspension ray (S89). The
//! port's tracks take `ride::Wheel::surface`, which is that byte, so the two have to agree on every
//! tick. `context/capture/v70/animandpump.csv` is the human's animandpump played back on the
//! original with `watch/v70-zones.toml` (journal 2026-10-27-v70-tire-tracks-beside): each wheel's
//! surface byte and the ray's end it was asked at, the snapshot's zone bytes and the manager's own.
//! It rides free09's dirt (13) and crosses its road (0) twice.
//!
//! The capture and the disc are not in the repo: without them the test says so and passes.

use game_sim::archive::{Entry, Psr, Res};
use game_sim::track::{Scene, Terrain};
use game_sim::world::{Ground, Place, Props, World};
use ps2_float::Vf;
use ps2_oracle::{Kit, skip};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

fn member(f: &mut File, psr: &Psr, name: &str) -> Vec<u8> {
    let e = psr.find(name).unwrap();
    f.seek(SeekFrom::Start(e.at)).unwrap();
    let mut b = vec![0u8; e.size as usize];
    f.read_exact(&mut b).unwrap();
    b
}

fn by_leaf<'a>(res: &'a Res, leaf: &str) -> Option<&'a Entry> {
    let want = leaf.to_ascii_lowercase();
    res.entries.iter().find(|e| e.name.to_ascii_lowercase() == want)
}

/// The capture's rows, by column name.
fn rows(text: &str) -> Vec<HashMap<&str, &str>> {
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap().split(',').collect();
    lines.map(|l| head.iter().copied().zip(l.split(',')).collect()).collect()
}

/// **The port's terrain query, asked at the original's own ray ends, answers the original's surface
/// byte on every row, and that byte is the zone the tracks read.** Per row and wheel: the snapshot's
/// zone byte (rider `+0x910`/`+0x911`) is the wheel's surface byte, and [`World::wheel_surface`] on
/// free09's terrain at the ray's end gives the same byte. The manager's own copy of the zone is the
/// snapshot's of the row before or of the row itself (it reads the snapshot once a frame, and a row is
/// read at one of three phases of the frame), except on the one row it holds `0xff`.
#[test]
fn the_wheels_surface_byte_is_the_zone_the_tracks_read() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(w) => return skip(&w),
    };
    let path = kit.ctx("capture/v70/animandpump.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return skip(&format!("no {}: V70's journal says how it is recorded", path.display()));
    };
    let mut f = match File::open(kit.ctx("iso").join("MX.PSR")) {
        Ok(f) => f,
        Err(e) => return skip(&format!("no disc container: {e}")),
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).unwrap();
    let psr = Psr::parse(&front).unwrap();
    let file = member(&mut f, &psr, "Tracks\\free09.res");
    let res = Res::parse(&file).unwrap();
    let scene = Scene::parse(&res.read(&file, by_leaf(&res, "tracks\\free09\\free09.scn").unwrap()).unwrap()).unwrap();
    let trn = res.read(&file, by_leaf(&res, "tracks\\free09\\free09.ps2.trn").unwrap()).unwrap();
    let terrain = Terrain::parse(&trn).unwrap();
    let mut place = Place { ground: Ground::new(&terrain, &scene.terrain), props: Props::default() };

    let rows = rows(&text);
    assert!(rows.len() >= 1000, "{} rows", rows.len());
    let byte = |r: &HashMap<&str, &str>, k: &str| -> u8 { r[k].parse().unwrap_or_else(|_| panic!("{k} = {:?}", r[k])) };
    let at = |r: &HashMap<&str, &str>, k: &str| {
        let c = |a: &str| r[format!("{k}.{a}").as_str()].parse::<f32>().unwrap();
        Vf::new(c("x"), c("y"), c("z"), 1.0)
    };
    let (mut seen, mut bad, mut held) = (BTreeMap::new(), Vec::new(), 0);
    for (k, r) in rows.iter().enumerate() {
        let counter = r["frame_counter"];
        for (wheel, zone, ours) in [("f", "s910", "t0_zone"), ("r", "s911", "t1_zone")] {
            let surface = byte(r, &format!("{wheel}_surface"));
            *seen.entry(surface).or_insert(0) += 1;
            assert_eq!(byte(r, zone), surface, "{counter} {wheel}: the snapshot's zone byte is the wheel's surface");
            let port = place.wheel_surface(at(r, &format!("{wheel}_end")));
            if port != Some(surface) {
                bad.push(format!("{counter} {wheel}: port {port:?}, original {surface}"));
            }
            if k > 0 {
                match byte(r, ours) {
                    0xff => held += 1,
                    z => assert!(
                        z == byte(&rows[k - 1], zone) || z == byte(r, zone),
                        "{counter} {wheel}: the manager's zone {z} is neither the row before's nor this row's"
                    ),
                }
            }
        }
    }
    println!("{} rows, surfaces {seen:?}, {held} manager zones held at 0xff", rows.len());
    assert!(bad.is_empty(), "{} of {} wheel rows part:\n{}", bad.len(), 2 * rows.len(), bad.join("\n"));
    assert!(seen.get(&0).is_some_and(|&n| n >= 100) && seen.get(&13).is_some_and(|&n| n >= 100), "both the dirt and the road: {seen:?}");
    assert!(held <= 2, "{held} manager zones at 0xff");
}
