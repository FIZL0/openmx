//! **Session markers kept between sessions** (P99): each player's five slots ([`markers::Markers`])
//! saved per track beside `settings.txt`, in a `markers/` folder — `markers/<track>/<profile>.markers`,
//! the profile the rider's name or, with none, the seat (`1P`, `2P`, ..). A file is written when a
//! slot is placed or cleared and read into the seat's markers when a ride starts on its track.
//!
//! The file is [`markers::Markers::to_file`]'s: a version and the bike's `.phy` name before the
//! slots, so a file of another format or another bike is dropped — said on the console and left
//! alone until a slot overwrites it — rather than read as something it is not.
//!
//! **Each machine its own file** (P107): the plane's markers beside the bike's, in
//! `markers/<track>/<profile>.biplane.markers`, named by the plane's `.phy` ([`markers::PLANE`])
//! where a bike's name the bike. A ride loads every machine's, and a slot placed or cleared writes
//! only its machine's file. A tape and a pick carry them all in one ([`Markers::to_files`]): the
//! bike's file alone, as before, while the plane has none.
//!
//! The markers are simulation state, so the files only ever start a ride: a tape carries the ones its
//! ride started with ([`game_sim::replay::PadTape::markers`]) and replays without the files, and
//! online each player's go to every machine with their seat's pick ([`openmx_net::Pick::markers`]).

use std::path::{Path, PathBuf};

use game_sim::hash;
use game_sim::markers::{self, Markers, Vehicle};

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

/// Where `profile`'s bike markers on `track` are kept, under the settings' folder `dir`.
pub fn path(dir: &Path, track: &str, profile: &str) -> PathBuf {
    path_of(dir, track, profile, Vehicle::Bike)
}

/// Where `profile`'s markers on `track` for the machine `on` are kept (P107): the bike's
/// [`path`], the plane's beside it, its name before the extension.
pub fn path_of(dir: &Path, track: &str, profile: &str, on: Vehicle) -> PathBuf {
    let leaf = match on {
        Vehicle::Bike => format!("{}.markers", file_word(profile)),
        Vehicle::Plane => format!("{}.{}.markers", file_word(profile), markers::PLANE),
    };
    dir.join(DIR).join(file_word(&track.to_ascii_lowercase())).join(leaf)
}

/// Every machine's name for its markers: the bike's, `bike`, and the plane's ([`markers::PLANE`]).
pub fn names(bike: &str) -> [(Vehicle, &str); 2] {
    [(Vehicle::Bike, bike), (Vehicle::Plane, markers::PLANE)]
}

/// The markers `profile` kept on `track`, every machine's — the bike's for `bike`: none without a
/// file, and none — said why — for a file that is not this build's format or is another machine's.
pub fn load(dir: &Path, track: &str, profile: &str, bike: &str) -> Markers {
    let mut out = Markers::default();
    for (on, name) in names(bike) {
        let file = path_of(dir, track, profile, on);
        let Ok(bytes) = std::fs::read(&file) else { continue };
        match Markers::from_file(&bytes, name) {
            Ok(m) if m.any_on(on) || !m.any() => out.join(m),
            Ok(_) => println!("markers: {} not loaded: another machine's", file.display()),
            Err(e) => println!("markers: {} not loaded: {e}", file.display()),
        }
    }
    out
}

/// Keep `markers`' slots as `profile`'s on `track`, every machine's file ([`save_of`]).
pub fn save(dir: &Path, track: &str, profile: &str, bike: &str, markers: &Markers) -> Result<(), String> {
    names(bike).into_iter().try_for_each(|(on, name)| save_of(dir, track, profile, on, name, markers))
}

/// Keep `markers`' slots of the machine `on` (named `name`) as `profile`'s on `track`: the file
/// written whole (a sibling, then a rename, so a crash never leaves half of one), or removed when
/// every slot is empty.
pub fn save_of(dir: &Path, track: &str, profile: &str, on: Vehicle, name: &str, markers: &Markers) -> Result<(), String> {
    let file = path_of(dir, track, profile, on);
    if !markers.any_on(on) {
        return match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("{}: {e}", file.display())),
            _ => Ok(()),
        };
    }
    let parent = file.parent().expect("a file in a folder");
    std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    let part = file.with_extension("markers.part");
    std::fs::write(&part, markers.set_file(on, name)).map_err(|e| format!("{}: {e}", part.display()))?;
    std::fs::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))
}

