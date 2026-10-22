//! **Session markers kept between sessions** (P99): each player's five slots ([`markers::Markers`])
//! saved per track beside `settings.txt`, in a `markers/` folder — `markers/<track>/<profile>.markers`,
//! the profile the rider's name or, with none, the seat (`1P`, `2P`, ..). A file is written when a
//! slot is placed or cleared and read into the seat's markers when a ride starts on its track.
//!
//! The file is [`markers::Markers::to_file`]'s: a version and the bike's `.phy` name before the
//! slots, so a file of another format or another bike is dropped — said on the console and left
//! alone until a slot overwrites it — rather than read as something it is not.
//!
//! The markers are simulation state, so the files only ever start a ride: a tape carries the ones its
//! ride started with ([`game_sim::replay::PadTape::markers`]) and replays without the files, and
//! online each player's go to every machine with their seat's pick ([`openmx_net::Pick::markers`]).

use std::path::{Path, PathBuf};

use game_sim::hash;
use game_sim::markers::Markers;

/// The folder beside `settings.txt` the markers are kept in.
pub const DIR: &str = "markers";

/// The profile a seat's markers are kept under: its rider's name (`name`, as the garage cleans it,
/// `roster::clean_rider_name`), or the seat (`1P`) with none.
pub fn profile(name: &str, seat: usize) -> String {
    match name.trim().is_empty() {
        true => format!("{}P", seat + 1),
        false => name.trim().to_string(),
    }
}

/// A name as a file name: letters, digits and `-` kept, anything else `_`, so no track or rider name
/// reaches outside the folder.
fn file_word(word: &str) -> String {
    let w: String = word.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    if w.is_empty() { "_".into() } else { w }
}

/// Where `profile`'s markers on `track` are kept, under the settings' folder `dir`.
pub fn path(dir: &Path, track: &str, profile: &str) -> PathBuf {
    dir.join(DIR).join(file_word(&track.to_ascii_lowercase())).join(format!("{}.markers", file_word(profile)))
}

/// The markers `profile` kept on `track` for `bike`: none without a file, and none — said why — for a
/// file that is not this build's format or is another bike's.
pub fn load(dir: &Path, track: &str, profile: &str, bike: &str) -> Markers {
    let file = path(dir, track, profile);
    let Ok(bytes) = std::fs::read(&file) else { return Markers::default() };
    match Markers::from_file(&bytes, bike) {
        Ok(m) => m,
        Err(e) => {
            println!("markers: {} not loaded: {e}", file.display());
            Markers::default()
        }
    }
}

/// Keep `markers`' slots as `profile`'s on `track`: the file written whole (a sibling, then a
/// rename, so a crash never leaves half of one), or removed when every slot is empty.
pub fn save(dir: &Path, track: &str, profile: &str, bike: &str, markers: &Markers) -> Result<(), String> {
    let file = path(dir, track, profile);
    if markers.slots.iter().all(Option::is_none) {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", file.display())),
            _ => Ok(()),
        };
    }
    let parent = file.parent().expect("a file in a folder");
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let part = file.with_extension("markers.part");
    std::fs::write(&part, markers.to_file(bike)).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))
}

/// What a seat's slots hold, as one word: a change of it is a slot placed or cleared. The selection
/// is not in it — stepping through the slots writes nothing.
pub fn signature(markers: &Markers) -> u64 {
    markers.slots.iter().fold(0xcbf2_9ce4_8422_2325, |h, slot| {
        let one = slot.as_ref().map_or(0, |ride| hash::of(&**ride));
        (h ^ one).wrapping_mul(0x100_0000_01b3)
    })
}

/// One local seat's file, and what its slots held when last written: [`Kept::frame`] writes the file
/// when they change.
#[derive(Clone, Debug)]
pub struct Kept {
    pub track: String,
    pub profile: String,
    pub bike: String,
    pub last: u64,
}

impl Kept {
    /// A seat that starts its ride on `markers`.
    pub fn new(track: &str, profile: &str, bike: &str, markers: &Markers) -> Kept {
        Kept { track: track.into(), profile: profile.into(), bike: bike.into(), last: signature(markers) }
    }

