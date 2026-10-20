//! The datagrams a session sends, and the session's own messages over them.
//!
//! Every datagram starts with a kind byte:
//!
//! | kind | then |
//! |---|---|
//! | 0 | the ggrs session's round (a restart's number), then ggrs' message as bincode |
//! | 1 | a sequence number (u32 little-endian), then a [`Msg`] as bincode |
//! | 2 | the next sequence number this peer is waiting for: everything before it arrived |
//! | 3 | nothing: a keep-alive ([`super::recover`]) |
//! | 4 | the round, a number (u16 little-endian), the piece's index and the count: a piece of a kind-0 body |
//!
//! ggrs brings its own reliability for its inputs. Its input message carries every input a peer has
//! not acknowledged, for every seat on this machine — with eight seats and a second of them owed, more
//! than [`DATAGRAM_MAX`] — so a bigger one goes in pieces (kind 4), and is whole again only when every
//! piece arrived; a piece lost loses the message, which ggrs sends again with the next frame anyway. The session's messages — a resync's state, the
//! host's word about who is away, a restart — went over a reliable stream in opentennis; here they are
//! numbered per peer, sent again every [`RESEND`] until acknowledged, and handed over in order, so the
//! one transport the caller provides is datagrams that may be lost, late or reordered.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Lineup;

/// A message that is not sent again for this long is sent again (by ggrs' clock, so a test drives it).
pub const RESEND: Duration = Duration::from_millis(200);
/// The most data bytes in one [`Msg`]'s piece of a state; a datagram stays under [`DATAGRAM_MAX`].
pub const PIECE: usize = 1024;
/// The biggest datagram this session sends (a safe UDP payload on any path).
pub const DATAGRAM_MAX: usize = 1200;
/// The most unacknowledged messages sent again in one go, per peer.
const BURST: usize = 16;

/// The session's own messages.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Msg {
    /// The host's state before tick `tick`, a piece at a time ([`super::resync`]).
    State { tick: u32, data: Vec<u8>, last: bool },
    /// A peer took the host's state and is in sync from this tick.
    Resynced(u32),
    /// The host: these peers are away, their seats emptied in `left` seconds; `who` empty: all back.
    Away { who: Vec<String>, left: u32 },
    /// The host: the ride goes on from the state before `tick` with this line-up, under session
    /// `round` ([`super::recover`]), a piece at a time.
    Restart { round: u8, tick: u32, lineup: Lineup, data: Vec<u8>, last: bool },
    /// The host: the session is over, with why.
    End(String),
}

pub enum Kind<'a> {
    Ggrs(u8, &'a [u8]),
    Reliable(u32, &'a [u8]),
    Ack(u32),
    Alive,
    Piece { round: u8, id: u16, index: u8, count: u8, body: &'a [u8] },
}

/// A datagram's kind and body.
pub fn open(d: &[u8]) -> Option<Kind<'_>> {
    let word = |b: &[u8]| b.get(..4).map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]));
    match *d.first()? {
        0 => Some(Kind::Ggrs(*d.get(1)?, &d[2..])),
        1 => Some(Kind::Reliable(word(&d[1..])?, &d[5..])),
        2 => Some(Kind::Ack(word(&d[1..])?)),
        3 => Some(Kind::Alive),
        4 => Some(Kind::Piece {
            round: *d.get(1)?,
            id: u16::from_le_bytes([*d.get(2)?, *d.get(3)?]),
            index: *d.get(4)?,
            count: *d.get(5)?,
            body: &d[6..],
        }),
        _ => None,
    }
}

/// A ggrs message: one datagram, or its pieces numbered `id` when it is bigger than [`DATAGRAM_MAX`].
pub fn ggrs(round: u8, id: u16, body: &[u8]) -> Vec<Vec<u8>> {
    if body.len() + 2 <= DATAGRAM_MAX {
        return vec![[&[0, round][..], body].concat()];
    }
    let pieces: Vec<&[u8]> = body.chunks(DATAGRAM_MAX - 6).collect();
    let count = pieces.len() as u8;
    let id = id.to_le_bytes();
    pieces.iter().enumerate().map(|(i, b)| [&[4, round, id[0], id[1], i as u8, count][..], b].concat()).collect()
}

