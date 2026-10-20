//! What every peer must agree on before a ride starts (opentennis's `hst-base/src/handshake.rs`, M2e): the host
//! picks the seed, and each peer's handshake has to match the host's on everything that changes the simulation,
//! else that peer is refused with a message saying what differs.
//!
//! Here that is the track, every player's bike set (the `.phy` the simulation loads for them, D7's loader), wrecks
//! on or off (`game_sim::state::Sense::wrecks_enabled` in the shell) and the biplane switch. A name is not enough
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
    pub biplane: bool,
    /// The host's pick; a peer's own is ignored (it rides the host's).
    pub seed: u32,
}

/// The build: two builds can't be trusted to simulate alike. The crate's version and "dev" unless the shell stamps
/// its own (`set_build`, as opentennis's `hst` does with its git commit): dev builds of one version meet each other.
static BUILD: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();

pub fn build() -> &'static str {
    BUILD.get().copied().unwrap_or(concat!(env!("CARGO_PKG_VERSION"), " dev"))
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
            biplane: room.biplane,
            seed: room.seed,
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
        if self.biplane != host.biplane {
            return differ("the biplane", on(self.biplane), on(host.biplane));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Handshake {
        Handshake {
            build: build().into(),
            track: "free06".into(),
            track_hash: Some(1),
            bikes: vec![("race125p".into(), Some(7)), ("pro250a".into(), Some(8))],
            wrecks: true,
            biplane: false,
            seed: 99,
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
        refused(Handshake { biplane: true, ..h.clone() }, "the biplane differs");
    }

    #[test]
    fn fnv_is_the_reference_hash() {
        // FNV-1a 64's published test vectors
        assert_eq!(fnv(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv(b"foobar"), 0x8594_4171_f739_67e8);
    }
}
