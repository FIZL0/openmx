//! A peer joining the ride under way (N10): nobody's ride restarts, and every machine takes the
//! join on at one tick.
//!
//! The host decides when. At the first frame it can (no restart or resync in flight) once it has heard
//! from the joiner — any datagram: a joiner's game sends keep-alives once its ride is loaded, and a
//! peer taken on before that would hold every other machine up for its load — it takes its
//! state before the last tick every seat's pads are in for — [`super::rollback::confirmed`], what a
//! resync sends — puts the joiner's seats into it, each a new rider in the spawn state the caller
//! handed over ([`super::Game::seat`]), and sends that state with the new line-up as a restart
//! ([`super::recover`]'s `Msg::Restart`, under the next round) to every peer, the joiner included.
//! Every machine, the host too, then decodes the same bytes and starts a new ggrs session at that
//! tick with the new player count, so from the join tick on every machine rides one state on one
//! line-up. A host riding alone has every pad in already, so its state is the one it is at and its
//! ride goes on without a tick lost; with others in, the ride steps back to the settled tick — the
//! few ticks a rollback would re-run anyway — and rides on from there.
//!
//! A change of the game's modifiers by the host (P100, [`super::Netplay::remodify`]) is taken the
//! same way, with or without a join: the settled state goes out as a restart with `Msg::Mods` ahead
//! of it, and every machine decodes the state and moves it onto the new set
//! ([`super::Game::remodify`]) before the new session's first tick, so every machine rides the new
//! numbers from the same tick on.
//!
//! The joiner waits with [`super::Netplay::joining`]: no seat and no session, keep-alives to the
//! host, until the restart comes.

use super::link::{Msg, PIECE};
use super::{Game, Netplay, SEATS, recover, rollback};

#[derive(Default)]
pub struct Admit {
    /// The host: (peer, its seats, the state each spawns in), waiting for a frame that can take
    /// them on.
    pending: Vec<(String, Vec<usize>, Vec<u8>)>,
    /// Every join this machine sent: (peer, the tick it joined at).
    pub joins: Vec<(String, u32)>,
    /// The peers heard from: a pending join goes ahead once its peer is among them.
    heard: std::collections::BTreeSet<String>,
    /// The host: the modifiers to change to at the next frame that can (P100), as a join is taken.
    pub(super) mods: Option<String>,
}

impl Admit {
    /// Joins asked for and not taken on yet.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// A datagram came from `peer`.
    pub fn heard(&mut self, peer: &str) {
        if self.pending.iter().any(|p| p.0 == peer) && !self.heard.contains(peer) {
            self.heard.insert(peer.to_string());
        }
    }

    /// `peer` will not come after all (it left the lobby before it was taken on).
    pub fn forget(&mut self, peer: &str) {
        self.pending.retain(|p| p.0 != peer);
        self.heard.remove(peer);
    }
}

/// The host is asked to take `peer` on at `seats`.
pub(super) fn ask<S: Clone + 'static>(np: &mut Netplay<S>, peer: &str, seats: &[usize], spawn: Vec<u8>) -> Result<(), String> {
    if !np.host() {
        return Err("only the host takes a peer on".into());
    }
    if peer == np.me || np.lineup.peers().iter().any(|p| p == peer) || np.admit.pending.iter().any(|p| p.0 == peer) {
        return Err(format!("{peer} is in the ride already"));
    }
    if seats.is_empty() {
        return Err(format!("{peer} has no seat to join on"));
    }
    let taken = |s: usize| np.lineup.seats.get(s).is_some_and(Option::is_some) || np.admit.pending.iter().any(|p| p.1.contains(&s));
    if let Some(&s) = seats.iter().find(|&&s| s >= SEATS || taken(s)) {
        return Err(format!("seat {} is not free", s + 1));
    }
    np.admit.pending.push((peer.to_string(), seats.to_vec(), spawn));
    Ok(())
}

/// The host, after a frame's ticks: the joins asked for go out and are taken on, if this frame can.
pub(super) fn run<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G) {
    let joins = np.admit.pending.iter().any(|p| np.admit.heard.contains(&p.0));
    if !np.host() || !(joins || np.admit.mods.is_some()) || np.end.is_some() || np.recover.ready.is_some() || np.resync.due || np.resync.sent {
        return;
    }
    // no settled state this frame: the next
    let Some((tick, data)) = rollback::confirmed(np, game) else { return };
    let now = game.save();
    let (pending, waiting) = std::mem::take(&mut np.admit.pending).into_iter().partition(|p| np.admit.heard.contains(&p.0));
    np.admit.pending = waiting;
    let pending: Vec<(String, Vec<usize>, Vec<u8>)> = pending;
    for p in &pending {
        np.admit.heard.remove(&p.0);
    }
    let grown = grow(np, game, &data, &pending);
    game.restore(&now);
    let (lineup, data) = match grown {
        Ok(g) => g,
        Err(e) if pending.is_empty() => {
            np.admit.mods = None;
            return np.say(format!("online: the modifiers cannot change: {e}"));
        }
        Err(e) => {
            let who: Vec<&str> = pending.iter().map(|p| p.0.as_str()).collect();
            return np.say(format!("online: {} cannot join: {e}", who.join(", ")));
        }
    };
    let round = np.rb.round.wrapping_add(1);
    let n = data.len().div_ceil(PIECE).max(1);
    // the modifiers go with every restart from here (P100): a change at this tick, or the set the
    // ride is on for a joiner whose machine may have loaded the room's older one
    let (was, to) = (np.mods.clone(), np.admit.mods.take().unwrap_or_else(|| np.mods.clone()));
    for p in lineup.peers().into_iter().filter(|p| *p != np.me) {
        np.link.send(&p, Msg::Mods { round, was: was.clone(), to: to.clone() });
        for i in 0..n {
            let piece = data[i * PIECE..((i + 1) * PIECE).min(data.len())].to_vec();
            np.link.send(&p, Msg::Restart { round, tick, lineup: lineup.clone(), data: piece, last: i + 1 == n });
        }
    }
    for (peer, seats, _) in &pending {
        let seats: Vec<String> = seats.iter().map(|s| (s + 1).to_string()).collect();
        np.say(format!("online: {peer} joins the ride at tick {tick}, seat {}", seats.join(", ")));
        np.admit.joins.push((peer.clone(), tick));
    }
    np.recover.ready = Some((round, tick, lineup, data));
    np.recover.mods = Some((round, was, to));
    recover::restarted(np, game);
}

/// The settled state `data` with every pending join's seats spawned into it, and the line-up with
/// them seated.
fn grow<S: Clone + 'static, G: Game<State = S>>(
    np: &Netplay<S>,
    game: &mut G,
    data: &[u8],
    pending: &[(String, Vec<usize>, Vec<u8>)],
) -> Result<(super::Lineup, Vec<u8>), String> {
    game.decode(data)?;
    let mut lineup = np.lineup.clone();
    for (peer, seats, spawn) in pending {
        for &s in seats {
            game.seat(s, spawn)?;
            if lineup.seats.len() <= s {
                lineup.seats.resize(s + 1, None);
            }
            lineup.seats[s] = Some(peer.clone());
        }
    }
    Ok((lineup, game.encode().ok_or("this ride cannot say its state in bytes")?))
}
