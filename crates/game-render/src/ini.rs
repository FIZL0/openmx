//! The plain-text format the disc uses for anything that is not geometry: `[Section]` headings,
//! `key = value` lines, `;` comments.
//!
//! Both `.psm` (a material) and `.scn` (a scene) are written in it, and a scene repeats the same
//! section name pattern 1506 times (`[Model1]`, `[Model2]`, …), so the parse keeps sections in file
//! order rather than folding them into a map.

/// One `[Section]` and its lines, in file order.
#[derive(Clone, Debug, Default)]
pub struct Section {
    pub name: String,
    pub keys: Vec<(String, String)>,
}

impl Section {
    /// The first value of that key, case-insensitively.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    /// A key as a number, if it is one.
    pub fn number(&self, key: &str) -> Option<f32> {
        self.get(key)?.trim().parse().ok()
    }

    /// A key as a comma-separated list of numbers: `Position=101.49,90.12,458.48`.
    pub fn numbers(&self, key: &str) -> Option<Vec<f32>> {
        let v = self.get(key)?;
        v.split(',').map(|p| p.trim().parse().ok()).collect()
    }
}

/// Parse the whole file. Lines before the first heading are kept under an empty section name.
pub fn parse(text: &str) -> Vec<Section> {
    let mut out: Vec<Section> = vec![Section::default()];
    for line in text.lines() {
        // A comment runs to the end of the line, and some lines are only a comment.
        let line = match line.find(';') {
            Some(at) => &line[..at],
            None => line,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push(Section {
                name: name.trim().to_string(),
                keys: Vec::new(),
            });
        } else if let Some((k, v)) = line.split_once('=') {
            if let Some(s) = out.last_mut() {
                s.keys.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    out
}

/// A switch the way the original's reader takes one (P61), ignoring case: a value starting with `T` is
/// on and one starting with `F` off, then `ON` and `OFF` whole, then `Y` and `N` by the first letter,
/// and anything else is read as a number, on when it is not 0. So `TRUE` is on, and so is `1`.
pub fn switch(value: &str) -> bool {
    let v = value.trim().to_ascii_uppercase();
    match () {
        _ if v.starts_with('T') => true,
        _ if v.starts_with('F') => false,
        _ if v == "ON" => true,
        _ if v == "OFF" => false,
        _ if v.starts_with('Y') => true,
        _ if v.starts_with('N') => false,
        _ => {
            // A C `strtol` with base 0: a sign, then hex after `0x`, octal after a `0`, or decimal,
            // as far as the digits go.
            let (negative, digits) = match v.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, v.strip_prefix('+').unwrap_or(&v)),
            };
            let (radix, digits) = match digits.strip_prefix("0X") {
                Some(hex) => (16, hex),
                None if digits.starts_with('0') => (8, digits),
                None => (10, digits),
            };
            let end = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());
            let n = i64::from_str_radix(&digits[..end], radix).unwrap_or(0);
            (if negative { -n } else { n }) != 0
        }
    }
}