/// A ggrs message's pieces as they come in, per peer: the one being put together.
#[derive(Default)]
pub struct Pieces {
    peers: BTreeMap<String, (u16, Vec<Option<Vec<u8>>>)>,
}

impl Pieces {
    /// A piece: the whole message when it was the last one missing. A piece of a newer message drops
    /// what there was of an older one.
    pub fn heard(&mut self, from: &str, id: u16, index: u8, count: u8, body: &[u8]) -> Option<Vec<u8>> {
        let (at, parts) = self.peers.entry(from.to_string()).or_insert((id, Vec::new()));
        if *at != id || parts.len() != count as usize {
            if id.wrapping_sub(*at) > u16::MAX / 2 && !parts.is_empty() {
                return None; // an older message's piece, late
            }
            (*at, *parts) = (id, vec![None; count as usize]);
        }
        *parts.get_mut(index as usize)? = Some(body.to_vec());
        if parts.iter().any(Option::is_none) {
            return None;
        }
        let whole = parts.iter_mut().flat_map(|p| p.take().unwrap()).collect();
        parts.clear();
        Some(whole)
    }
}

pub fn alive() -> Vec<u8> {
    vec![3]
}

fn reliable(seq: u32, body: &[u8]) -> Vec<u8> {
    [&[1][..], &seq.to_le_bytes(), body].concat()
}

fn ack(next: u32) -> Vec<u8> {
    [&[2][..], &next.to_le_bytes()].concat()
}

/// One peer's two directions.
#[derive(Default)]
struct Chan {
    /// The next number to give a message, the ones not acknowledged yet, and when they last went.
    next_out: u32,
    unacked: VecDeque<(u32, Vec<u8>, bool)>,
    sent: Option<ggrs::clock::Instant>,
    /// The next number to hand over, the ones that came early, and whether an ack is owed.
    next_in: u32,
    early: BTreeMap<u32, Msg>,
    owe: bool,
}

/// The session's messages to and from every peer.
#[derive(Default)]
pub struct Link {
    peers: BTreeMap<String, Chan>,
    /// Messages sent again, in all.
    pub resent: u64,
}

impl Link {
    pub fn send(&mut self, to: &str, m: Msg) {
        let c = self.peers.entry(to.to_string()).or_default();
        let body = bincode::serialize(&m).expect("a message");
        c.unacked.push_back((c.next_out, body, false));
        c.next_out += 1;
    }

    /// A message numbered `seq` from `from`: the messages now due, in order.
    pub fn heard(&mut self, from: &str, seq: u32, body: &[u8]) -> Vec<Msg> {
        let c = self.peers.entry(from.to_string()).or_default();
        c.owe = true;
        if seq >= c.next_in {
            if let Ok(m) = bincode::deserialize::<Msg>(body) {
                c.early.insert(seq, m);
            }
        }
        let mut due = Vec::new();
        while let Some(m) = c.early.remove(&c.next_in) {
            due.push(m);
            c.next_in += 1;
        }
        due
    }

    /// `from` has every message before `next`.
    pub fn acked(&mut self, from: &str, next: u32) {
        if let Some(c) = self.peers.get_mut(from) {
            while c.unacked.front().is_some_and(|m| m.0 < next) {
                c.unacked.pop_front();
            }
        }
    }

    /// A peer that is not in the ride any more: nothing more to it.
    pub fn forget(&mut self, peer: &str) {
        self.peers.remove(peer);
    }

    /// What goes out now: new messages once, the unacknowledged ones again every [`RESEND`], acks.
    pub fn outgoing(&mut self) -> Vec<(String, Vec<u8>)> {
        let now = ggrs::clock::now();
        let mut out = Vec::new();
        for (p, c) in &mut self.peers {
            if std::mem::take(&mut c.owe) {
                out.push((p.clone(), ack(c.next_in)));
            }
            let again = c.sent.is_some_and(|t| now - t >= RESEND);
            let mut any = false;
            for (seq, body, gone) in c.unacked.iter_mut().take(BURST) {
                if !*gone || again {
                    self.resent += *gone as u64;
                    *gone = true;
                    any = true;
                    out.push((p.clone(), reliable(*seq, body)));
                }
            }
            if any {
                c.sent = Some(now);
            }
        }
        out
    }
}
