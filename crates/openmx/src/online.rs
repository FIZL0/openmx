//! The shell online (N2): [`net`]'s session over [`net::udp`], one bike a seat, each a [`Sim`] of
//! its own on the one track.
//!
//! A frame online is: what came in goes to the session, the session runs (ticks, and any rollback's
//! ticks again, on [`Rides`]), what it sends goes out, and this machine's first seat is copied into
//! the shell's first seat — so the camera, the chassis, the overlay and the picture are the offline
//! loop's, reading the seat this machine rides. Every other seat is copied into a shell seat after it
//! (N5, [`Online::drawn`]), which the loop poses and draws as it does a split screen's other players,
//! with no view of their own; a seat nobody sits in (`-`, or a peer that did not come back) is not
//! drawn. The body's own frame is part of the tick (the `after` hook), because the body is simulation
//! state and must be rolled back with the ride; the pose it chose for each rider is written once the
//! session has run.
//!
//! Not here yet: only the first local seat has a pad (P35's seats bring the rest); no Start freeze,
//! machine swap or tape online.
//!
//! From the lobby (N3, [`Online::lobby`]) the session's datagrams go over the lobby's own iroh
//! connection instead of `--peer` UDP: to and from the host direct, guest to guest through the host.
//! A datagram too big for that path goes on the lobby's reliable stream instead, addressed, and every
//! other peer but the one it is for drops it. A peer dropping or the host ending the ride ends it here,
//! and what else the network said on the way is kept for the menu's lobby.
//!
//! A local freeride opened to online players from its pause (N10, [`Online::open`]) is the host's
//! ride of one, its session started at the tick it opened on, hosting a lobby that takes peers on
//! while it rides: each that passes the handshake ([`openmx_net::Event::Arrived`]) is handed to the
//! session, which takes it on at a tick with its seats spawned where the ride started
//! ([`crate::net::admit`]). The peer rides in with [`Online::drop_in`]: no seat until the host's
//! state comes.

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
    /// The frame length of the last tick run.
    dt: F,
    last: Instant,
    /// The hands this machine's seat last rode with.
    pub hands: Controls,
    /// Each seat's pose from the last tick this frame ran, for its body.
    poses: Vec<Option<vehicle_anim::Frame>>,
    /// The ticks run in all, and the notice last printed.
    shown: String,
    /// The host's time-scale modifier (P55), which every machine scales the same ticks by.
    pub time_scale: Option<F>,
    /// A ride opened to online players (N10): a joiner's seats' state as each spawns (one seat in
    /// [`game_sim::snapshot`]'s codec), and the peers handed to the session not taken on yet.
    spawn: Option<Vec<u8>>,
    arrived: Vec<String>,
    /// The room as the network last said it, for the controller preset of a peer that arrives
    /// (N13): its seat's pick says it, and the network seats a joiner before it arrives.
    room: Option<openmx_net::Room>,
}

impl Online {
    /// How many seats `--seats` names, before the session starts: the shell makes a rider for each.
    pub fn seats(seats: Option<&str>) -> Result<usize, String> {
        Ok(Lineup::parse(seats.ok_or("--online wants --seats, like host,g")?)?.seats.len())
    }

    /// `--online`, `--peer`s and `--seats`, seat `s` starting as `starts[s]` (every machine must
    /// hand the same ones).
    pub fn start(me: &str, peers: &[String], seats: Option<&str>, starts: &[Sim]) -> Result<Online, String> {
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
        if starts.len() != lineup.seats.len() {
            return Err(format!("{} seats and {} riders to start them", lineup.seats.len(), starts.len()));
        }
        let net = Netplay::new(lineup.clone(), &me, Timing::default(), crate::net::desync::dumps())?;
        let udp = Udp::bind(at, peers)?;
        println!("online: {me} on {at}, seat {} of {} ({})", seat + 1, lineup.seats.len(), lineup.text());
        Ok(Online {
            net,
            wire: Wire::Udp(udp),
            sims: starts.to_vec(),
            poses: vec![None; crate::net::SEATS],
            seat,
            dt: rides::tick_dt(0),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
            time_scale: None,
            spawn: None,
            arrived: Vec::new(),
            room: None,
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
            poses: vec![None; crate::net::SEATS],
            sims,
            seat,
            dt: rides::tick_dt(0),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
            time_scale: None,
            spawn: None,
            arrived: Vec::new(),
            room: None,
        })
    }

