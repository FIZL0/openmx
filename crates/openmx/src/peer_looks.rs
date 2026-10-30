//! Every online peer's rider drawn in their own look (N16): the room's picks (P86's
//! [`openmx_net::Pick::look`], the rider archive, the bike model) turned into the garage's
//! [`roster::Pick`]s the shell dresses a seat's rig from, in the order the shell holds the seats —
//! this machine's first, then every other in seat order ([`crate::online::Online::drawn`]).
//!
//! A ride from the lobby knows every seat before its first tick and dresses each as the split
//! screen's other players are dressed ([`lobby_others`]). A ride that takes peers on as it goes (N10,
//! a freeride opened online, or a drop-in onto one) grows its seats tick by tick, and a seat can
//! change hands (a leaver's seat taken, a restart moving seats): [`Dressed`] says which shell seat
//! wears a look it is not dressed in yet.
//!
//! Looks only: nothing here reaches the simulation, which rides each seat's `.phy` as the room's
//! bike set says.

use openmx_net::room::Seat;
use openmx_net::{Pick, Room};

use crate::roster;

/// The shell seats' order for a ride of `seats` seats whose own first seat is `mine`: `mine`, then
/// every other in seat order.
pub fn drawn(seats: usize, mine: usize) -> Vec<usize> {
    std::iter::once(mine).chain((0..seats).filter(|&s| s != mine)).collect()
}

/// A room seat's pick as the garage's: its rider archive and look, its bike set (the `.phy` the
/// simulation rides, which only names the model's paint here) and its bike model. `own` is this
/// machine's: what a pick that does not say draws as.
pub fn garage_pick(pick: &Pick, own: &roster::Pick) -> roster::Pick {
    roster::Pick {
        bike: if pick.model.trim().is_empty() { own.bike.clone() } else { pick.model.trim().to_ascii_lowercase() },
        // A peer is a person: P, whatever its pick says ([`roster::Set::ridden`]).
        set: roster::Set::from_player_word(&pick.bike).unwrap_or(own.set.ridden()),
        rider: if pick.rider.trim().is_empty() { own.rider.clone() } else { pick.rider.trim().to_ascii_lowercase() },
        look: roster::Look::parse(&pick.look),
        preset: String::new(),
    }
}

/// The lobby ride's other shell seats (1..), each in its room seat's pick; an open seat draws as
/// `own`, and is not drawn while nobody sits in it. `None` when `me` has no seat in the room.
pub fn lobby_others(room: &Room, me: &str, own: &roster::Pick) -> Option<Vec<roster::Pick>> {
    let mine = room.seats.iter().position(|s| s.as_ref().is_some_and(|s| s.peer == me))?;
    let picks = drawn(room.seats.len(), mine)
        .into_iter()
        .skip(1)
        .map(|s| room.seats[s].as_ref().map_or_else(|| own.clone(), |seat| garage_pick(&seat.pick, own)))
        .collect();
    Some(picks)
}

/// What a seat is dressed for: who sits in it and what they wear. A change of any of it dresses the
/// seat again.
pub fn key(seat: &Seat) -> String {
    let p = &seat.pick;
    format!("{}|{}|{}|{}", seat.peer, p.rider, p.model, p.look)
}

/// The look each shell seat is dressed in now, by [`key`]: `None` for one dressed as the ride
/// loaded it (this machine's own, or an open seat).
#[derive(Clone, Debug, Default)]
pub struct Dressed {
    pub keys: Vec<Option<String>>,
}

impl Dressed {
    /// Every shell seat as the lobby ride dressed them: seat `d`'s room seat's.
    pub fn lobby(room: &Room, me: &str) -> Dressed {
        let Some(mine) = room.seats.iter().position(|s| s.as_ref().is_some_and(|s| s.peer == me)) else {
            return Dressed::default();
        };
        let keys = drawn(room.seats.len(), mine).into_iter().enumerate().map(|(d, s)| room.seats[s].as_ref().filter(|_| d > 0).map(key)).collect();
        Dressed { keys }
    }

    /// The shell seats past the first that sit someone wearing another look than they are dressed
    /// in: `seats[d]` is shell seat `d`'s room seat now, `None` for an empty one. A seat that empties
    /// keeps its dress — it is not drawn — and the first is this machine's own, which the ride
    /// dressed.
    pub fn changed(&self, seats: &[Option<&Seat>]) -> Vec<usize> {
        (1..seats.len())
            .filter(|&d| seats[d].is_some_and(|s| self.keys.get(d).cloned().flatten().as_deref() != Some(key(s).as_str())))
            .collect()
    }

    /// Shell seat `d` dressed as `seat` now.
    pub fn dress(&mut self, d: usize, seat: &Seat) {
        if self.keys.len() <= d {
            self.keys.resize(d + 1, None);
        }
        self.keys[d] = Some(key(seat));
    }
}