    /// After a frame: write the seat's file if its slots changed since the last write. What was
    /// written, or why it could not be, for the console; `None` for no change.
    pub fn frame(&mut self, dir: &Path, markers: &Markers) -> Option<String> {
        let now = signature(markers);
        if now == self.last {
            return None;
        }
        self.last = now;
        let file = path(dir, &self.track, &self.profile);
        Some(match save(dir, &self.track, &self.profile, &self.bike, markers) {
            Ok(()) => format!("markers: kept in {}", file.display()),
            Err(e) => format!("markers: not kept: {e}"),
        })
    }
}

/// `markers` as a lobby pick carries them ([`openmx_net::Pick::markers`]): the track they are for,
/// a colon, and the file's bytes in hex; empty for no marker at all.
pub fn to_pick(markers: &Markers, bike: &str, track: &str) -> String {
    if markers.slots.iter().all(Option::is_none) {
        return String::new();
    }
    let hex: String = markers.to_file(bike).iter().map(|b| format!("{b:02x}")).collect();
    format!("{}:{hex}", track.to_ascii_lowercase())
}

/// A pick's markers back, for a seat riding `bike` on `track`: none for a pick with none, none for
/// another track's (a pick the lobby had not caught up with when the ride started), and none for
/// any that do not read — the same on every machine, which all run one build.
pub fn from_pick(text: &str, bike: &str, track: &str) -> Markers {
    let Some((on, hex)) = text.split_once(':') else { return Markers::default() };
    if !on.eq_ignore_ascii_case(track) || hex.len() % 2 != 0 {
        return Markers::default();
    }
    let bytes: Option<Vec<u8>> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect();
    bytes.and_then(|b| Markers::from_file(&b, bike).ok()).unwrap_or_default()
}

/// The lobby pick's markers (P99): what [`to_pick`] makes of the player's saved markers on the
/// room's track, read again only when the track, the bike or the player changes, or after a ride
/// ([`PickMarkers::stale`]).
#[derive(Default)]
pub struct PickMarkers {
    read: Option<(String, String, String)>,
    text: String,
}

impl PickMarkers {
    /// The pick's text for `profile` on `bike` and `track`, out of the settings' folder `dir`.
    pub fn of(&mut self, dir: &Path, track: &str, profile: &str, bike: &str) -> &str {
        let key = (track.to_string(), profile.to_string(), bike.to_string());
        if self.read.as_ref() != Some(&key) {
            self.text = to_pick(&load(dir, track, profile, bike), bike, track);
            self.read = Some(key);
        }
        &self.text
    }

    /// Read the file again next time: a ride may have written it.
    pub fn stale(&mut self) {
        self.read = None;
    }
}

/// The name a bike's markers are kept under: its `.phy`'s, as the lobby's pick names a bike set
/// (`Bike\Race125P.phy` is `race125p`).
pub fn bike_word(member: &str) -> String {
    let file = member.to_ascii_lowercase();
    let leaf = file.rsplit(['\\', '/']).next().unwrap_or(&file);
    leaf.trim_end_matches(".phy").to_string()
}

/// `markers` as a `--peer` ride sends them to every machine (P105, `net::udp`'s pieces): the track
/// they are for — its length, then its name — and the file's bytes; empty for no marker at all.
pub fn to_wire(markers: &Markers, bike: &str, track: &str) -> Vec<u8> {
    if markers.slots.iter().all(Option::is_none) {
        return Vec::new();
    }
    let track = track.to_ascii_lowercase();
    let track = &track.as_bytes()[..track.len().min(255)];
    [&[track.len() as u8][..], track, &markers.to_file(bike)].concat()
}

/// [`to_wire`]'s bytes back, for a seat riding `bike` on `track`: none for none, for another
/// track's or bike's, and for any that do not read — the same on every machine, which all run one
/// build and are handed the same bytes.
pub fn from_wire(bytes: &[u8], bike: &str, track: &str) -> Markers {
    let Some((&n, rest)) = bytes.split_first() else { return Markers::default() };
    let Some((on, file)) = rest.split_at_checked(n as usize) else { return Markers::default() };
    if !on.eq_ignore_ascii_case(track.as_bytes()) {
        return Markers::default();
    }
    Markers::from_file(file, bike).unwrap_or_default()
}
