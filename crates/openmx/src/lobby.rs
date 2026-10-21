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

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use game_render::disc::Disc;
use openmx_net::handshake::fnv;
use openmx_net::{Board, Net, Online, Relay};

use crate::menu::{Act, Menu};
use crate::roster;

/// What this machine's disc holds, read when first asked and kept.
#[derive(Default)]
pub struct DiscFiles {
    bikes: Mutex<HashMap<String, Option<Arc<Vec<u8>>>>>,
    tracks: Mutex<HashMap<String, Option<u64>>>,
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

fn read_phy(set: &str) -> Option<Vec<u8>> {
    let set = roster::Set::from_word(set)?;
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

impl openmx_net::Disc for DiscFiles {
    fn track(&self, name: &str) -> Option<u64> {
        self.track_hash(name)
    }

    fn bike(&self, set: &str) -> Option<u64> {
        self.phy(set).map(|b| fnv(&b))
    }
}

/// The bike sets the lobby offers, as the menu shows them.
pub fn bikes() -> Vec<(String, String)> {
    roster::Set::all().into_iter().map(|s| (s.word(), s.label())).collect()
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
        Online::parse(&std::fs::read_to_string(&self.file).unwrap_or_default())
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
            if let Err(e) = std::fs::write(&self.file, o.text()) {
                menu.note = Some(format!("Could not save {}: {e}", self.file.display()));
            }
            // Started again with the new network the next time it is asked for.
            self.net = None;
        }
        let out = std::mem::take(&mut menu.lobby.out);
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
                if menu.heard(e) == Act::Online {
                    act = Act::Online;
                }
            }
        }
        act
    }
}
