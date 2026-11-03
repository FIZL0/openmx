//! The shell's side of the lobby (N3): what this machine's disc holds under a track's and a bike
//! set's name, for the handshake to compare, and the network the front end's online screens drive.
//!
//! The menu ([`crate::menu`]) only queues `openmx_net` commands and reads its events; [`Network`]
//! starts the network thread the first time it is asked for, hands it the commands, and passes what
//! it says back to the menu, once a frame. An online ride from the lobby goes over the same
//! connection ([`crate::online::Online::lobby`]).
//!
//! **The hashes** ([`DiscFiles`]) are 64-bit FNV-1a (`openmx_net::handshake::fnv`) over the files
//! the ride loads: a track is its two archives (`<track>.res` and `<track>_2.res`, the props and
//! terrain and the bike's set, camera and animation that load alongside), a bike set its `.phy`
//! (`Bike\Race125P.phy` and the other fifteen, D7). Which archive the `.phy` comes from is the same
//! rule on every disc — the first bike archive in the disc's own order that carries the set's file —
//! and an online ride rides every seat on exactly those bytes, so what was compared is what is ridden.
//!
//! A track mod (MD6, [`mod_track_hash`]) is every file in its folder, by path, and then the disc's
//! freeride it rides with (its second archive: the bike's animation set, physics and follow camera,
//! `track_mods::FREERIDE`), so a room on one seats only peers whose mods folder holds the same
//! files, and the refusal says the mod's name and version ([`openmx_net::Disc::track_mod`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use game_render::disc::Disc;
use openmx_net::handshake::{fnv, fnv_on};
use openmx_net::{Board, Net, Online, Relay};

use crate::menu::{Act, Menu};
use crate::roster;

/// What this machine's disc holds, read when first asked and kept.
#[derive(Default)]
pub struct DiscFiles {
    bikes: Mutex<HashMap<String, Option<Arc<Vec<u8>>>>>,
    tracks: Mutex<HashMap<String, Option<u64>>>,
    /// Whose saved markers a ride this machine drops in on brings (P105): the settings' folder and
    /// 1P's profile (`saved_markers::profile`), as the front end last said them.
    markers: Mutex<Option<(PathBuf, String)>>,
    /// The mods folder this machine's trick packs (MD19) and track mods (MD6) are read from, `None`
    /// while the Mods row is off, as the front end last said it.
    mods: Mutex<Option<PathBuf>>,
}

/// The disc's member ending in `suffix`, the shortest (the shell's own rule: the menus keep deeper
/// copies of some archives).
fn ending(disc: &Disc, suffix: &str) -> Option<String> {
    let want = suffix.to_ascii_lowercase();
    disc.entries()
        .iter()
        .filter(|e| e.name.to_ascii_lowercase().ends_with(&want))
        .min_by_key(|e| e.name.len())
        .map(|e| e.name.clone())
}

impl DiscFiles {
    /// The bike set's `.phy` (`race125p`): the first `Bike\<model>.res` on the disc carrying it.
    pub fn phy(&self, set: &str) -> Option<Arc<Vec<u8>>> {
        if let Some(got) = self.bikes.lock().unwrap().get(set) {
            return got.clone();
        }
        let got = read_phy(set).map(Arc::new);
        self.bikes.lock().unwrap().insert(set.to_string(), got.clone());
        got
    }

    /// The track's two archives, hashed.
    pub fn track_hash(&self, track: &str) -> Option<u64> {
        if let Some(got) = self.tracks.lock().unwrap().get(track) {
            return *got;
        }
        let got = read_track(track);
        self.tracks.lock().unwrap().insert(track.to_string(), got);
        got
    }
}

/// Every seat online is a person's, so a set word reads as the P files whatever its tuning says
/// ([`roster::Set::ridden`]): what is hashed for the handshake is what the ride rides.
fn read_phy(set: &str) -> Option<Vec<u8>> {
    let set = roster::Set::from_player_word(set)?;
    let mut disc = Disc::open_configured().ok()?;
    let bikes: Vec<String> = disc
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| {
            let lower = n.to_ascii_lowercase();
            lower.strip_prefix("bike\\").is_some_and(|leaf| leaf.ends_with(".res") && !leaf.contains('\\'))
        })
        .collect();
    let file = set.file();
    bikes.iter().find_map(|member| {
        let res = disc.res(member).ok()?;
        res.find_suffix(&file)?;
        res.read_suffix(&file).ok()
    })
}