/// What a seat's slots hold, as one word: a change of it is a slot placed or cleared, on any
/// machine. The selection is not in it — stepping through the slots writes nothing. With markers on
/// the bike alone it is P99's word, the bike's ([`signatures`]).
pub fn signature(markers: &Markers) -> u64 {
    let words = signatures(markers);
    Vehicle::ALL.iter().zip(words).skip(1).filter(|(on, _)| markers.any_on(**on)).fold(words[0], |h, (_, w)| (h ^ w).wrapping_mul(PRIME))
}

/// What each machine's slots hold, a word each in [`Vehicle::ALL`] order (P107): a change of one is
/// a slot of that machine placed or cleared.
pub fn signatures(markers: &Markers) -> [u64; Vehicle::ALL.len()] {
    Vehicle::ALL.map(|on| {
        let empty = Default::default();
        markers.slots_of(on).unwrap_or(&empty).iter().fold(0xcbf2_9ce4_8422_2325, |h, slot| {
            let one = slot.as_ref().map_or(0, |mark| hash::of(&**mark));
            (h ^ one).wrapping_mul(PRIME)
        })
    })
}

/// The signatures' multiplier (FNV-1a's).
const PRIME: u64 = 0x100_0000_01b3;

/// One local seat's file, and what its slots held when last written: [`Kept::frame`] writes the file
/// when they change.
#[derive(Clone, Debug)]
pub struct Kept {
    pub track: String,
    pub profile: String,
    pub bike: String,
    /// [`signature`] at the last write.
    pub last: u64,
    /// [`signatures`] at the last write: which machine's file a change is to.
    each: [u64; Vehicle::ALL.len()],
}

impl Kept {
    /// A seat that starts its ride on `markers`.
    pub fn new(track: &str, profile: &str, bike: &str, markers: &Markers) -> Kept {
        Kept { track: track.into(), profile: profile.into(), bike: bike.into(), last: signature(markers), each: signatures(markers) }
    }

    /// After a frame: write the seat's file of each machine whose slots changed since the last
    /// write (P107: only that machine's). What was written, or why it could not be, for the
    /// console; `None` for no change.
    pub fn frame(&mut self, dir: &Path, markers: &Markers) -> Option<String> {
        let now = signature(markers);
        if now == self.last {
            return None;
        }
        let each = signatures(markers);
        let mut said = Vec::new();
        for (i, (on, name)) in names(&self.bike).into_iter().enumerate() {
            if each[i] == self.each[i] {
                continue;
            }
            let file = path_of(dir, &self.track, &self.profile, on);
            said.push(match save_of(dir, &self.track, &self.profile, on, name, markers) {
                Ok(()) => format!("markers: kept in {}", file.display()),
                Err(e) => format!("markers: not kept: {e}"),
            });
        }
        (self.last, self.each) = (now, each);
        (!said.is_empty()).then(|| said.join("\n"))
    }
}

/// `markers` as a lobby pick carries them ([`openmx_net::Pick::markers`]): the track they are for,
/// a colon, and the file's bytes in hex; empty for no marker at all.
pub fn to_pick(markers: &Markers, bike: &str, track: &str) -> String {
    if !markers.any() {
        return String::new();
    }
    let hex: String = markers.to_files(&names(bike)).iter().map(|b| format!("{b:02x}")).collect();
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
    bytes.and_then(|b| Markers::from_files(&b, &names(bike)).ok()).unwrap_or_default()
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
    if !markers.any() {
        return Vec::new();
    }
    let track = track.to_ascii_lowercase();
    let track = &track.as_bytes()[..track.len().min(255)];
    [&[track.len() as u8][..], track, &markers.to_files(&names(bike))].concat()
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
    Markers::from_files(file, &names(bike)).unwrap_or_default()
}
