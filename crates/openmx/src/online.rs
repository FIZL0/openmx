//! The shell online (N2): [`net`]'s session over [`net::udp`], one bike a seat, each a [`Sim`] of
//! its own on the one track.
//!
//! A frame online is: what came in goes to the session, the session runs (ticks, and any rollback's
//! ticks again, on [`Rides`]), what it sends goes out, and this machine's first seat is copied into
//! the shell's own `world.sim` — so the camera, the chassis, the overlay and the picture are the
//! offline loop's, reading the seat this machine rides. The body's own frame is part of the tick
//! (the `after` hook), because the body is simulation state and must be rolled back with the ride;
//! the pose it chose for this machine's rider is written once the session has run.
//!
//! Not here yet: the other seats' bikes are ridden and kept in sync but not drawn; only the first
//! local seat has a pad (P35's seats bring the rest); no Start freeze, machine swap or tape online.
//!
//! From the lobby (N3, [`Online::lobby`]) the session's datagrams go over the lobby's own iroh
//! connection instead of `--peer` UDP: to and from the host direct, guest to guest through the host.
//! A datagram too big for that path goes on the lobby's reliable stream instead, addressed, and every
//! other peer but the one it is for drops it. A peer dropping or the host ending the ride ends it here,
//! and what else the network said on the way is kept for the menu's lobby.

use std::sync::Arc;
use std::time::Instant;

use game_render::rider::Body;
use game_sim::input::Report;
use game_sim::ride::Controls;
use game_sim::rider::Gate;
use game_sim::vehicle_anim;
use game_sim::world::World;
use game_sim::Sim;
use ps2_float::F;

use crate::net::rides::{self, Rides};
use crate::net::udp::{self, Udp};
use crate::net::{Lineup, Netplay, Timing};

/// The biggest session datagram sent as a datagram over the lobby: iroh's QUIC path at its smallest
/// MTU carries a little over 1100 bytes, less the peer's name the host relays it under.
pub const LOBBY_DATAGRAM: usize = 1000;

/// Where the session's datagrams go.
enum Wire {
    /// `--peer`s over plain UDP (N2).
    Udp(Udp),
    /// The lobby's connection (N3), and what it said that was not the ride's.
    Lobby { net: Arc<openmx_net::Net>, kept: Vec<openmx_net::Event> },
}

pub struct Online {
    net: Netplay<rides::State>,
    wire: Wire,
    sims: Vec<Sim>,
    /// This machine's first seat: the one the shell draws and its pad rides.
    seat: usize,
    dt: F,
    last: Instant,
    /// The hands this machine's seat last rode with.
    pub hands: Controls,
    /// The ticks run in all, and the notice last printed.
    shown: String,
}

impl Online {
    /// `--online`, `--peer`s and `--seats`, every seat starting as `sim`.
    pub fn start(me: &str, peers: &[String], seats: Option<&str>, sim: &Sim) -> Result<Online, String> {
        let (me, at) = udp::named(me)?;
        let peers = peers.iter().map(|p| udp::named(p)).collect::<Result<Vec<_>, _>>()?;
        let seats = seats.ok_or("--online wants --seats, like host,g")?;
        let lineup = Lineup::parse(seats)?;
        for p in lineup.peers() {
            if p != me && !peers.iter().any(|q| q.0 == p) {
                return Err(format!("--seats names {p}, and no --peer says where it is"));
            }
        }
        let seat = *lineup.mine(&me).first().ok_or(format!("--seats gives {me} no seat"))?;
        let net = Netplay::new(lineup.clone(), &me, Timing::default(), crate::net::desync::dumps())?;
        let udp = Udp::bind(at, peers)?;
        println!("online: {me} on {at}, seat {} of {} ({})", seat + 1, lineup.seats.len(), lineup.text());
        Ok(Online {
            net,
            wire: Wire::Udp(udp),
            sims: vec![sim.clone(); lineup.seats.len()],
            seat,
            dt: rides::steady_dt(),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
        })
    }

    /// The lobby's ride (N3): its line-up, this machine as `me` in it, and a [`Sim`] a seat, each on
    /// its seat's bike, over the lobby's connection.
    pub fn lobby(net: Arc<openmx_net::Net>, room: &openmx_net::Room, me: &str, sims: Vec<Sim>) -> Result<Online, String> {
        let lineup = Lineup { seats: room.seats.iter().map(|s| s.as_ref().map(|s| s.peer.clone())).collect() };
        if sims.len() != lineup.seats.len() {
            return Err(format!("{} seats and {} rides", lineup.seats.len(), sims.len()));
        }
        let seat = *lineup.mine(me).first().ok_or("the lobby's line-up gives this machine no seat")?;
        let session = Netplay::new(lineup.clone(), me, Timing::default(), crate::net::desync::dumps())?;
        println!("online: lobby ride, seat {} of {}, seed {:#010x}", seat + 1, lineup.seats.len(), room.seed);
        Ok(Online {
            net: session,
            wire: Wire::Lobby { net, kept: Vec::new() },
            sims,
            seat,
            dt: rides::steady_dt(),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
        })
    }

    /// What the lobby's network said during the ride that was not the ride's, for the menu.
    pub fn kept(&mut self) -> Vec<openmx_net::Event> {
        match &mut self.wire {
            Wire::Lobby { kept, .. } => std::mem::take(kept),
            Wire::Udp(_) => Vec::new(),
        }
    }

    /// Leave a lobby's ride from the pause: the host takes everyone back to the lobby, a guest leaves
    /// the lobby (the others go back to theirs). Whether this machine is still in a lobby after.
    pub fn leave(&self) -> bool {
        let Wire::Lobby { net, .. } = &self.wire else { return false };
        if self.net.me == openmx_net::HOST {
            net.send(openmx_net::Cmd::Lobby);
            true
        } else {
            net.send(openmx_net::Cmd::Leave);
            false
        }
    }

