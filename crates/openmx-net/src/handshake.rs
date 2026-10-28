//! What every peer must agree on before a ride starts (opentennis's `hst-base/src/handshake.rs`, M2e): the host
//! picks the seed, and each peer's handshake has to match the host's on everything that changes the simulation,
//! else that peer is refused with a message saying what differs.
//!
//! Here that is the track, every player's bike set (the `.phy` the simulation loads for them, D7's loader), wrecks
//! on or off (`game_sim::state::Sense::wrecks_enabled` in the shell), Air Wrecks on or off (`Sense::air_wrecks`, P138), Backwards Landings Bail (`Sense::backwards_bail`, S161), the biplane switch and every player's controller
//! preset (N13: the binding table their seat's bike is read through on every machine). A name is not enough
//! for a bike set or a track: two discs (or a modded one) can hold different files under one name, so each peer
//! also says what its own disc has under it, as a hash ([`Disc`]). The riders' looks (model, gear, colours) are not
//! in it: they are drawn, not simulated.

use serde::{Deserialize, Serialize};

use crate::room::Room;

/// What a peer's disc holds under a track's or a bike set's name, as a hash of the files the ride would load
/// (`None`: it has no such file, or this peer doesn't hash, as in the terminal lobby with no disc).
pub trait Disc: Send + Sync {
    fn track(&self, _name: &str) -> Option<u64> {
        None
    }
    fn bike(&self, _set: &str) -> Option<u64> {
        None
    }
    /// The trick pack `id` (MD19) as this machine's mods folder holds it, hashed: `None` when it has
    /// no such pack.
    fn tricks(&self, _id: &str) -> Option<u64> {
        None
    }
    /// This machine's player's saved markers on `track` for the bike set `bike`, as a pick carries
    /// them ([`crate::room::Pick::markers`]), for a ride it drops in on (P105): empty for none, and
    /// where nothing is saved (the terminal lobby).
    fn markers(&self, _track: &str, _bike: &str) -> String {
        String::new()
    }
}

/// No disc: the handshake compares names only.
pub struct Names;

impl Disc for Names {}

/// 64-bit FNV-1a, the same on every platform: for a [`Disc`] to hash a file's bytes with.
pub fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3))
}

/// What one peer brings to a ride, made from the lobby's line-up as it stands at the host's Start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Handshake {
    pub build: String,
    pub track: String,
    pub track_hash: Option<u64>,
    /// Every seated player's bike set in seat order, with this peer's disc's hash of it.
    pub bikes: Vec<(String, Option<u64>)>,
    pub wrecks: bool,
    /// The room's Air Wrecks switch (P138); on from a peer that does not say.
    #[serde(default = "crate::room::on")]
    pub air_wrecks: bool,
    /// The room's Backwards Landings Bail switch (S161); off from a peer that does not say.
    #[serde(default)]
    pub backwards_bail: bool,
    pub biplane: bool,
    /// Every seated player's controller preset (P79) in seat order: each machine reads that seat's pad through it.
    #[serde(default)]
    pub presets: Vec<u8>,
    /// The host's pick; a peer's own is ignored (it rides the host's).
    pub seed: u32,
    /// A peer dropping in on a ride under way (P105): its player's saved markers on the ride's track
    /// and bike set ([`Disc::markers`]), which the host puts in its pick before the ride takes it on —
    /// it joined before it knew the track, and the room takes no new pick once riding. Not compared:
    /// they are the peer's own, and every machine is sent them in the ride's state.
    #[serde(default)]
    pub markers: String,
    /// The room's trick pack (MD19, [`Room::tricks`]), empty for the original's tricks, and this
    /// peer's hash of its own pack of that id ([`Disc::tricks`]).
    #[serde(default)]
    pub tricks: String,
    #[serde(default)]
    pub tricks_hash: Option<u64>,
}

/// The build: two builds can't be trusted to simulate alike. The crate's version and "dev" unless the shell stamps
/// its own (`set_build`, as opentennis's `hst` does with its git commit): dev builds of one version meet each other.
/// The dev id names the ride's tick too (N6, the 30 Hz one, `net::ALPN`'s 2), so Find greys out the lobbies of a
/// dev build still on the 60 Hz tick rather than offering one it cannot connect to.
static BUILD: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

pub fn build() -> &'static str {
    BUILD.get().copied().unwrap_or(concat!(env!("CARGO_PKG_VERSION"), " dev, 30 Hz"))
}

/// Stamps this build's id before any lobby; a second call changes nothing.
pub fn set_build(id: &'static str) {
    let _ = BUILD.set(id);
}