    /// A local freeride opened to online players (N10): the host's ride of one from here on, `sim`
    /// its seat as it is now, over `net`, which hosts the room. `spawn` is a seat where the ride
    /// starts, which every peer that joins rides in from.
    pub fn open(net: Arc<openmx_net::Net>, sim: Sim, spawn: &Sim) -> Result<Online, String> {
        let me = openmx_net::HOST;
        let session = Netplay::new(Lineup { seats: vec![Some(me.to_string())] }, me, Timing::default(), crate::net::desync::dumps())?;
        println!("online: the ride is open to online players");
        Ok(Online {
            net: session,
            wire: Wire::Lobby { net, kept: Vec::new() },
            poses: vec![None; crate::net::SEATS],
            sims: vec![sim],
            seat: 0,
            dt: rides::tick_dt(0),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
            time_scale: None,
            spawn: Some(rides::seat_bytes(spawn)),
            arrived: Vec::new(),
            room: None,
        })
    }

    /// Into a ride under way (N10), as `me` in `room`: no seat until the host takes this machine on
    /// and sends the ride's state. `first` is the host's first seat's rider and bike, whose
    /// constants every seat of the ride is decoded onto ([`Rides`]).
    pub fn drop_in(net: Arc<openmx_net::Net>, room: &openmx_net::Room, me: &str, first: Sim) -> Result<Online, String> {
        let session = Netplay::joining(openmx_net::HOST, me, Timing::default(), crate::net::desync::dumps())?;
        println!("online: dropping in on {}'s ride, waiting for the host", room.name);
        Ok(Online {
            net: session,
            wire: Wire::Lobby { net, kept: Vec::new() },
            poses: vec![None; crate::net::SEATS],
            sims: vec![first],
            seat: 0,
            dt: rides::tick_dt(0),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
            time_scale: None,
            spawn: None,
            arrived: Vec::new(),
            room: None,
        })
    }

    /// Whether this machine has a seat in the ride yet (a drop-in waits for the host's state).
    pub fn seated(&self) -> bool {
        !self.net.waiting()
    }

    /// The riders in the ride now (each machine's seats; a drop-in's grow as peers join).
    pub fn riders(&self) -> usize {
        self.net.lineup.seats.iter().flatten().count()
    }

    /// What the lobby's network said during the ride that was not the ride's, for the menu.
    pub fn kept(&mut self) -> Vec<openmx_net::Event> {
        match &mut self.wire {
            Wire::Lobby { kept, .. } => std::mem::take(kept),
            Wire::Udp(_) => Vec::new(),
        }
    }

    /// The pause's commands for the lobby under the ride (N14, an opened ride's Seats row), to its
    /// network; a ride with no lobby has nobody to tell.
    pub fn command(&self, c: openmx_net::Cmd) {
        if let Wire::Lobby { net, .. } = &self.wire {
            net.send(c);
        }
    }

    /// Leave a lobby's ride from the pause: the host takes everyone back to the lobby, a guest leaves
    /// the lobby (the others go back to theirs). Whether this machine is still in a lobby after.
    pub fn leave(&self) -> bool {
        let Wire::Lobby { net, .. } = &self.wire else { return false };
        // an opened freeride's room was only ever the ride: it closes with it
        if self.spawn.is_some() {
            net.send(openmx_net::Cmd::Leave);
            return false;
        }
        if self.net.me == openmx_net::HOST {
            net.send(openmx_net::Cmd::Lobby);
            true
        } else {
            net.send(openmx_net::Cmd::Leave);
            false
        }
    }

