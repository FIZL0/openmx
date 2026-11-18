//! **A free ride's progression kept per track** (P349): the save profile's map record that opens a
//! free ride's start zones ([`seek::open_flags`], S315) — a byte per hit and per run progression
//! place (bit 0 unlocked, bit 1 shown) and the race and stunt zones' shown words — kept beside
//! `settings.txt` in a `progress/` folder, `progress/<track>/<profile>.progress`, the profile the
//! rider's name or the seat (`saved_markers::profile`).
//!
//! A track with no file starts on a fresh profile's record ([`fresh`]: the first hit and the first
//! run place unlocked, nothing shown — what every free-ride image holds) and writes it, so the file is
//! there to keep what a ride later unlocks. A file of another format is said on the console, left
//! alone, and read as the fresh record.
//!
//! The record opens zones in the seeker world, which is simulation state, so only a local ride reads
//! the file: a played tape and an online ride start on the fresh record on every machine.

use std::path::{Path, PathBuf};

use game_sim::ai::seek::{Opening, Progress};

/// The folder beside `settings.txt` the progressions are kept in.
pub const DIR: &str = "progress";

/// The file's first line: its format.
const HEAD: &str = "openmx progress 1";

/// The map record's length as the zones read it: the race and stunt words at +0x14 and +0x18, five
/// hit places from +0x20 and five run places from +0x25.
pub const RECORD: usize = 0x30;

/// Where the record's first hit place and first run place are.
const FIRST_HIT: usize = 0x20;
const FIRST_RUN: usize = 0x25;

/// A fresh profile's record for a track: the first place of each progression unlocked (bit 0), none
/// shown, the race and stunt words 0.
pub fn fresh() -> Progress {
    let mut bytes = vec![0; RECORD];
    bytes[FIRST_HIT] = 1;
    bytes[FIRST_RUN] = 1;
    Progress(bytes)
}

/// A name as a file name: letters, digits and `-` kept, anything else `_`, so no track or rider name
/// reaches outside the folder.
fn file_word(word: &str) -> String {
    let w: String = word.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if w.is_empty() { "_".into() } else { w }
}

/// Where `profile`'s progression on `track` is kept, under the settings' folder `dir`.
pub fn path(dir: &Path, track: &str, profile: &str) -> PathBuf {
    dir.join(DIR).join(file_word(&track.to_ascii_lowercase())).join(format!("{}.progress", file_word(profile)))
}

/// The record as the file holds it: the format line, then its bytes in hex.
pub fn to_file(p: &Progress) -> String {
    let hex: String = p.0.iter().map(|b| format!("{b:02x}")).collect();
    format!("{HEAD}\n{hex}\n")
}

/// A file's record, or why it is not one.
pub fn from_file(text: &str) -> Result<Progress, String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(HEAD) {
        return Err("not this build's format".into());
    }
    let hex = lines.next().unwrap_or("").trim();
    if hex.len() % 2 != 0 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("its record is not hex bytes".into());
    }
    let bytes = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex digits")).collect();
    Ok(Progress(bytes))
}

/// Keep `p` as `profile`'s on `track`: the file written whole (a sibling, then a rename).
pub fn save(dir: &Path, track: &str, profile: &str, p: &Progress) -> Result<(), String> {
    let file = path(dir, track, profile);
    let parent = file.parent().expect("a file in a folder");
    crate::files::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let part = file.with_extension("progress.part");
    crate::files::write(&part, to_file(p)).map_err(|e| format!("{}: {e}", part.display()))?;
    crate::files::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))
}

/// The record `profile` kept on `track`: the file's, or with none the [`fresh`] one, written; a file
/// that is not a record is said and read as the fresh one, and left as it is.
pub fn load(dir: &Path, track: &str, profile: &str) -> Progress {
    let file = path(dir, track, profile);
    let Ok(bytes) = crate::files::read(&file) else {
        let p = fresh();
        if let Err(e) = save(dir, track, profile, &p) {
            println!("progress: not kept: {e}");
        }
        return p;
    };
    match from_file(&String::from_utf8_lossy(&bytes)) {
        Ok(p) => p,
        Err(e) => {
            println!("progress: {} not loaded: {e}", file.display());
            fresh()
        }
    }
}

/// What the zones' opening reads besides the record, for a free ride's load: nothing holds the zones
/// and no rider runs a hit progression (the app has neither), and the race zone is live unless the
/// player starts the ride on a machine (`on_machine`: the race activity's machine list is empty while
/// the player rides one).
pub fn opening(on_machine: bool) -> Opening {
    Opening { held: false, hit_running: false, race_live: !on_machine }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::ai::seek::{Activity, Zone, opener};

    fn zone(activity: Activity, place: Option<usize>) -> Zone {
        Zone { activity: Some(activity), number: 0, place }
    }

    #[test]
    fn fresh_record_opens_what_the_old_closure_did() {
        let p = fresh();
        let humans = [0];
        let open = opener(&humans, Some(&p), opening(false));
        for a in [Activity::Hit, Activity::Run] {
            assert!(open(0, zone(a, Some(0))), "{a:?} place 0");
            assert!(!open(0, zone(a, Some(1))), "{a:?} place 1");
            assert!(!open(0, zone(a, None)), "{a:?} no place");
        }
        assert!(open(0, zone(Activity::Race, Some(0))));
        assert!(open(0, zone(Activity::Stunt, Some(0))));
        assert!(!open(1, zone(Activity::Stunt, Some(0))), "player 1 is no human here");
        let flying = opener(&humans, Some(&p), opening(true));
        assert!(!flying(0, zone(Activity::Race, Some(0))), "the race is not live on a machine");
        assert!(flying(0, zone(Activity::Stunt, Some(0))));
    }

    #[test]
    fn a_record_round_trips_and_a_bad_file_is_refused() {
        let mut p = fresh();
        p.0[0x21] = 3;
        p.0[0x14] = 1;
        assert_eq!(from_file(&to_file(&p)), Ok(p));
        assert!(from_file("something else\n00").is_err());
        assert!(from_file(&format!("{HEAD}\n0g")).is_err());
    }

    #[test]
    fn names_stay_inside_the_folder() {
        let file = path(Path::new("/d"), "../Free09", "a/b");
        assert_eq!(file, Path::new("/d/progress/___free09/a_b.progress"));
    }
}
