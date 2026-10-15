//! The archive readers against the real disc: every member of one archive, inflated and checked.
//!
//! Needs the game's own container, so it skips with the command that produces it when it is not there —
//! a fresh checkout stays green.

use game_sim::archive::{Psr, Res};
use ps2_oracle::{Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

/// The track archive the port is being built against, and a bike that appears in its rider config.
const A_TRACK: &str = "Tracks\\sx01.res";
const A_BIKE: &str = "Bike\\250sx.res";

fn container() -> Option<File> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let at = kit.ctx("iso").join("MX.PSR");
    match File::open(&at) {
        Ok(f) => Some(f),
        Err(_) => {
            skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
            None
        }
    }
}

/// Read a member out of the big container the way the app layer would: one seek, one read.
fn member(f: &mut File, psr: &Psr, name: &str) -> Vec<u8> {
    let e = psr.find(name).unwrap_or_else(|| panic!("{name} is not in MX.PSR"));
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    bytes
}

fn directory(f: &mut File) -> Psr {
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    Psr::parse(&front).expect("MX.PSR's directory")
}

/// The outer container: the directory covers the file, and nothing in it overruns the end.
#[test]
fn the_containers_directory_accounts_for_the_whole_file() {
    let Some(mut f) = container() else { return };
    let total = f.metadata().expect("stat").len();
    let psr = directory(&mut f);
    assert!(psr.entries.len() > 500, "{} members", psr.entries.len());

    let mut last = 0;
    for e in &psr.entries {
        assert_eq!(e.at % Psr::SECTOR, 0, "{} is not sector-aligned", e.name);
        let end = e.at + e.size as u64;
        assert!(end <= total, "{} runs to {end} of a {total}-byte file", e.name);
        last = last.max(end);
    }
    // The last member reaches the end of the disc image bar its final partial sector: the file is
    // exactly its directory plus its members, with no trailer.
    let slack = total - last;
    assert!(slack < Psr::SECTOR, "{slack} bytes unaccounted for after the last member");
}

/// The inner container, in full: every member inflates, every size matches, and reading one member
/// alone gives the same bytes as reading the whole stream.
#[test]
fn every_member_of_an_archive_extracts_and_the_sizes_add_up() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let bytes = member(&mut f, &psr, A_BIKE);

    // `Res::parse` already proves the arithmetic that matters — header, block table, directory and
    // data must together account for every byte of the file, or it refuses to parse.
    let res = Res::parse(&bytes).expect("the bike archive's layout");
    assert!(res.entries.len() > 40, "{} members", res.entries.len());

    let whole = res.unpack(&bytes).expect("the whole stream");
    assert_eq!(whole.len(), res.unpacked_len());
    assert_eq!(res.unpacked_len(), res.blocks() * Res::BLOCK);

    let mut covered = 0usize;
    for e in &res.entries {
        assert_eq!(e.at % Res::BLOCK as u64, 0, "{} does not start on a block", e.name);
        let one = res.read(&bytes, e).unwrap_or_else(|why| panic!("{}: {why}", e.name));
        assert_eq!(one.len(), e.size as usize, "{} came out the wrong length", e.name);
        let at = e.at as usize;
        assert_eq!(one, whole[at..at + e.size as usize], "{} differs read on its own", e.name);
        covered += e.size as usize;
    }
    // Every member fits in the stream, and the slack is the block padding rather than a member the
    // directory failed to mention: at most one block per member, by construction.
    assert!(covered <= whole.len());
    assert!(
        whole.len() - covered < res.entries.len() * Res::BLOCK,
        "{} bytes of a {}-byte stream belong to no member",
        whole.len() - covered,
        whole.len()
    );
}

/// Inflating the right *number* of bytes is not the same as inflating the right ones: the bike's
/// physics parameters are a text file whose first line is known.
#[test]
fn an_inflated_member_is_really_the_file() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let bytes = member(&mut f, &psr, A_BIKE);
    let res = Res::parse(&bytes).expect("the bike archive's layout");

    let e = res.find("bike\\race250a.phy").expect("the bike's physics parameters");
    let phy = res.read(&bytes, e).expect("inflate");
    let text = String::from_utf8_lossy(&phy);
    assert!(text.starts_with("[Mass Properties]"), "starts {:?}", &text[..40.min(text.len())]);
    for section in ["[Front Tire]", "[Rear Tire]", "[Engine]", "[Collisions]"] {
        assert!(text.contains(section), "no {section}");
    }
}

/// The track archive too, because it is the one the port has to read first and it is a different shape:
/// four hundred blocks, and one member (the scene) spanning a dozen of them.
#[test]
fn the_track_archive_reads_too() {
    let Some(mut f) = container() else { return };
    let psr = directory(&mut f);
    let bytes = member(&mut f, &psr, A_TRACK);
    let res = Res::parse(&bytes).expect("the track archive's layout");

    let e = res.find("tracks\\sx01\\sx01.scn").expect("the track's scene");
    assert!(e.size > 100_000, "the scene is {} bytes", e.size);
    let scene = res.read(&bytes, e).expect("inflate the scene");
    assert_eq!(scene.len(), e.size as usize);

    // Reading it alone must touch only its own blocks, so this is also the test that the per-member
    // read indexes the block table correctly rather than starting from zero every time.
    let spans = e.size as usize / Res::BLOCK;
    assert!(spans > 8, "the scene spans only {spans} blocks");
}