    /// Everything that came in since the last frame, as (peer, datagram); `Err` when the lobby's
    /// network ended the ride, or a `--peer` runs another build.
    fn receive(&mut self) -> Result<Vec<(String, Vec<u8>)>, String> {
        use openmx_net::Event;
        let me = self.net.me.clone();
        let session = &mut self.net;
        let (spawn, arrived, room) = (&self.spawn, &mut self.arrived, &mut self.room);
        match &mut self.wire {
            Wire::Udp(udp) => udp.receive(),
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
                        // a peer into the opened ride: the session takes it on once it hears it
                        // on its own controller preset (N13), each seat its pick's
                        Event::Arrived { ref peer, ref seats } if let Some(spawn) = spawn => {
                            let pick = |s: usize| room.as_ref()?.seats.get(s)?.as_ref().filter(|seat| seat.peer == *peer);
                            let presets: Vec<(usize, u8)> = seats
                                .iter()
                                .map(|&s| match pick(s) {
                                    Some(seat) => (s, seat.pick.preset),
                                    None => {
                                        println!("online: {peer}'s seat {} is not in the room as last heard: preset 0", s + 1);
                                        (s, 0)
                                    }
                                })
                                .collect();
                            match session.admit(peer, seats, rides::spawn_on(spawn, &presets)) {
                                Ok(()) => arrived.push(peer.clone()),
                                Err(why) => println!("online: {peer} cannot join: {why}"),
                            }
                            kept.push(e);
                        }
                        Event::Room(ref r) => {
                            *room = Some(r.clone());
                            // one that left before it was taken on
                            let seated = r.peers();
                            for p in arrived.extract_if(.., |p| !seated.contains(p)) {
                                session.unadmit(&p);
                            }
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
    /// the last tick every machine has every pad for that is a multiple of ten seconds (300 ticks).
    pub fn summary(&self) -> String {
        let settled = self.net.confirmed().max(0) as usize;
        let at = settled / 300 * 300;
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

    /// The frame length of the last tick run online (the 30 Hz cap, once a second of the ride is in).
    pub fn dt(&self) -> F {
        self.dt
    }

    /// The seats in the order the shell holds them: this machine's first, then every other in seat
    /// order. Shell seat `d` is seat `drawn()[d]`.
    pub fn drawn(&self) -> Vec<usize> {
        std::iter::once(self.seat).chain((0..self.sims.len()).filter(|&s| s != self.seat)).collect()
    }

    /// Every seat's player by name, `None` for an empty seat: what the markers' slot display calls
    /// the other players (P88).
    pub fn names(&self) -> Vec<Option<String>> {
        (0..self.sims.len()).map(|s| self.net.lineup.seats.get(s).cloned().flatten()).collect()
    }

    /// How many seats the ride has, and which is this machine's: the markers' `players` and `me`.
    pub fn seat_of(&self) -> (usize, usize) {
        (self.sims.len(), self.seat)
    }

    /// Shell seat `d` after the frame: its state into `sim`, its pose onto `body`. Whether anyone
    /// sits in it — an empty seat's bike rides on with an unplugged pad, and is not drawn.
    pub fn show(&self, d: usize, sim: &mut Sim, body: Option<&mut Body>) -> bool {
        let Some(&seat) = self.drawn().get(d) else { return false };
        sim.clone_from(&self.sims[seat]);
        if let (Some(frame), Some(body)) = (self.poses[seat], body) {
            body.posed_by(frame, &sim.rider.body.player, sim.rider.body_clocked);
        }
        self.net.lineup.seats[seat].is_some()
    }

    /// One frame: `pad` rides this machine's first seat; [`Online::show`] reads every seat after.
    /// `body` is any rider's, for the clip table the body's machine reads. `Err` is why the ride is
    /// over.
    pub fn frame(&mut self, pad: Report, place: &mut dyn World, body: Option<&Body>) -> Result<(), String> {
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
        let (me, mut hands) = (self.seat, None);
        let poses = &mut self.poses;
        poses.iter_mut().for_each(|p| *p = None);
        {
            let lib_set = body.map(|b| (&b.pose_lib, &b.rig.set));
            let mut after = |seat: usize, dt: F, sim: &mut Sim, c: &Controls| {
                if let Some((lib, set)) = lib_set {
                    let frame = sim.rider.body_frame(dt, c, Gate::default(), lib, &|id| set.timing(Some(id)));
                    poses[seat] = Some(frame);
                }
                if seat == me {
                    hands = Some(*c);
                }
            };
            let mut game = Rides { sims: &mut self.sims, world: place, after: Some(&mut after), time_scale: self.time_scale };
            self.net.frame(&mut game, &local, real);
        }
        self.dt = game_sim::modifiers::scaled_dt(rides::tick_dt(self.net.tick().saturating_sub(1)), self.time_scale);
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
        if let Some(c) = hands {
            self.hands = c;
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