fn read_track(track: &str) -> Option<u64> {
    let mut disc = Disc::open_configured().ok()?;
    let first = ending(&disc, &format!("{track}.res"))?;
    let mut bytes = disc.member(&first).ok()?;
    if let Some(second) = ending(&disc, &format!("{track}_2.res")) {
        bytes.extend(disc.member(&second).ok()?);
    }
    Some(fnv(&bytes))
}

/// A track mod's files (MD6), hashed: every file under its folder in path order, each as its path
/// (from the folder, `/`-separated, lower case, as the folder archive looks names up) and a zero,
/// then its bytes.
pub fn mod_track_hash(dir: &Path) -> Result<u64, String> {
    fn walk(dir: &Path, under: &str, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
        for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let path = e.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            let name = if under.is_empty() { name } else { format!("{under}/{name}") };
            if path.is_dir() {
                walk(&path, &name, out)?;
            } else {
                out.push((name, path));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, "", &mut files)?;
    files.sort();
    let mut h = fnv(b"");
    for (name, path) in files {
        h = fnv_on(fnv_on(h, name.as_bytes()), &[0]);
        h = fnv_on(h, &std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?);
    }
    Ok(h)
}

impl DiscFiles {
    /// The track mod `name` in this machine's mods folder (MD6), as a ride on it finds it: `None`
    /// with the Mods row off, or with no such track mod.
    fn mod_track(&self, name: &str) -> Option<crate::track_mods::ModTrack> {
        let mods = self.mods.lock().unwrap().clone()?;
        crate::track_mods::find(&mods, name).ok()
    }
}

impl openmx_net::Disc for DiscFiles {
    /// A disc track's archives, else a track mod's files and the disc freeride it rides with, as a
    /// ride looks for them: the disc first.
    fn track(&self, name: &str) -> Option<u64> {
        if let Some(h) = self.track_hash(name) {
            return Some(h);
        }
        let h = mod_track_hash(&self.mod_track(name)?.dir).ok()?;
        Some(match self.track_hash(crate::track_mods::FREERIDE) {
            Some(disc) => fnv_on(h, &disc.to_le_bytes()),
            None => h,
        })
    }

    /// The mod's name and version (`MC01 Canyon 1.0.0`), as its `mod.json` says them.
    fn track_mod(&self, name: &str) -> Option<String> {
        if self.track_hash(name).is_some() {
            return None;
        }
        let t = self.mod_track(name)?;
        let text = std::fs::read_to_string(t.dir.join("mod.json")).ok()?;
        let version = crate::track_mods::Json::parse(&text).ok()?.get("version")?.str()?.to_string();
        Some(format!("{} {version}", t.name))
    }

    fn bike(&self, set: &str) -> Option<u64> {
        self.phy(set).map(|b| fnv(&b))
    }

    fn markers(&self, track: &str, bike: &str) -> String {
        let Some((dir, profile)) = self.markers.lock().unwrap().clone() else { return String::new() };
        crate::saved_markers::to_pick(&crate::saved_markers::load(&dir, track, &profile, bike), bike, track)
    }

    /// A room's trick packs' hash (MD19), or a list of them stacked (MD77, `a,b`:
    /// [`crate::trick_packs::hash`]); `None` when this machine lacks one of them.
    fn tricks(&self, id: &str) -> Option<u64> {
        let mods = self.mods.lock().unwrap().clone()?;
        let packs = crate::trick_packs::on(&mods, &crate::trick_packs::ids_of_room(id)).ok()?;
        crate::trick_packs::hash(&packs).ok()
    }
}

impl DiscFiles {
    /// The mods folder this machine's trick packs (MD19) and track mods (MD6) are hashed from, `None`
    /// while the Mods row is off. The front end's, every frame.
    pub fn tricks_from(&self, mods: Option<PathBuf>) {
        *self.mods.lock().unwrap() = mods;
    }

    /// The player whose saved markers a ride this machine drops in on brings (P105): `profile`'s in
    /// the settings' folder `dir`. The front end's, every frame.
    pub fn bring_markers(&self, dir: &Path, profile: &str) {
        let mut m = self.markers.lock().unwrap();
        if m.as_ref().is_none_or(|(d, p)| d != dir || p != profile) {
            *m = Some((dir.to_path_buf(), profile.to_string()));
        }
    }
}

/// The trick packs a ride on `tricks` stacks (MD19, MD77), as a room names them: the ids of the
/// packs `named` that `mods` has, or with none named its first (the ones `ride_trick_packs` would
/// lay), joined by commas in the folder's order, for MX vs. ATV's; else empty, the original's.
pub fn room_tricks(tricks: crate::menu::Tricks, named: &[String], mods: Option<&Path>) -> String {
    match (tricks, mods) {
        (crate::menu::Tricks::MxVsAtv, Some(mods)) => {
            let installed: Vec<String> = crate::trick_packs::listed(mods).0.into_iter().map(|p| p.id).collect();
            crate::trick_packs::room_ids(&crate::trick_packs::chosen(&installed, named))
        }
        _ => String::new(),
    }
}

/// A lobby this machine hosts rides its Tricks rows (MD19, MD77): the trick packs on, in the room
/// the commands `out` host, the packs a ride of its own would stack over the original's, and the
/// handshake holds every seat to them. A ride opened online under way (N10) already says its own.
pub fn name_tricks(out: &mut [openmx_net::Cmd], settings: &crate::menu::Settings, mods: Option<&Path>) {
    for c in out {
        if let openmx_net::Cmd::Host(room) | openmx_net::Cmd::Quick(room) = c
            && !room.riding
        {
            room.tricks = room_tricks(settings.tricks, &settings.trick_packs, mods);
        }
    }
}

/// The handling a ride on `handling` rides (N21, MD40's setting), as a room names it: the id of the
/// pack whose `mx_*.phy` the bikes are read from for MX vs. ATV's — `named` when `mods` has it
/// (MD77), else the first of the room's trick packs `tricks`, else the folder's first, the one
/// `ride_handling_pack` would read — else empty, the original's.
pub fn room_handling(handling: game_sim::handling::Handling, named: &str, tricks: &str, mods: Option<&Path>) -> String {
    match (handling, mods) {
        (game_sim::handling::Handling::MxVsAtv | game_sim::handling::Handling::Untamed, Some(mods)) => {
            let installed: Vec<String> = crate::trick_packs::listed(mods).0.into_iter().map(|p| p.id).collect();
            let first_trick = crate::trick_packs::ids_of_room(tricks).into_iter().next();
            Some(named.to_string()).filter(|n| installed.contains(n)).or(first_trick).or_else(|| installed.first().cloned()).unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// A lobby this machine hosts rides its Handling rows (N21, MD77): MX vs. ATV's names its pack in
/// the room the commands `out` host, and the handshake holds every seat to it. A ride opened online
/// under way (N10) already says its own. After [`name_tricks`], whose packs it falls back on.
pub fn name_handling(out: &mut [openmx_net::Cmd], settings: &crate::menu::Settings, mods: Option<&Path>) {
    for c in out {
        if let openmx_net::Cmd::Host(room) | openmx_net::Cmd::Quick(room) = c
            && !room.riding
        {
            room.handling = room_handling(settings.handling, &settings.handling_pack, &room.tricks, mods);
        }
    }
}

/// The bike sets the lobby offers, as the menu shows them: the eight a player rides, P every one
/// ([`roster::Set::ridden`]).
pub fn bikes() -> Vec<(String, String)> {
    roster::Set::players().into_iter().map(|s| (s.word(), s.label())).collect()
}

/// The network under the front end: none until the online screens first ask for something.
pub struct Network {
    net: Option<Arc<Net>>,
    pub disc: Arc<DiscFiles>,
    /// `online.txt`, beside the settings.
    file: PathBuf,
}

impl Network {
    pub fn new(dir: &Path) -> Network {
        Network { net: None, disc: Arc::new(DiscFiles::default()), file: dir.join("online.txt") }
    }

    /// `online.txt` as it stands (the defaults when there is none).
    pub fn online(&self) -> Online {
        Online::parse(&crate::files::read_to_string(&self.file).unwrap_or_default())
    }

    /// The configuration the network starts with: `online.txt`'s, under the player's name, and with
    /// nothing but the LAN when the menu says so.
    fn config(&self, name: &str, lan: bool) -> Online {
        let mut o = self.online();
        o.name = name.to_string();
        if lan {
            o.relay = Relay::Off;
            o.board = Board::Off;
            o.code_servers = Some(Vec::new());
        }
        o
    }

    /// The connection, for a ride from the lobby.
    pub fn net(&self) -> Option<Arc<Net>> {
        self.net.clone()
    }

    /// Open the ride under way to online players (N10, the pause's "Open to Online"): host `room`,
    /// a riding one that takes peers on, starting the network if it is not up. The connection, for
    /// the ride to go over.
    pub fn open(&mut self, menu: &mut Menu, room: openmx_net::Room) -> Arc<Net> {
        menu.lobby.out.push(openmx_net::Cmd::Host(room));
        self.pump(menu);
        self.net.clone().expect("pump starts it for the command")
    }

    /// A lobby's ride that could not start here: the host takes everyone back to the lobby, a guest
    /// leaves it, so nobody waits on a machine that is not riding.
    pub fn abandon(&mut self, menu: &mut Menu) {
        let Some(net) = &self.net else { return };
        if menu.lobby.hosting() {
            net.send(openmx_net::Cmd::Lobby);
        } else {
            net.send(openmx_net::Cmd::Leave);
            menu.lobby.room = None;
        }
    }

    /// One frame: the menu's commands out, the network's events in. `Act::Online` when the lobby's
    /// ride starts.
    pub fn pump(&mut self, menu: &mut Menu) -> Act {
        if std::mem::take(&mut menu.lobby.network_changed) {
            let mut o = self.online();
            o.relay = match (menu.lobby.lan, o.relay) {
                (true, _) => Relay::Off,
                (false, Relay::Off) => Relay::N0,
                (false, r) => r,
            };
            if let Err(e) = crate::files::write(&self.file, o.text()) {
                menu.note = Some(format!("Could not save {}: {e}", self.file.display()));
            }
            // Started again with the new network the next time it is asked for.
            self.net = None;
        }
        // The mods folder a trick pack (MD19) or a track mod (MD6) is read from, as the Mods row says now.
        let dir = self.file.parent().unwrap_or(Path::new("."));
        let mods = menu.settings.mods.then(|| dir.join(crate::track_mods::MODS_DIR));
        self.disc.tricks_from(mods.clone());
        let mut out = std::mem::take(&mut menu.lobby.out);
        name_tricks(&mut out, &menu.settings, mods.as_deref());
        name_handling(&mut out, &menu.settings, mods.as_deref());
        if !out.is_empty() {
            let name = menu.settings.name.clone();
            let lan = menu.lobby.lan;
            if self.net.is_none() {
                println!("online: starting the network ({})", if lan { "LAN only" } else { "internet and LAN" });
                self.net = Some(Arc::new(Net::start(self.config(&name, lan), self.disc.clone())));
            }
            let net = self.net.as_ref().expect("just started");
            for c in out {
                if matches!(c, openmx_net::Cmd::Host(_) | openmx_net::Cmd::Join(..) | openmx_net::Cmd::Quick(_)) {
                    net.send(openmx_net::Cmd::Name(name.clone()));
                }
                net.send(c);
            }
        }
        let mut act = Act::Stay;
        if let Some(net) = &self.net {
            for e in net.poll() {
                #[cfg(target_arch = "wasm32")]
                if let Some(line) = page_line(&e) {
                    crate::host_say(&line);
                }
                if menu.heard(e) == Act::Online {
                    act = Act::Online;
                }
            }
        }
        act
    }
}

/// What the front end's lobby heard, as a line for the page's console (P204): the browser has no
/// standard output, and a test driving the game's own Online menu in headless chromium reads the
/// room's code, its seats and the start from these. `None` for what says nothing of the lobby.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub fn page_line(e: &openmx_net::Event) -> Option<String> {
    use openmx_net::Event;
    Some(match e {
        Event::Status(s) => format!("lobby: status {s}"),
        Event::Short(code) => format!("lobby: code {code}"),
        Event::Me(me) => format!("lobby: me {me}"),
        Event::Room(r) => format!("lobby: room {} {} seated, ready {}", r.short, r.taken(), r.all_ready()),
        Event::Start(r, me) => format!("lobby: start {} on {} as {me}", r.taken(), r.track),
        Event::Failed(why) => format!("lobby: failed {why}"),
        Event::Dropped(why) => format!("lobby: dropped {why}"),
        _ => return None,
    })
}