impl Handshake {
    pub fn of(room: &Room, disc: &dyn Disc) -> Handshake {
        Handshake {
            build: build().into(),
            track: room.track.clone(),
            track_hash: disc.track(&room.track),
            bikes: room.seats.iter().flatten().map(|s| (s.pick.bike.clone(), disc.bike(&s.pick.bike))).collect(),
            wrecks: room.wrecks,
            air_wrecks: room.air_wrecks,
            backwards_bail: room.backwards_bail,
            biplane: room.biplane,
            presets: room.seats.iter().flatten().map(|s| s.pick.preset).collect(),
            seed: room.seed,
            markers: String::new(),
            tricks: room.tricks.clone(),
            tricks_hash: if room.tricks.is_empty() { None } else { disc.tricks(&room.tricks) },
        }
    }

    /// A joining peer's handshake (`self`) against the host's: `Ok` to ride, else why not, in words for the player.
    pub fn check(&self, host: &Handshake) -> Result<(), String> {
        let differ = |what: &str, mine: String, theirs: String| Err(format!("Can't ride: {what} differs (host: {theirs}, you: {mine})."));
        if self.build != host.build {
            return differ("the game version", self.build.clone(), host.build.clone());
        }
        if self.track != host.track {
            return differ("the track", self.track.clone(), host.track.clone());
        }
        if self.track_hash != host.track_hash {
            return Err(format!("Can't ride: your disc's {} isn't the same as the host's.", self.track));
        }
        if self.bikes.len() != host.bikes.len() {
            return differ("the number of riders", self.bikes.len().to_string(), host.bikes.len().to_string());
        }
        for (n, (mine, theirs)) in self.bikes.iter().zip(&host.bikes).enumerate() {
            if mine.0 != theirs.0 {
                return differ(&format!("rider {}'s bike", n + 1), mine.0.clone(), theirs.0.clone());
            }
            if mine.1 != theirs.1 {
                let why = if mine.1.is_none() { "isn't on your disc" } else { "isn't the same on your disc as on the host's" };
                return Err(format!("Can't ride: rider {}'s bike set {} {why}.", n + 1, mine.0));
            }
        }
        let on = |b: bool| if b { "on" } else { "off" }.to_string();
        if self.wrecks != host.wrecks {
            return differ("wrecks", on(self.wrecks), on(host.wrecks));
        }
        if self.air_wrecks != host.air_wrecks {
            return differ("air wrecks", on(self.air_wrecks), on(host.air_wrecks));
        }
        if self.backwards_bail != host.backwards_bail {
            return differ("backwards landings bail", on(self.backwards_bail), on(host.backwards_bail));
        }
        if self.biplane != host.biplane {
            return differ("the biplane", on(self.biplane), on(host.biplane));
        }
        let tricks = |id: &str| if id.is_empty() { "none, the original's tricks".to_string() } else { id.to_string() };
        if self.tricks != host.tricks {
            return differ("the trick pack", tricks(&self.tricks), tricks(&host.tricks));
        }
        if self.tricks_hash != host.tricks_hash {
            let why = if self.tricks_hash.is_none() { "isn't in your mods folder" } else { "isn't the same in your mods folder as in the host's" };
            return Err(format!("Can't ride: the host's trick pack {} {why}.", host.tricks));
        }
        if self.presets != host.presets {
            let n = (0..self.presets.len().max(host.presets.len())).find(|&n| self.presets.get(n) != host.presets.get(n)).unwrap_or(0);
            let said = |p: Option<&u8>| p.map_or("none".to_string(), |p| p.to_string());
            return differ(&format!("rider {}'s controller preset", n + 1), said(self.presets.get(n)), said(host.presets.get(n)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::room::Pick;

    fn host() -> Handshake {
        Handshake {
            build: build().into(),
            track: "free06".into(),
            track_hash: Some(1),
            bikes: vec![("race125p".into(), Some(7)), ("pro250a".into(), Some(8))],
            wrecks: true,
            air_wrecks: true,
            backwards_bail: false,
            biplane: false,
            presets: vec![0, 2],
            seed: 99,
            markers: String::new(),
            tricks: String::new(),
            tricks_hash: None,
        }
    }

    #[test]
    fn the_same_ride_joins_and_each_difference_is_refused() {
        let h = host();
        assert_eq!(Handshake { seed: 1, ..h.clone() }.check(&h), Ok(()), "the joiner's own seed doesn't matter");
        let refused = |p: Handshake, says: &str| {
            let e = p.check(&h).unwrap_err();
            assert!(e.contains(says), "{e}");
        };
        refused(Handshake { build: "0.0.0 other".into(), ..h.clone() }, "game version");
        refused(Handshake { track: "free02".into(), ..h.clone() }, "the track differs (host: free06, you: free02)");
        refused(Handshake { track_hash: Some(2), ..h.clone() }, "disc's free06");
        refused(Handshake { bikes: vec![("race125p".into(), Some(7)), ("pro500a".into(), Some(8))], ..h.clone() }, "rider 2's bike differs");
        refused(Handshake { bikes: vec![("race125p".into(), Some(6)), ("pro250a".into(), Some(8))], ..h.clone() }, "rider 1's bike set race125p isn't the same");
        refused(Handshake { bikes: vec![("race125p".into(), None), ("pro250a".into(), Some(8))], ..h.clone() }, "isn't on your disc");
        refused(Handshake { bikes: vec![("race125p".into(), Some(7))], ..h.clone() }, "number of riders");
        refused(Handshake { wrecks: false, ..h.clone() }, "wrecks differs (host: on, you: off)");
        refused(Handshake { air_wrecks: false, ..h.clone() }, "air wrecks differs (host: on, you: off)");
        refused(Handshake { backwards_bail: true, ..h.clone() }, "backwards landings bail differs (host: off, you: on)");
        refused(Handshake { biplane: true, ..h.clone() }, "the biplane differs");
        refused(Handshake { presets: vec![0, 3], ..h.clone() }, "rider 2's controller preset differs (host: 2, you: 3)");
        refused(Handshake { presets: vec![0], ..h.clone() }, "rider 2's controller preset differs (host: 2, you: none)");
        refused(Handshake { tricks: "mxvatv".into(), tricks_hash: Some(5), ..h.clone() }, "the trick pack differs (host: none, the original's tricks, you: mxvatv)");
    }

    /// A disc whose mods folder holds one trick pack.
    struct Pack(&'static str, u64);

    impl Disc for Pack {
        fn tricks(&self, id: &str) -> Option<u64> {
            (id == self.0).then_some(self.1)
        }
    }

    #[test]
    fn every_seat_rides_the_hosts_trick_pack() {
        let mut room = Room::new("h", "free06", 2);
        room.take(crate::room::HOST, &[Pick::new("thq1", "race125p")]);
        assert_eq!(Handshake::of(&room, &Pack("mxvatv", 1)).tricks_hash, None, "the original's tricks hash nothing");
        assert_eq!(Handshake::of(&room, &Names).check(&Handshake::of(&room, &Pack("mxvatv", 1))), Ok(()));
        room.tricks = "mxvatv".into();
        let host = Handshake::of(&room, &Pack("mxvatv", 1));
        assert_eq!(host.tricks_hash, Some(1));
        assert_eq!(Handshake::of(&room, &Pack("mxvatv", 1)).check(&host), Ok(()), "the same pack rides");
        let e = Handshake::of(&room, &Pack("mxvatv", 2)).check(&host).unwrap_err();
        assert_eq!(e, "Can't ride: the host's trick pack mxvatv isn't the same in your mods folder as in the host's.");
        let e = Handshake::of(&room, &Names).check(&host).unwrap_err();
        assert_eq!(e, "Can't ride: the host's trick pack mxvatv isn't in your mods folder.");
        let e = Handshake::of(&room, &Pack("untamed", 1)).check(&host).unwrap_err();
        assert!(e.contains("trick pack mxvatv isn't in your mods folder"), "{e}");
        let old: Room = serde_json::from_str(&serde_json::to_string(&Room::new("h", "free06", 2)).unwrap()).unwrap();
        assert_eq!(old.tricks, "", "a room from a build before MD19 rides the original's tricks");
    }

    #[test]
    fn every_seats_preset_is_in_it() {
        let mut room = Room::new("h", "free06", 3);
        let mut late = Pick::new("thq2", "pro250a");
        late.preset = 3;
        room.take(crate::room::HOST, &[Pick::new("thq1", "race125p")]);
        room.take("a", &[late]);
        assert_eq!(Handshake::of(&room, &Names).presets, [0, 3]);
        let old: Pick = serde_json::from_str(r#"{"rider":"thq1","bike":"race125p"}"#).unwrap();
        assert_eq!(old.preset, 0, "a pick from a build before the preset");
    }

    #[test]
    fn fnv_is_the_reference_hash() {
        // FNV-1a 64's published test vectors
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
