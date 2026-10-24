//! A peer that goes silent mid-ride (a crash, a lost network, a frozen machine), after opentennis's
//! `recover.rs` (B196).
//!
//! Every machine sends a keep-alive every [`ALIVE`] seconds, and any datagram counts as one. The host,
//! hearing nothing from a peer for [`SILENT`] seconds, says it is away (`Msg::Away`) and everyone shows
//! "Waiting for …" with a countdown; the ride holds by itself (rollback stalls at its window without
//! the peer's pads). The same peer heard again: the ride just goes on. Still away after [`WAIT`], its
//! seats are emptied: the host sends a restart (`Msg::Restart`) — its state before a tick every seat's
//! pads are in for, and the new line-up — and every machine, the host too, takes the state on and
//! starts a new ggrs session from that tick, numbered one more so the old one's datagrams are dropped.
//! A seat nobody sits in has an unplugged pad ([`super::Game::step`]). The host silent (N11): the next
//! peer in seat order waits [`HOST_WAIT`] more for each peer before it (in case one of them takes over
//! first), then empties the host's seats from its own settled state and sends that restart as the new
//! host; a game that cannot say its state in bytes ends the ride, naming the host.
//!
//! What opentennis has and this does not, because it needs a lobby or a menu (N3): the choice between a
//! bot, the lobby and leaving; a player rejoining; "switched away".

use std::collections::BTreeMap;

use super::link::{self, Msg, PIECE};
use super::{Game, Lineup, Netplay, rollback};

/// Keep-alive every this many seconds.
pub const ALIVE: f32 = 0.5;
/// A peer not heard for this long is away; before it has been heard at all, [`LOADING`].
pub const SILENT: f32 = 3.0;
pub const LOADING: f32 = 60.0;
/// How long the others wait for an away peer before its seats are emptied: opentennis's before its
/// rejoin (M126) made it two minutes, because there is nothing to rejoin here yet.
pub const WAIT: f32 = 20.0;
/// How long the others wait for a silent host past [`SILENT`].
pub const HOST_WAIT: f32 = 10.0;

pub struct Recover {
    /// Seconds since each other peer was last heard, and whether it has been at all.
    quiet: BTreeMap<String, (f32, bool)>,
    alive: f32,
    /// The host: the peers away and seconds waited; the countdown last sent.
    pub away: Vec<String>,
    waited: f32,
    sent: Option<u32>,
    /// A guest: the host's last word (who, seconds left).
    told: Option<(Vec<String>, u32)>,
    /// A restart's pieces so far, and a whole one to take on (`tick` `u32::MAX`: the host's own,
    /// its state still to be read).
    pieces: Vec<u8>,
    pub(super) ready: Option<(u8, u32, Lineup, Vec<u8>)>,
    /// The modifiers a restart takes on (P100): its round, from, to.
    pub(super) mods: Option<(u8, String, String)>,
    /// Restarts so far.
    pub restarts: u32,
    /// This machine is taking the ride over from a silent host (N11): its restart is the one ready.
    pub(super) taking: bool,
    /// Every time this machine took a ride over from a silent host: (the old host, the tick).
    pub takeovers: Vec<(String, u32)>,
}

impl Recover {
    pub fn new(others: impl Iterator<Item = String>) -> Recover {
        Recover {
            quiet: others.map(|p| (p, (0.0, false))).collect(),
            alive: 0.0,
            away: Vec::new(),
            waited: 0.0,
            sent: None,
            told: None,
            pieces: Vec::new(),
            ready: None,
            mods: None,
            restarts: 0,
            taking: false,
            takeovers: Vec::new(),
        }
    }

    /// Any word from a peer.
    pub fn heard_from(&mut self, p: &str) {
        if let Some(q) = self.quiet.get_mut(p) {
            *q = (0.0, true);
        }
    }

    /// The keep-alives due now.
    pub fn alive(&mut self, lineup: &Lineup, me: &str) -> Vec<(String, Vec<u8>)> {
        if self.alive < ALIVE {
            return Vec::new();
        }
        self.alive = 0.0;
        lineup.peers().into_iter().filter(|p| p != me).map(|p| (p, link::alive())).collect()
    }
}

