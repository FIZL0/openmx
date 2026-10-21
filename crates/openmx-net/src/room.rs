//! The lobby as the host keeps it and every peer sees it (opentennis's `net.rs` lobby data, `Room`, `Seat`,
//! `Pick`, `Listing`, `quick_pick`), cut to what a ride needs: the track and its two switches, up to [`SEATS`]
//! players with their riders and bikes, who is ready, and the seed once the host starts.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The most players in one ride (S54's players, P35's seats).
pub const SEATS: usize = 8;

/// The host's own peer name in the seats (a guest's is its endpoint ID).
pub const HOST: &str = "host";

/// One local player's pick in the garage: the rider (model and gear, `--rider`'s names, like thq1) and the bike set
/// (the `.phy` the simulation loads, like race125p: what the handshake compares).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pick {
    pub rider: String,
    pub bike: String,
}

impl Pick {
    pub fn new(rider: &str, bike: &str) -> Pick {
        Pick { rider: rider.into(), bike: bike.into() }
    }
}

/// A taken seat: whose peer and their pick.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub peer: String,
    pub pick: Pick,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Room {
    /// The lobby's name in Find (the host's player name).
    pub name: String,
    /// The ride: the track and its switches, the host's to change.
    pub track: String,
    pub wrecks: bool,
    pub biplane: bool,
    /// Seats in order (1P..), `None` open; as many as the host opened (2..=[`SEATS`]).
    pub seats: Vec<Option<Seat>>,
    /// Each peer's player name (its Hello's), and the guests who said they're ready (the host always is).
    #[serde(default)]
    pub names: BTreeMap<String, String>,
    #[serde(default)]
    pub ready: BTreeSet<String>,
    /// Each guest's last round trip to the host, in ms.
    #[serde(default)]
    pub ping: BTreeMap<String, u32>,
    /// Set by the host at its Start: the ride's seed.
    #[serde(default)]
    pub seed: u32,
    /// Counts the line-up's changes: a ready is for the line-up it was given on.
    #[serde(default)]
    pub ver: u32,
    /// Not listed (no DHT board entry, no LAN listing): only the code joins it.
    #[serde(default)]
    pub private: bool,
    /// The host's room code once it is out, so a guest's lobby shows the same one to pass on.
    #[serde(default)]
    pub short: String,
    /// The ride is under way (between the host's go and the way back to the lobby).
    #[serde(default)]
    pub riding: bool,
    /// The host's game modifiers (P55), in `game_sim::modifiers::Mods`' text form: every peer loads
    /// the ride with them, so the lockstep rides one set of numbers. Empty is the original game.
    #[serde(default)]
    pub modifiers: String,
    /// The ride takes peers on while under way (N10, a freeride opened online from its pause menu):
    /// listed as joinable while riding, and a peer that joins is seated, handshaken and sent into
    /// the ride as it is, rather than turned away until the lobby is back.
    #[serde(default)]
    pub drop_in: bool,
}

impl Room {
    /// A lobby for `size` players on `track`, wrecks on and the biplane off, as the shell starts.
    pub fn new(name: &str, track: &str, size: usize) -> Room {
        Room { name: name.into(), track: track.into(), wrecks: true, seats: vec![None; size.clamp(2, SEATS)], ..Default::default() }
    }

    pub fn open(&self) -> usize {
        self.seats.iter().filter(|s| s.is_none()).count()
    }

    pub fn taken(&self) -> usize {
        self.seats.len() - self.open()
    }

    /// Seats `peer`'s players in the first open seats, all or none (false: not enough room).
    pub fn take(&mut self, peer: &str, picks: &[Pick]) -> bool {
        if picks.is_empty() || picks.len() > self.open() {
            return false;
        }
        let mut picks = picks.iter();
        for s in self.seats.iter_mut().filter(|s| s.is_none()) {
            let Some(p) = picks.next() else { break };
            *s = Some(Seat { peer: peer.into(), pick: p.clone() });
        }
        self.changed();
        true
    }

    /// `peer` gone: its seats open, its name and ready out.
    pub fn free(&mut self, peer: &str) {
        for s in &mut self.seats {
            if s.as_ref().is_some_and(|s| s.peer == peer) {
                *s = None;
            }
        }
        self.names.remove(peer);
        self.ping.remove(peer);
        self.ready.remove(peer);
        self.changed();
    }

    /// The seats `peer` holds, in order.
    pub fn seats_of(&self, peer: &str) -> Vec<usize> {
        self.seats.iter().enumerate().filter(|(_, s)| s.as_ref().is_some_and(|s| s.peer == peer)).map(|(i, _)| i).collect()
    }

    /// `peer`'s new picks for the seats it holds, in order (extra picks are dropped, missing ones keep the old).
    pub fn repick(&mut self, peer: &str, picks: &[Pick]) {
        let held = self.seats_of(peer);
        for (i, p) in held.into_iter().zip(picks) {
            if let Some(s) = &mut self.seats[i] {
                s.pick = p.clone();
            }
        }
        self.changed();
    }

    /// The peers with a seat (the host first when it has one).
    pub fn peers(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for s in self.seats.iter().flatten() {
            if !v.contains(&s.peer) {
                v.push(s.peer.clone());
            }
        }
        v
    }

    /// Every seated guest has said it's ready.
    pub fn all_ready(&self) -> bool {
        self.peers().iter().filter(|p| *p != HOST).all(|p| self.ready.contains(p))
    }

    /// The line-up or the ride changed: the readies were for the old one.
    pub fn changed(&mut self) {
        self.ver += 1;
        self.ready.clear();
    }

