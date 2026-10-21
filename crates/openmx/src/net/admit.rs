//! A peer joining the ride under way (N10): nobody's ride restarts, and every machine takes the
//! join on at one tick.
//!
//! The host decides when. At the first frame it can (no restart or resync in flight), it takes its
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
}

impl Admit {
    /// Joins asked for and not taken on yet.
    pub fn pending(&self) -> usize {
        self.pending.len()
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
    if !np.host() || np.admit.pending.is_empty() || np.end.is_some() || np.recover.ready.is_some() || np.resync.due || np.resync.sent {
        return;
    }
    // no settled state this frame: the next
    let Some((tick, data)) = rollback::confirmed(np, game) else { return };
    let now = game.save();
    let pending = std::mem::take(&mut np.admit.pending);
    let grown = grow(np, game, &data, &pending);
    game.restore(&now);
    let (lineup, data) = match grown {
        Ok(g) => g,
        Err(e) => {
            let who: Vec<&str> = pending.iter().map(|p| p.0.as_str()).collect();
            return np.say(format!("online: {} cannot join: {e}", who.join(", ")));
        }
    };
    let round = np.rb.round.wrapping_add(1);
    let n = data.len().div_ceil(PIECE).max(1);
    for p in lineup.peers().into_iter().filter(|p| *p != np.me) {
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
