//! After a desync the host sends the others its state and they take it on, so the ride goes on in
//! sync (opentennis's B154b, its rollback half).
//!
//! The state is the host's before tick F, the last tick every seat's pads are in for there
//! ([`super::rollback::confirmed`], from ggrs' own cells). A peer that has it waits until every seat's
//! pads before F are in here too — a rollback must never go back past it to a cell that was not
//! mended — then puts the state in, runs ticks F..N again on the pads it ran them on (its log; any
//! guessed ones ggrs puts right as usual), saving each into ggrs' cells, and tells the host
//! (`Msg::Resynced(N)`). Hashes of ticks before N are from before the resync: neither side reports
//! them, nor anything while a sent state is not taken yet.

use super::link::{Msg, PIECE};
use super::{Game, Netplay, rollback};

#[derive(Default)]
pub struct Resync {
    /// A peer: the host's state before tick `.0`, waiting for this machine to get there.
    pub pending: Option<(u32, Vec<u8>)>,
    /// A peer: the pieces of the host's state heard so far.
    pieces: Vec<u8>,
    /// The host: a desync was seen, the state goes out after the next tick.
    pub due: bool,
    /// The host: a state went out and no peer has taken it yet.
    pub sent: bool,
    /// Hashes of ticks before this are from before the last resync.
    pub fresh: u32,
    /// Resyncs this machine has done (sent and taken, or taken).
    pub done: u32,
}

impl Resync {
    /// A desync at tick `t` is one a resync is already mending.
    pub fn stale(&self, t: u32) -> bool {
        t < self.fresh || self.sent
    }
}

/// A desync was seen: the host's state goes out after the next tick.
pub(super) fn ask<S: Clone + 'static>(np: &mut Netplay<S>) {
    if np.host() && !np.resync.sent {
        np.resync.due = true;
    }
}

/// A peer's message about a resync.
pub(super) fn heard<S: Clone + 'static>(np: &mut Netplay<S>, from: &str, m: Msg) {
    let host = np.host();
    match m {
        Msg::State { tick, data, last } if !host && from == np.lineup.host() => {
            np.resync.pieces.extend(data);
            if last {
                np.resync.pending = Some((tick, std::mem::take(&mut np.resync.pieces)));
            }
        }
        Msg::Resynced(n) if host => {
            np.say(format!("online: {from} took the state, in sync from tick {n}"));
            let r = &mut np.resync;
            (r.fresh, r.sent, r.done) = (r.fresh.max(n), false, r.done + 1);
            np.desynced = false;
        }
        _ => {}
    }
}

/// The host after a desync: the state at the last tick every seat's pads are in for goes out.
pub(super) fn send_state<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G) {
    if !np.resync.due {
        return;
    }
    let Some((tick, data)) = rollback::confirmed(np, game) else {
        if game.encode().is_none() {
            np.resync.due = false;
            np.say("online: this ride cannot send its state, so the desync is not mended".into());
        }
        return;
    };
    np.say(format!("online: sending the state before tick {tick} ({} KB) to resync", data.len() / 1024));
    np.resync.due = false;
    np.resync.sent = true;
    let n = data.len().div_ceil(PIECE).max(1);
    for i in 0..n {
        let piece = data[i * PIECE..((i + 1) * PIECE).min(data.len())].to_vec();
        np.tell(Msg::State { tick, data: piece, last: i + 1 == n });
    }
}

/// A peer with the host's state, once every seat's pads before its tick are in here too.
pub(super) fn take_state<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G) {
    let confirmed = np.confirmed();
    let Some((from, data)) = np.resync.pending.take_if(|p| p.0 as i64 <= confirmed) else { return };
    if (from as usize) < np.rb.base || from as usize > np.rb.frame {
        return;
    }
    if let Err(e) = game.decode(&data) {
        return np.say(format!("online: the host's state did not load: {e}"));
    }
    let now = np.rb.frame;
    rollback::rerun(np, game, from as usize);
    np.say(format!("online: took the host's state before tick {from}, ran {} ticks again: in sync from tick {now}", now - from as usize));
    let r = &mut np.resync;
    (r.pending, r.fresh, r.done) = (None, now as u32, r.done + 1);
    np.desynced = false;
    np.tell(Msg::Resynced(now as u32));
}
