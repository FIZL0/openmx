//! MD98a — two quads' collision records resolving their contacts with each other, against C38's image.
//!
//! C38's `c38-37523.ee.bin` is an ATV race (Desert Oasis, SLUS-21104) on the frame two AI quads touch:
//! each one's record holds four contacts naming the other's record (research/oracle.txt `# C38`), its
//! own (kind 0) and the mirrors of the other's (kind 1). The port refills each record's list from those
//! contacts with the other quad's record and body held ([`Held::refill_among`]) and resolves it as the
//! gravity's call does ([`Held::resolve`]); the original runs the record's vt `+0x88` handed 1. First
//! the one record's call, then the other's on what the first left: after each, every word the port
//! writes (both records' centres in the world, normal speed and relative velocity, the caller's entries'
//! arms and relative velocities, both bodies whole) is the original's, and the original writes nothing
//! else outside its stack.

use super::*;
use game_sim::atv_contact::{Held, Knocked};
use game_sim::handling::record::{self, Contact as Filed};

/// The touching pair's records (research/oracle.txt `# C38`), and SLUS-21104's `$gp`.
const RECORDS: [u32; 2] = [0x01a5_11b0, 0x01aa_5020];
const GP: u64 = 0x003d_baf0;

fn c38_image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2/c38").join(format!("c38-37523.ee{}", ".bin"));
    if !path.exists() {
        skip(&format!("{} is not there (C38's touch image, research/c38)", path.display()));
        return None;
    }
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, GP);
    Some(ee)
}

fn v(ee: &Ee, a: u32) -> Vf {
    Vf([0, 4, 8, 12].map(|k| f32::from_bits(ee.u32_at(a + k))))
}

/// The record at `rec` as [`record::held`] reads it, its list left out.
fn held_bare(ee: &Ee, rec: u32) -> Held {
    let w = |a: u32| if a == rec + 0xf0 { 0 } else { ee.u32_at(a) };
    record::held(&w, rec).expect("a list left out")
}

/// The record at `rec` and its body as another quad's resolution pushes them.
fn knocked(ee: &Ee, rec: u32) -> Knocked {
    let h = held_bare(ee, rec);
    Knocked { collider: h.record.collider, body: h.body.expect("a body"), mass: h.mass, cofg: v(ee, ee.u32_at(rec + 0xe4) + 0x1a0) }
}

/// The contacts in the record's list at `rec` as its pass filed them.
fn filed(ee: &Ee, rec: u32) -> Vec<Filed> {
    let list = ee.u32_at(rec + 0xf8);
    (0..ee.u32_at(rec + 0xf0))
        .map(|k| {
            let e = list + record::CONTACT * k;
            Filed {
                kind: ee.u32_at(e),
                from: v(ee, e + 0x10),
                to: v(ee, e + 0x20),
                normal: v(ee, e + 0x30),
                point: v(ee, e + 0x40),
                surface: ee.u32_at(e + 0x80),
                other: ee.u32_at(e + 0x84),
                mover: ee.u32_at(e + 0x88),
            }
        })
        .collect()
}

/// The record at `rec` as the gravity's call holds it, refilled from its list with `other` (the other
/// quad's record, as the port last left it) held; each entry's relative velocity the list slot's.
fn held(ee: &Ee, rec: u32, other_rec: u32, other: Knocked) -> Held {
    let mut h = held_bare(ee, rec);
    let list = ee.u32_at(rec + 0xf8);
    h.entries = (0..ee.u32_at(rec + 0xf0))
        .map(|k| game_sim::atv_contact::Entry { relative: v(ee, list + record::CONTACT * k + 0x70), ..Default::default() })
        .collect();
    assert!(h.refill_among(&filed(ee, rec), vec![other], &[other_rec]), "the list naming the other quad was refused");
    h
}

/// **Two AI quads' records resolve their contacts with each other as the original's** (C38's touch).
#[test]
fn two_quads_records_resolve_their_touch_as_the_originals() {
    let Some(mut ee) = c38_image() else { return };
    let resolve_at = ps2_oracle::addr("atv_record_resolve").unwrap();
    for (k, &rec) in RECORDS.iter().enumerate() {
        let other = RECORDS[1 - k];
        assert_eq!(ee.u32_at(ee.u32_at(rec + 4) + 0x88), resolve_at, "{rec:#x}'s vt +0x88");
        assert_eq!(ee.u32_at(ee.u32_at(rec + 0xe4) + 0xe8), 0, "{rec:#x}'s body has listeners");
        let f = filed(&ee, rec);
        assert_eq!(f.len(), 4, "{rec:#x}'s list");
        assert!(f.iter().all(|c| c.other == other && c.mover == 0), "{rec:#x}'s contacts are not all with {other:#x}");
    }
    let kinds = |rec: u32| filed(&ee, rec).iter().map(|c| c.kind).collect::<Vec<_>>();
    assert_eq!((kinds(RECORDS[0]), kinds(RECORDS[1])), (vec![0, 1, 1, 1], vec![1, 0, 0, 0]), "the lists' kinds");

    // The port carries the other quad's record and body from one call to the next.
    let mut theirs = knocked(&ee, RECORDS[1]);
    for (k, &rec) in RECORDS.iter().enumerate() {
        let other = RECORDS[1 - k];
        let mut port = held(&ee, rec, other, theirs);
        assert!(port.entries.iter().all(|e| e.contact.other == Some(0)), "{rec:#x}: an entry not held with the other quad");
        let cofg = v(&ee, ee.u32_at(rec + 0xe4) + 0x1a0);
        let before = (port.body, port.bodies[0].body);
        assert!(port.resolve(cofg), "{rec:#x}: the port did not resolve");
        assert_ne!((port.body, port.bodies[0].body), before, "{rec:#x}: neither body was pushed");

        ee.written.clear();
        ee.call(resolve_at, &[u64::from(rec), 1], &[], 400_000);

        let mut compared = std::collections::BTreeSet::new();
        let pushed = port.bodies[0];
        let other_held = Held { record: game_sim::atv_contact::Record { collider: pushed.collider, ..Default::default() }, body: Some(pushed.body), ..Default::default() };
        let words = |a: u32| ee.u32_at(a);
        let mine: Vec<(u32, u32)> = record::held_words(&port, rec, &words).into_iter().chain(record::held_words(&other_held, other, &words)).collect();
        for (a, w) in mine {
            assert_eq!(w, ee.u32_at(a), "{rec:#x}'s call: the port wrote {w:#010x} at {a:#x}, the original {:#010x}", ee.u32_at(a));
            compared.extend(a..a + 4);
        }
        let stray: Vec<u32> = ee.written.iter().copied().filter(|&a| !(ee.stack - 0x4000..ee.stack).contains(&a) && !compared.contains(&a)).collect();
        assert!(stray.is_empty(), "{rec:#x}'s call: the original wrote what the port does not: {:x?}", &stray[..stray.len().min(16)]);

        // The next call's other is this one's record, as the port left it.
        theirs = Knocked { collider: port.record.collider, body: port.body.expect("the body"), mass: port.mass, cofg };
    }
}