/// Decode as text the way the disc writes it: ASCII, with the odd high byte in a name.
pub fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// An INI file's bytes as text: MX vs. ATV Untamed's compiled INI (a BPB, `game_sim::bpb`:
/// `ATV_R_250_KTM.sub.bpb`, `Striker_Plastics.psm.bpb`, MD144b) written back as the text of its
/// sections and pairs, each section in file order (an empty one too); anything else as [`text`].
/// A BPB the reader refuses comes out empty, as a file with nothing in it reads.
pub fn text_of(bytes: &[u8]) -> String {
    if !game_sim::bpb::is_bpb(bytes) {
        return text(bytes);
    }
    let Ok(ini) = game_sim::bpb::parse(bytes) else {
        return String::new();
    };
    let mut out = String::new();
    let mut pairs = ini.entries().peekable();
    for name in ini.names() {
        out += &format!("[{name}]\n");
        while let Some((_, key, value)) = pairs.next_if(|(s, _, _)| *s == name.as_str()) {
            out += &format!("{key}={value}\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_switch_reads_like_the_originals() {
        for on in ["T", "TRUE", "t", " True ", "ON", "on", "Y", "yes", "1", "-2", "0x10", "7abc"] {
            assert!(switch(on), "{on:?}");
        }
        for off in ["F", "FALSE", "f", "OFF", "N", "no", "0", "", "abc", "0x0", "00"] {
            assert!(!switch(off), "{off:?}");
        }
    }

    #[test]
    fn sections_keep_their_order_and_lose_their_comments() {
        let s = parse(
            "[SceneInfo]\nSceneName=Supercross01\n; a comment\n\n[Model1]\nSLT=bale_single_02  ; and here\nPosition=627.03,94.28,651.93\n",
        );
        assert_eq!(s.len(), 3, "the preamble, then two sections");
        assert_eq!(s[1].name, "SceneInfo");
        assert_eq!(s[1].get("scenename"), Some("Supercross01"));
        assert_eq!(s[2].get("SLT"), Some("bale_single_02"));
        assert_eq!(
            s[2].numbers("Position"),
            Some(vec![627.03, 94.28, 651.93])
        );
        assert_eq!(s[2].get("missing"), None);
    }

    /// A BPB of `(name, index, keys)` sections as Untamed's are laid out (`game_sim::bpb`): a key's
    /// value a long string (type 1), a short one (2), an int (4) or a float (8).
    fn bpb(sections: &[(&str, Option<u16>, &[(&str, u16, &str)])]) -> Vec<u8> {
        let intern = |table: &mut Vec<u8>, s: &str| {
            let at = table.len() as u32;
            table.extend_from_slice(s.as_bytes());
            table.push(0);
            at
        };
        let (mut names, mut values, mut secs, mut keys) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        let mut n_keys = 0u32;
        for (name, index, pairs) in sections {
            secs.extend_from_slice(&0u32.to_le_bytes());
            secs.extend_from_slice(&intern(&mut names, name).to_le_bytes());
            secs.extend_from_slice(&index.unwrap_or(0xffff).to_le_bytes());
            secs.extend_from_slice(&(pairs.len() as u16).to_le_bytes());
            secs.extend_from_slice(&n_keys.to_le_bytes());
            for &(key, kind, value) in pairs.iter() {
                let mut r = [0u8; 16];
                r[4..8].copy_from_slice(&intern(&mut names, key).to_le_bytes());
                r[8..10].copy_from_slice(&kind.to_le_bytes());
                match kind {
                    1 => r[12..16].copy_from_slice(&intern(&mut values, value).to_le_bytes()),
                    2 => r[10..10 + value.len()].copy_from_slice(value.as_bytes()),
                    4 => r[12..16].copy_from_slice(&value.parse::<i32>().unwrap().to_le_bytes()),
                    _ => r[12..16].copy_from_slice(&value.parse::<f32>().unwrap().to_le_bytes()),
                }
                keys.extend_from_slice(&r);
                n_keys += 1;
            }
        }
        let mut out = b"BPB\0".to_vec();
        let key_at = 0x20 + secs.len();
        let names_at = key_at + keys.len();
        let values_at = names_at + names.len();
        for w in [0x20, key_at] {
            out.extend_from_slice(&(w as u32).to_le_bytes());
        }
        out.extend_from_slice(&(sections.len() as u16).to_le_bytes());
        out.extend_from_slice(&(n_keys as u16).to_le_bytes());
        for w in [names_at, names.len(), values_at, values.len()] {
            out.extend_from_slice(&(w as u32).to_le_bytes());
        }
        for part in [secs, keys, names, values] {
            out.extend_from_slice(&part);
        }
        out
    }

    /// MD144b: a compiled `.sub` reads as its text would: sections in order, an empty one kept, an
    /// indexed one titled `SubObject1`, each value type as text, the text readers' pairs.
    #[test]
    fn a_bpb_reads_as_the_text_it_was_compiled_from() {
        let b = bpb(&[
            ("Objects", None, &[("MCF", 2, "a.mcf"), ("NumberOfSubObjects", 4, "1")]),
            ("Empty", None, &[]),
            ("Material Changes", None, &[("ATV_R_450f_KTM_Plastics", 1, "Striker_Plastics")]),
            ("SubObject", Some(1), &[("SLT", 1, "atv_r_250_plastics"), ("Scale", 8, "0.5")]),
        ]);
        let s = parse(&text_of(&b));
        let titled: Vec<&str> = s.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(titled, ["", "Objects", "Empty", "Material Changes", "SubObject1"]);
        assert_eq!((s[1].get("MCF"), s[1].number("NumberOfSubObjects")), (Some("a.mcf"), Some(1.0)));
        assert!(s[2].keys.is_empty());
        assert_eq!(s[3].keys, [("ATV_R_450f_KTM_Plastics".to_string(), "Striker_Plastics".to_string())]);
        assert_eq!((s[4].get("SLT"), s[4].number("Scale")), (Some("atv_r_250_plastics"), Some(0.5)));
        // Text stays text, and a BPB the reader refuses reads as nothing.
        assert_eq!(text_of(b"[A]\nB=1\n"), "[A]\nB=1\n");
        assert_eq!(text_of(&b[..0x10]), "");
    }
}