/// A recovery message in.
pub(super) fn heard<S: Clone + 'static>(np: &mut Netplay<S>, from: &str, m: Msg) {
    // a restart from the peer that takes over from a silent host (N11): it hosts the line-up it
    // sends, which the old host is out of
    let takeover = match &m {
        Msg::Restart { lineup, .. } => lineup.host() == from && np.lineup.peers().iter().any(|p| p == from) && !lineup.peers().iter().any(|p| p == np.lineup.host()),
        _ => false,
    };
    if np.host() || from != np.lineup.host() && !takeover {
        return;
    }
    let r = &mut np.recover;
    match m {
        Msg::Away { who, left } => r.told = (!who.is_empty()).then_some((who, left)),
        Msg::Mods { round, was, to } => r.mods = Some((round, was, to)),
        Msg::Restart { round, tick, lineup, data, last } => {
            r.pieces.extend(data);
            if last {
                r.taking = false;
                r.ready = Some((round, tick, lineup, std::mem::take(&mut r.pieces)));
            }
        }
        _ => {}
    }
}

/// Each frame (`dt` real seconds): who is silent, the countdowns; the notice to show.
pub(super) fn tick<S: Clone + 'static>(np: &mut Netplay<S>, dt: f32) {
    np.notice.clear();
    let (host, hostname, me, peers) = (np.host(), np.lineup.host().to_string(), np.me.clone(), np.lineup.peers());
    let r = &mut np.recover;
    r.alive += dt;
    if np.end.is_some() {
        return;
    }
    for q in r.quiet.values_mut() {
        q.0 += dt;
    }
    let t = np.timing;
    let limit = |q: (f32, bool)| if q.1 { t.silent } else { t.loading };
    if !host {
        let q = r.quiet.get(&hostname).copied().unwrap_or_default();
        // a silent host's ride goes to the others (N11): the peer in the first seat after it takes
        // it over once the host has been silent `host_wait` past away, the next one a `host_wait`
        // later if that one did not, and so on down the seats
        let rest: Vec<String> = peers.iter().filter(|p| **p != hostname).cloned().collect();
        let rank = rest.iter().position(|p| *p == me).unwrap_or(0) + 1;
        let at = limit(q) + rank as f32 * t.host_wait;
        if q.0 > at && r.ready.is_none() {
            np.said.push(format!("online: the host stopped answering: {me} takes the ride over"));
            let mut lineup = np.lineup.clone();
            for s in lineup.seats.iter_mut() {
                if s.as_deref() == Some(hostname.as_str()) {
                    *s = None;
                }
            }
            np.recover.ready = Some((0, u32::MAX, lineup, Vec::new()));
            np.recover.taking = true;
            return;
        }
        if q.0 > limit(q) {
            let first = limit(q) + t.host_wait;
            np.notice = match rest.first() {
                Some(next) if q.0 <= first => format!("The host stopped answering: {next} takes over in {} s", (first - q.0).ceil()),
                _ => format!("The host stopped answering: taking over in {} s", (at - q.0).ceil()),
            };
        } else if let Some((who, left)) = &r.told {
            np.notice = format!("Waiting for {}... {left} s", who.join(", "));
        }
        return;
    }
    // the host: who is away and who is back
    for p in peers {
        if p == me {
            continue;
        }
        let q = r.quiet.get(&p).copied().unwrap_or_default();
        let away = r.away.contains(&p);
        let gone = q.0 > limit(q);
        if !away && gone {
            np.said.push(format!("online: {p} went silent"));
            r.away.push(p);
        } else if away && !gone {
            np.said.push(format!("online: {p} is answering again"));
            r.away.retain(|x| *x != p);
        }
    }
    if r.away.is_empty() {
        if r.sent.take().is_some() {
            np.tell(Msg::Away { who: Vec::new(), left: 0 });
        }
        np.recover.waited = 0.0;
        return;
    }
    r.waited += dt;
    let left = (t.wait - r.waited).ceil().max(0.0) as u32;
    let who = r.away.clone();
    np.notice = format!("Waiting for {}... {left} s", who.join(", "));
    if np.recover.sent != Some(left) {
        np.recover.sent = Some(left);
        np.tell(Msg::Away { who: who.clone(), left });
    }
    if np.recover.waited >= t.wait && np.recover.ready.is_none() {
        np.said.push(format!("online: {} did not come back: their seats are empty from here", who.join(", ")));
        let mut lineup = np.lineup.clone();
        for s in lineup.seats.iter_mut() {
            if s.as_ref().is_some_and(|p| who.contains(p)) {
                *s = None;
            }
        }
        np.recover.ready = Some((0, u32::MAX, lineup, Vec::new()));
    }
}

