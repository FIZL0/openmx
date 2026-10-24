//! A player leaving the ride under way (N11): their seats go at one tick on every machine, and the
//! ride carries on for the rest.
//!
//! A machine's player picks Leave Session in the pause ([`super::Netplay::leave`]): a guest tells the
//! host which of its seats go (`Msg::Leave`); the host's own go into its own list. At the first frame
//! the host can make a restart (none in flight), it empties those seats in the line-up and sends its
//! settled state with it as a restart — [`super::recover`]'s, as a silent peer's seats are emptied
//! there, and as [`super::admit`] takes a joiner on — so every machine, the leaver too, takes the
//! same state and line-up at the same tick. A leaver whose every seat went is then out of the ride
//! ("you left the ride"); one with seats left (a second local player leaving) rides on in them.
//!
//! **The host leaving** empties its own seats the same way, and the restart's line-up then has
//! another host: the peer in the first taken seat ([`super::Lineup::host`]). That peer is the host
//! from the restart's tick on — it counts the others' silences, takes joins and leaves — and the
//! lobby's transport hands it the room ([`openmx_net`]'s `Cmd::HandOver`). The old host keeps
//! sending until every peer has the restart ([`flushed`]), then goes.
//!
//! A leaver that hears nothing back in [`WAIT`] (the host gone) is out anyway: the others empty its
//! seats once it has been silent long enough, as any silent peer's.

use super::link::Msg;
use super::Netplay;

/// How long a leaver waits for the restart that takes its seats out, seconds; and how long a host
/// that left keeps sending until every peer has that restart.
pub const WAIT: f32 = 5.0;

#[derive(Default)]
pub struct Leave {
    /// The host: seats whose players asked to go, for the next restart it can make.
    pub asked: Vec<usize>,
    /// This machine asked for these of its seats to go, this many seconds ago.
    pub mine: Option<(Vec<usize>, f32)>,
    /// Seconds since this machine's session ended after it left: what [`flushed`] counts.
    pub(super) since: f32,
    /// The host: every leave it made a restart for, (peer, its seats, the tick they went at).
    pub left: Vec<(String, Vec<usize>, u32)>,
    /// A host that left: the peer its restart made host, which the lobby's room goes to as well.
    pub heir: Option<String>,
}

impl Leave {
    /// Whether this machine asked to leave.
    pub fn leaving(&self) -> bool {
        self.mine.is_some()
    }
}

/// This machine's seats `seats` go (all of them when empty).
pub(super) fn ask<S: Clone + 'static>(np: &mut Netplay<S>, seats: &[usize]) -> Result<(), String> {
    let mine = np.lineup.mine(&np.me);
    if mine.is_empty() {
        return Err("this machine has no seat in the ride".into());
    }
    let seats: Vec<usize> = if seats.is_empty() { mine.clone() } else { seats.to_vec() };
    if let Some(s) = seats.iter().find(|s| !mine.contains(s)) {
        return Err(format!("seat {} is not this machine's", s + 1));
    }
    let all = seats.len() == mine.len();
    // the last machine in the ride: nobody to carry on, nothing to agree
    if all && np.lineup.peers().len() == 1 {
        np.leave.mine = Some((seats, 0.0));
        np.finish("you left the ride".into());
        return Ok(());
    }
    if np.host() {
        take(np, &np.me.clone(), &seats);
    } else {
        let host = np.lineup.host().to_string();
        np.link.send(&host, Msg::Leave { seats: seats.clone() });
    }
    np.say(format!("online: leaving the ride (seat {})", words(&seats)));
    np.leave.mine = Some((seats, 0.0));
    Ok(())
}

/// The host hears `from` ask for its seats `seats` to go.
pub(super) fn heard<S: Clone + 'static>(np: &mut Netplay<S>, from: &str, seats: Vec<usize>) {
    if np.host() {
        take(np, from, &seats);
    }
}

/// `peer`'s `seats` are to go at the host's next restart: only those it sits in.
fn take<S: Clone + 'static>(np: &mut Netplay<S>, peer: &str, seats: &[usize]) {
    for &s in seats {
        if np.lineup.seats.get(s).is_some_and(|p| p.as_deref() == Some(peer)) && !np.leave.asked.contains(&s) {
            np.leave.asked.push(s);
        }
    }
}

/// Each frame (`dt` real seconds): the host makes its restart for the seats asked to go, if it can
/// this frame; a leaver counts its wait.
pub(super) fn tick<S: Clone + 'static>(np: &mut Netplay<S>, dt: f32) {
    if np.end.is_some() {
        np.leave.since += dt;
        return;
    }
    if let Some((_, waited)) = &mut np.leave.mine {
        *waited += dt;
        if *waited >= WAIT {
            let (seats, _) = np.leave.mine.take().unwrap();
            if seats.len() == np.lineup.mine(&np.me).len() {
                np.leave.mine = Some((seats, WAIT));
                // nothing owed that will be taken: out now
                np.leave.since = WAIT;
                return np.finish("you left the ride (the host did not answer)".into());
            }
            np.say(format!("online: the host did not answer: seat {} is still in the ride", words(&seats)));
        }
    }
    if !np.host() || np.leave.asked.is_empty() || np.recover.ready.is_some() {
        return;
    }
    let asked = std::mem::take(&mut np.leave.asked);
    let mut lineup = np.lineup.clone();
    let mut who: Vec<(String, Vec<usize>)> = Vec::new();
    for &s in &asked {
        let Some(peer) = lineup.seats[s].take() else { continue };
        match who.iter_mut().find(|w| w.0 == peer) {
            Some(w) => w.1.push(s),
            None => who.push((peer, vec![s])),
        }
    }
    if lineup.seats.iter().all(Option::is_none) {
        np.tell(Msg::End("everyone left the ride".into()));
        return np.finish("everyone left the ride".into());
    }
    // the tick is the restart's, once it is made (`recover::restarted`)
    for (peer, seats) in who {
        let gone = lineup.mine(&peer).is_empty();
        np.say(format!("online: {peer} leaves seat {}{}", words(&seats), if gone { "" } else { " and rides on in the rest" }));
        np.leave.left.push((peer, seats, u32::MAX));
    }
    if lineup.host() != np.lineup.host() {
        np.say(format!("online: {} hosts the ride from here", lineup.host()));
        np.leave.heir = Some(lineup.host().to_string());
    }
    np.recover.ready = Some((0, u32::MAX, lineup, Vec::new()));
}

/// After a leave ended this machine's session: whether what it owes the others is sent — every peer
/// acknowledged every message (the restart that took its seats out, from a host) — or it has
/// waited [`WAIT`] for that. The caller keeps passing datagrams both ways until then.
pub fn flushed<S: Clone + 'static>(np: &Netplay<S>) -> bool {
    np.end.is_some() && (np.link.owed() == 0 || np.leave.since >= WAIT)
}

/// Seats as players count them: "2" or "2, 3".
fn words(seats: &[usize]) -> String {
    seats.iter().map(|s| (s + 1).to_string()).collect::<Vec<_>>().join(", ")
}
