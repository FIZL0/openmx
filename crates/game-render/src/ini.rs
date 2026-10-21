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
}