/// A restart ready: the host's goes out first (its state now), then every machine takes it on.
pub(super) fn restarted<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G) {
    let Some((round, tick, lineup, data)) = np.recover.ready.take() else { return };
    let (round, tick, data) = if tick == u32::MAX {
        // the host's own: its state before a tick every seat's pads are in for
        let round = np.rb.round.wrapping_add(1);
        let Some((tick, data)) = rollback::confirmed(np, game) else {
            if game.encode().is_none() {
                let gone = if np.recover.taking { np.lineup.host().to_string() } else { np.recover.away.join(", ") };
                let why = match gone.is_empty() {
                    true => "a player left, and this ride cannot hand its state over to go on without them".to_string(),
                    false => format!("{gone} went silent, and this ride cannot hand its state over to go on without them"),
                };
                np.tell(Msg::End(why.clone()));
                return np.finish(why);
            }
            // no confirmed cell this frame: try again with the next
            np.recover.ready = Some((0, u32::MAX, lineup, data));
            return;
        };
        let n = data.len().div_ceil(PIECE).max(1);
        for i in 0..n {
            let piece = data[i * PIECE..((i + 1) * PIECE).min(data.len())].to_vec();
            np.tell(Msg::Restart { round, tick, lineup: lineup.clone(), data: piece, last: i + 1 == n });
        }
        (round, tick, data)
    } else {
        (round, tick, data)
    };
    // leaves (N11): the tick the host's took them at
    for l in np.leave.left.iter_mut().filter(|l| l.2 == u32::MAX) {
        l.2 = tick;
    }
    if lineup.mine(&np.me).is_empty() {
        let why = match (np.leave.leaving(), np.host()) {
            (true, true) => format!("you left the ride: {} hosts it now", lineup.host()),
            (true, false) => "you left the ride".to_string(),
            _ => "the others rode on without this machine".to_string(),
        };
        return np.finish(why);
    }
    if let Err(e) = game.decode(&data) {
        return np.finish(format!("the host's state did not load: {e}"));
    }
    // the modifiers the host changed at this tick (P100), on the state as it was before them
    if let Some((_, was, to)) = np.recover.mods.take().filter(|m| m.0 == round) {
        if let Err(e) = game.remodify(&was, &to) {
            return np.finish(format!("the host's modifiers did not take: {e}"));
        }
        if to != np.mods {
            np.say(format!("online: the modifiers are '{to}' from tick {tick}"));
            np.remods.push((tick, to.clone()));
        }
        np.mods = to;
    }
    let rb = match rollback::Rollback::new(&lineup, &np.me, tick as usize, round) {
        Ok(rb) => rb,
        Err(e) => return np.finish(format!("ggrs: {e}")),
    };
    np.say(format!("online: the ride goes on from tick {tick} (restart {round}) as {}", lineup.text()));
    // a leaver that keeps some seats rides on
    if np.leave.mine.as_ref().is_some_and(|m| m.0.iter().all(|&s| lineup.seats.get(s).is_none_or(|p| p.as_deref() != Some(np.me.as_str())))) {
        np.leave.mine = None;
    }
    if std::mem::take(&mut np.recover.taking) {
        np.recover.takeovers.push((np.lineup.host().to_string(), tick));
    }
    for p in np.lineup.peers() {
        if !lineup.peers().contains(&p) {
            // a leaver still needs this restart to know it is out: its link stays until it acks
            if !np.leave.left.iter().any(|l| l.0 == p && l.2 == tick) {
                np.link.forget(&p);
            }
            np.recover.quiet.remove(&p);
        }
    }
    // a peer new to the ride (N10's join): heard from as the others are, and given the loading
    // time to answer
    for p in lineup.peers() {
        if p != np.me {
            np.recover.quiet.entry(p).or_insert((0.0, false));
        }
    }
    np.rb = rb;
    np.lineup = lineup;
    np.log.truncate(tick as usize);
    np.hashes.truncate(tick as usize);
    np.resync = Default::default();
    np.desynced = false;
    let r = &mut np.recover;
    r.away.retain(|p| np.lineup.peers().contains(p));
    for q in r.quiet.values_mut() {
        q.0 = 0.0;
    }
    (r.told, r.sent, r.waited) = (None, None, 0.0);
    r.restarts += 1;
}