    /// The ride over (a peer dropped, or the host ended it): everyone still here back in the lobby, to ready again.
    pub fn back_to_lobby(&mut self) {
        self.riding = false;
        self.seed = 0;
        self.changed();
    }
}

/// A lobby as Find lists it: on the DHT board, and on the LAN in its mDNS record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Listing {
    /// The host's endpoint ID (what a joiner dials, its address from the DHT or mDNS).
    pub id: String,
    pub name: String,
    pub track: String,
    pub taken: usize,
    pub seats: usize,
    /// Unix seconds when the host wrote it (on the LAN: when it was heard).
    pub time: u64,
    /// Its room code, empty until the host has one out.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
    /// The host's build: Find greys out the others, and Quickplay skips them.
    #[serde(default)]
    pub build: String,
    /// The ride is under way and takes peers on (a [`Room::drop_in`] one): Find says so. Not written
    /// when false, so a lobby's mDNS record (245 bytes at most) is no longer than it was.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub riding: bool,
}

impl Listing {
    pub fn of(id: &str, room: &Room) -> Listing {
        let closed = room.private || room.riding && !room.drop_in;
        let taken = if closed { room.seats.len() } else { room.taken() };
        let riding = room.riding && !closed;
        Listing { id: id.into(), name: room.name.clone(), track: room.track.clone(), taken, seats: room.seats.len(), time: now(), code: room.short.clone(), build: crate::handshake::build().into(), riding }
    }

    pub fn open(&self) -> usize {
        self.seats.saturating_sub(self.taken)
    }

    pub fn same_build(&self) -> bool {
        self.build == crate::handshake::build()
    }
}

/// The Quickplay rule (opentennis's, from the Halo CE port's Quick Match): the busiest open lobby of this build with
/// room for `need` players, the newest first on a tie.
pub fn quick_pick(list: &[Listing], need: usize) -> Vec<Listing> {
    let mut fit: Vec<_> = list.iter().filter(|l| l.open() >= need && l.same_build()).cloned().collect();
    fit.sort_by_key(|l| (std::cmp::Reverse(l.taken), std::cmp::Reverse(l.time)));
    fit
}

pub(crate) fn now() -> u64 {
    n0_future::time::SystemTime::now().duration_since(n0_future::time::SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seats_fill_in_order_and_free() {
        let mut r = Room::new("h", "free06", 4);
        assert!(r.take(HOST, &[Pick::new("thq1", "race125p")]));
        assert!(r.take("a", &[Pick::new("thq2", "pro250a"), Pick::new("thq3", "race500p")]));
        assert!(!r.take("b", &[Pick::new("x", "y"), Pick::new("x", "y")]), "two players, one seat: none seated");
        assert_eq!(r.seats_of("a"), [1, 2]);
        assert_eq!(r.peers(), [HOST, "a"]);
        r.ready.insert("a".into());
        assert!(r.all_ready());
        r.repick("a", &[Pick::new("thq2", "race50p")]);
        assert_eq!(r.seats[1].as_ref().unwrap().pick.bike, "race50p");
        assert_eq!(r.seats[2].as_ref().unwrap().pick.bike, "race500p");
        assert!(!r.all_ready(), "a new pick asks for ready again");
        r.free("a");
        assert_eq!((r.open(), r.taken()), (3, 1));
        assert!(r.take("b", &[Pick::new("x", "y")]));
        assert_eq!(r.seats_of("b"), [1]);
    }

    #[test]
    fn quickplay_takes_the_busiest_that_fits() {
        let l = |id: &str, taken, seats, time| Listing { id: id.into(), name: String::new(), track: "free06".into(), taken, seats, time, code: String::new(), build: crate::handshake::build().into(), riding: false };
        let other = Listing { build: "0.0.0 other".into(), ..l("x", 3, 4, 9) };
        let list = [l("a", 1, 4, 5), l("b", 2, 4, 3), l("c", 2, 4, 4), l("d", 3, 4, 9), other];
        let ids: Vec<String> = quick_pick(&list, 2).into_iter().map(|l| l.id).collect();
        assert_eq!(ids, ["c", "b", "a"]);
    }

    #[test]
    fn a_private_or_riding_lobby_lists_as_full() {
        let mut r = Room::new("h", "free06", 4);
        r.take(HOST, &[Pick::new("thq1", "race125p")]);
        assert_eq!(Listing::of("id", &r).open(), 3);
        r.private = true;
        assert_eq!(Listing::of("id", &r).open(), 0);
        r.private = false;
        r.riding = true;
        assert_eq!(Listing::of("id", &r).open(), 0);
        assert!(!Listing::of("id", &r).riding, "a ride that takes nobody on lists as a full lobby");
    }

    #[test]
    fn a_ride_open_to_drop_ins_lists_as_joinable() {
        let mut r = Room::new("h", "free06", 4);
        r.take(HOST, &[Pick::new("thq1", "race125p")]);
        (r.riding, r.drop_in) = (true, true);
        let l = Listing::of("id", &r);
        assert_eq!((l.open(), l.riding), (3, true));
        assert_eq!(quick_pick(&[l.clone()], 1).len(), 1, "Quickplay can drop in on it");
        let json = serde_json::to_string(&l).unwrap();
        assert!(json.contains("\"riding\":true"), "{json}");
        let back: Listing = serde_json::from_str(&json.replace(",\"riding\":true", "")).unwrap();
        assert!(!back.riding, "an older build's listing reads as a lobby");
        assert!(!serde_json::to_string(&Listing { riding: false, ..l }).unwrap().contains("riding"));
        r.private = true;
        assert_eq!(Listing::of("id", &r).open(), 0, "private is private, riding or not");
    }
}
