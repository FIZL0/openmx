//! MD69 — MX vs. ATV Untamed's stunt table is a BPB (`Rider\Anim\MX_Rider_Stunts.ini.bpb`, 54 stunts,
//! titled `[Stunt1]`, not `[Stunt01]`), and a trick pack carrying it loads.
//!
//! What it holds:
//! - a BPB stunt table generated here, `Stunt1`..`Stunt54` in Untamed's layout (every value type the
//!   format has: strings in the value table, short strings in the record, ints, floats), reads as 54
//!   rows with each value bit for bit, `DefaultKeyMappingClassic` as the gesture;
//! - the same table as text, MX vs. ATV Unleashed's `[Stunt01]` layout, reads to the same rows, so the
//!   text tables load as they did;
//! - a BPB that is cut short or has a type the format lacks is refused, not read as an empty table;
//! - Untamed's own table, when `mods/tricks/untamed_tricks` is installed, loads with its 54 stunts.

use game_sim::bpb;
use game_sim::trick_pack::{self, Row};
use ps2_float::F;

enum V {
    Text(&'static str),
    Short(&'static str),
    Int(i32),
    Float(f32),
}

/// Write a BPB of `(name, index, keys)` sections the way the disc's are laid out: header, section
/// records, key records, then the name table and the value strings, each padded to 16.
fn encode(sections: &[(&str, Option<u16>, Vec<(&str, V)>)]) -> Vec<u8> {
    fn intern(table: &mut Vec<u8>, s: &str) -> u32 {
        let mut at = 0;
        while at < table.len() {
            let end = at + table[at..].iter().position(|&c| c == 0).unwrap();
            if &table[at..end] == s.as_bytes() {
                return at as u32;
            }
            at = end + 1;
        }
        let at = table.len() as u32;
        table.extend_from_slice(s.as_bytes());
        table.push(0);
        at
    }
    let pad = |v: &mut Vec<u8>| v.resize(v.len().div_ceil(16) * 16, 0);
    let (mut names, mut values, mut secs, mut keys) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut n_keys = 0u32;
    for (name, index, list) in sections {
        secs.extend_from_slice(&bpb::hash(name.as_bytes()).to_le_bytes());
        secs.extend_from_slice(&intern(&mut names, name).to_le_bytes());
        secs.extend_from_slice(&index.unwrap_or(0xffff).to_le_bytes());
        secs.extend_from_slice(&(list.len() as u16).to_le_bytes());
        secs.extend_from_slice(&n_keys.to_le_bytes());
        for (key, value) in list {
            keys.extend_from_slice(&bpb::hash(key.as_bytes()).to_le_bytes());
            keys.extend_from_slice(&intern(&mut names, key).to_le_bytes());
            let (kind, tail): (u16, [u8; 4]) = match value {
                V::Text(s) => (1, intern(&mut values, s).to_le_bytes()),
                V::Short(_) => (2, [0; 4]),
                V::Int(i) => (4, i.to_le_bytes()),
                V::Float(f) => (8, f.to_bits().to_le_bytes()),
            };
            keys.extend_from_slice(&kind.to_le_bytes());
            if let V::Short(s) = value {
                assert!(s.len() <= 5);
                let mut raw = [0u8; 6];
                raw[..s.len()].copy_from_slice(s.as_bytes());
                keys.extend_from_slice(&raw);
            } else {
                keys.extend_from_slice(&[0, 0]);
                keys.extend_from_slice(&tail);
            }
            n_keys += 1;
        }
    }
    pad(&mut names);
    pad(&mut values);
    let key_off = 0x20 + secs.len();
    let names_off = key_off + keys.len();
    let values_off = names_off + names.len();
    let mut out = Vec::new();
    out.extend_from_slice(bpb::MAGIC);
    for word in [0x20u32, key_off as u32] {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out.extend_from_slice(&(sections.len() as u16).to_le_bytes());
    out.extend_from_slice(&(n_keys as u16).to_le_bytes());
    for word in [names_off, names.len(), values_off, values.len()] {
        out.extend_from_slice(&(word as u32).to_le_bytes());
    }
    out.extend(secs);
    out.extend(keys);
    out.extend(names);
    out.extend(values);
    out
}

/// One stunt of the generated table: Untamed's keys, the last two its 360s.
fn stunt(i: u16) -> Vec<(&'static str, V)> {
    let name: &'static str = match i {
        53 => "threesixtyL",
        54 => "threesixtyR",
        _ => Box::leak(format!("stunt{i}").into_boxed_str()),
    };
    let mut keys = vec![
        ("Name", V::Text(name)),
        ("DisplayName", V::Text(Box::leak(format!("Stunt Number {i}").into_boxed_str()))),
        ("HoldTime", V::Float(0.25 * f32::from(i))),
        ("SafeTime", V::Float(0.5 + 0.25 * f32::from(i))),
        ("Duration", V::Float(0.75 + 0.25 * f32::from(i))),
        ("UsedForBike", V::Short(if i % 7 == 0 { "F" } else { "T" })),
        ("HasAnimation", V::Short(if i % 5 == 0 { "False" } else { "T" })),
        ("PointValue", V::Int(100 * i32::from(i))),
        ("HoldPointValue", V::Float(1000.0 + f32::from(i))),
    ];
    if i == 3 {
        keys.push(("DefaultKeyMappingClassic", V::Short("1CR")));
    }
    keys
}

fn untamed_like() -> Vec<u8> {
    let mut sections: Vec<(&str, Option<u16>, Vec<(&str, V)>)> = (1..=54).map(|i| ("Stunt", Some(i), stunt(i))).collect();
    sections.push(("Depreciation", None, vec![("Rate", V::Float(0.4))]));
    encode(&sections)
}

/// The same stunts as an MX vs. ATV Unleashed text table: `[Stunt01]`, `DefaultKeyMapping`.
fn as_text() -> String {
    let mut out = String::new();
    for i in 1..=54u16 {
        out.push_str(&format!("[Stunt{i:02}]\r\n"));
        for (key, value) in stunt(i) {
            let key = if key == "DefaultKeyMappingClassic" { "DefaultKeyMapping" } else { key };
            let value = match value {
                V::Text(s) => format!("\"{s}\""),
                V::Short(s) => s.to_owned(),
                V::Int(n) => n.to_string(),
                V::Float(f) => format!("{f}"),
            };
            out.push_str(&format!("{key}={value}\r\n"));
        }
        out.push_str("\r\n");
    }
    out
}

#[test]
fn a_bpb_stunt_table_loads_as_a_pack_of_54() {
    let rows = trick_pack::table(&untamed_like()).expect("the generated BPB table reads");
    assert_eq!(rows.len(), 54);
    for (k, r) in rows.iter().enumerate() {
        let i = k as u16 + 1;
        assert_eq!(r.stunt.index, k as i32);
        assert_eq!(r.stunt.display_name, format!("Stunt Number {i}"));
        assert_eq!(r.stunt.hold_time, F(0.25 * f32::from(i)));
        assert_eq!(r.stunt.safe_time, F(0.5 + 0.25 * f32::from(i)));
        assert_eq!(r.stunt.duration, F(0.75 + 0.25 * f32::from(i)));
        assert_eq!(r.stunt.for_bike, i % 7 != 0, "Stunt{i} UsedForBike");
        assert_eq!(r.has_animation, i % 5 != 0, "Stunt{i} HasAnimation");
        assert_eq!(r.point_value, F(100.0 * f32::from(i)));
        assert_eq!(r.hold_point_value, F(1000.0 + f32::from(i)));
        assert_eq!(r.mapping, if i == 3 { "1CR" } else { "" });
    }
    assert_eq!(rows[52].stunt.name, "threesixtyL");
    assert_eq!(rows[53].stunt.name, "threesixtyR");
    // The 360 rows score their HoldPointValue, as the text pack's do.
    let r = trick_pack::row(&rows, "THREESIXTYR").expect("the 360 right row");
    assert_eq!(r.hold_point_value, F(1054.0));
}

#[test]
fn the_text_table_reads_to_the_same_rows() {
    let text: Vec<Row> = trick_pack::table(as_text().as_bytes()).expect("the text table reads");
    let bpb: Vec<Row> = trick_pack::table(&untamed_like()).expect("the BPB table reads");
    assert_eq!(text.len(), 54);
    assert_eq!(text, bpb);
}

#[test]
fn a_bpb_float_reads_back_to_its_bits() {
    // 0.4 is one the text reader's truncating read would put a bit below if it were written `0.4`.
    let fs = [0.4f32, 0.1, 1.0e-7, -3.25, 123456.79, f32::MIN_POSITIVE, 16777216.0];
    for f in fs {
        let one = encode(&[("S", None, vec![("K", V::Float(f))])]);
        let ini = bpb::parse(&one).expect("reads");
        assert_eq!(ini.section("s").float("k"), Some(F(f)), "{f}");
    }
}

#[test]
fn a_broken_bpb_is_refused() {
    let whole = untamed_like();
    assert!(trick_pack::table(&whole[..0x40]).is_err(), "cut short");
    let mut odd = encode(&[("Stunt", Some(1), vec![("Name", V::Int(1))])]);
    let key = u32::from_le_bytes(odd[8..12].try_into().unwrap()) as usize;
    odd[key + 8] = 3; // a type the format does not have
    let e = trick_pack::table(&odd).expect_err("type 3");
    assert!(e.contains("type 3"), "{e}");
    // A table with neither spelling of the first stunt still says so.
    let none = encode(&[("Stunt", Some(0), vec![("Name", V::Text("x"))])]);
    let e = trick_pack::table(&none).expect_err("no Stunt1");
    assert!(e.contains("[Stunt01]"), "{e}");
}

#[test]
fn untamed_s_own_table_loads() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../mods/tricks/untamed_tricks/Rider/Anim/MX_Rider_Stunts.ini.bpb");
    let Ok(bytes) = std::fs::read(&path) else {
        eprintln!("skipped: no {}", path.display());
        return;
    };
    let rows = trick_pack::table(&bytes).expect("Untamed's table reads");
    assert_eq!(rows.len(), 54);
    let varial = trick_pack::row(&rows, "Varial").expect("Varial, Stunt37");
    assert_eq!(varial.stunt.index, 36);
    assert_eq!(varial.stunt.duration, F(1.84));
    assert_eq!(rows[52].stunt.name, "threesixtyL");
    assert_eq!(rows[53].stunt.name, "threesixtyR");
    assert_eq!(rows[53].hold_point_value, F(15000.0));
}