    /// Everything that came in since the last frame, as (peer, datagram); `Err` when the lobby's
    /// network ended the ride.
    fn receive(&mut self) -> Result<Vec<(String, Vec<u8>)>, String> {
        use openmx_net::Event;
        let me = self.net.me.clone();
        match &mut self.wire {
            Wire::Udp(udp) => Ok(udp.receive()),
            Wire::Lobby { net, kept } => {
                let mut got = Vec::new();
                let mut end = None;
                for e in net.poll() {
                    match e {
                        Event::Datagram(from, d) => got.push((from, d)),
                        Event::Ride(from, d) => {
                            if let Some((to, data)) = unaddressed(&d)
                                && to == me
                            {
                                got.push((from, data.to_vec()));
                            }
                        }
                        Event::Dropped(ref why) | Event::Failed(ref why) => {
                            end = Some(why.clone());
                            kept.push(e);
                        }
                        Event::Room(ref r) if !r.riding => {
                            end.get_or_insert_with(|| "the host ended the ride".to_string());
                            kept.push(e);
                        }
                        e => kept.push(e),
                    }
                }
                match end {
                    Some(why) => Err(why),
                    None => Ok(got),
                }
            }
        }
    }

    fn send(&self, to: &str, d: &[u8]) {
        match &self.wire {
            Wire::Udp(udp) => udp.send(to, d),
            Wire::Lobby { net, .. } if d.len() <= LOBBY_DATAGRAM => net.send(openmx_net::Cmd::Datagram(to.to_string(), d.to_vec())),
            Wire::Lobby { net, .. } => net.send(openmx_net::Cmd::Send(addressed(to, d))),
        }
    }

    /// How far the ride went, and a hash to hold up against the other machines': the state before
    /// the last tick every machine has every pad for that is a multiple of ten seconds.
    pub fn summary(&self) -> String {
        let settled = self.net.confirmed().max(0) as usize;
        let at = settled / 600 * 600;
        let hash = match self.net.hashes.get(at) {
            Some(h) if at > 0 => format!(", state before tick {at} {h:#018x}"),
            _ => String::new(),
        };
        format!("online: {} ticks, {settled} settled, {} desyncs{hash}", self.net.tick(), self.net.desyncs.len())
    }

    /// The hash of the state before each tick every machine has every pad for, from the first, and
    /// how many desyncs were caught: what two machines' rides are held against each other by.
    pub fn settled(&self) -> (&[u64], usize) {
        let n = (self.net.confirmed().max(0) as usize).min(self.net.hashes.len());
        (&self.net.hashes[..n], self.net.desyncs.len())
    }

    /// The frame length every tick online is.
    pub fn dt(&self) -> F {
        self.dt
    }

    /// One frame: `pad` rides this machine's first seat, `sim` gets that seat's state after. `Err`
    /// is why the ride is over.
    pub fn frame(&mut self, pad: Report, place: &mut dyn World, body: Option<&mut Body>, sim: &mut Sim) -> Result<(), String> {
        let came = match self.receive() {
            Ok(came) => came,
            Err(why) => {
                self.net.finish(why.clone());
                return Err(format!("online: {why}"));
            }
        };
        for (from, d) in came {
            self.net.heard(&from, &d);
        }
        let mine = self.net.lineup.mine(&self.net.me);
        let local: Vec<Report> = mine.iter().map(|&s| if s == self.seat { pad } else { Report::idle() }).collect();
        let real = std::mem::replace(&mut self.last, Instant::now()).elapsed().as_secs_f32();
        let (me, mut pose, mut hands) = (self.seat, None::<vehicle_anim::Frame>, None);
        {
            let lib_set = body.as_ref().map(|b| (&b.pose_lib, &b.rig.set));
            let mut after = |seat: usize, dt: F, sim: &mut Sim, c: &Controls| {
                if let Some((lib, set)) = lib_set {
                    let frame = sim.rider.body_frame(dt, c, Gate::default(), lib, &|id| set.timing(Some(id)));
                    if seat == me {
                        pose = Some(frame);
                    }
                }
                if seat == me {
                    hands = Some(*c);
                }
            };
            let mut game = Rides { sims: &mut self.sims, world: place, dt: self.dt, after: Some(&mut after) };
            self.net.frame(&mut game, &local, real);
        }
        for (to, d) in self.net.outgoing() {
            self.send(&to, &d);
        }
        for line in self.net.said.drain(..) {
            println!("{line}");
        }
        if self.net.notice != self.shown {
            self.shown = self.net.notice.clone();
            if !self.shown.is_empty() {
                println!("online: {}", self.shown);
            }
        }
        if let Some(end) = &self.net.end {
            return Err(format!("online: {end}"));
        }
        // the seat may have moved in a restart
        if let Some(&s) = self.net.lineup.mine(&self.net.me).first() {
            self.seat = s;
        }
        sim.clone_from(&self.sims[self.seat]);
        if let Some(c) = hands {
            self.hands = c;
        }
        if let (Some(frame), Some(body)) = (pose, body) {
            body.posed_by(frame, &sim.rider.body.player, sim.rider.body_clocked);
        }
        Ok(())
    }
}

/// A datagram on the lobby's reliable stream: who it is for, then it.
pub fn addressed(to: &str, d: &[u8]) -> Vec<u8> {
    [&[to.len() as u8][..], to.as_bytes(), d].concat()
}

pub fn unaddressed(d: &[u8]) -> Option<(String, &[u8])> {
    let (&n, rest) = d.split_first()?;
    let (to, data) = rest.split_at_checked(n as usize)?;
    Some((String::from_utf8(to.to_vec()).ok()?, data))
}
