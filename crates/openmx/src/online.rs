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
//! Each seat's bike has its put-back ghost (P57's [`Ghost`], P59): the shell's [`crate::vehicle::Garage`]
//! ticks it offline, and online the seats are stepped here instead, so [`Seats`] ticks one a seat
//! beside the ride and carries it through the rollback state. A put-back on any machine is the same
//! ghost, at the same alpha on the same tick, on every machine.
//!
//! Not here yet: only the first local seat has a pad (P35's seats bring the rest); no Start freeze,
//! machine swap or tape online.
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
use crate::net::{Game, Lineup, Netplay, Timing};
use crate::vehicle::Ghost;

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

/// The online ride's rollback state: every seat's [`Sim`], and every seat's bike's ghost.
pub type State = (rides::State, Arc<Vec<Ghost>>);

/// The ride online with each seat's put-back ghost (P59): [`Rides`], and beside each seat the
/// [`Ghost`] that [`crate::vehicle::Garage::step`] runs offline, in the same places of the seat's
/// tick — its clock at the head, a put-back in the tick starts it again. Saved, restored, hashed and
/// said in bytes with the ride, so a rollback's re-run puts the ghost back where the ride is, a
/// desync in it is caught, and a resync or a restart hands it over.
pub struct Seats<'a> {
    pub rides: Rides<'a>,
    /// One a seat, in seat order.
    pub ghosts: &'a mut Vec<Ghost>,
}

/// A ghost in bytes: its clock's bits, its alpha's, its flag.
const GHOST_BYTES: usize = 9;

impl Game for Seats<'_> {
    type State = State;

    fn save(&self) -> State {
        (self.rides.save(), Arc::new(self.ghosts.clone()))
    }

    fn restore(&mut self, s: &State) {
        self.rides.restore(&s.0);
        self.ghosts.clone_from(&*s.1);
    }

    fn step(&mut self, tick: usize, pads: &[Report]) {
        let dt = rides::tick_dt(tick);
        for seat in 0..self.ghosts.len() {
            self.ghosts[seat].step(dt);
            let said = self.rides.seat(seat, dt, pads.get(seat).copied().unwrap_or(Report::missing()));
            if said.respawned {
                self.ghosts[seat].start();
            }
        }
    }

    fn hash(&self) -> u64 {
        self.ghosts.iter().fold(self.rides.hash(), |h, g| game_sim::state_hash(&ghost_bytes(g), h))
    }

    /// [`Rides`]' bytes, then each seat's ghost.
    fn encode(&self) -> Option<Vec<u8>> {
        let mut out = self.rides.encode()?;
        out.extend(self.ghosts.iter().flat_map(ghost_bytes));
        Some(out)
    }

    /// [`Seats::encode`]'s bytes back; nothing is written unless the ride and every ghost read.
    fn decode(&mut self, bytes: &[u8]) -> Result<(), String> {
        let n = self.ghosts.len() * GHOST_BYTES;
        let split = bytes.len().checked_sub(n).ok_or("the state ends before its ghosts")?;
        let (ride, tail) = bytes.split_at(split);
        let ghosts = tail
            .chunks(GHOST_BYTES)
            .map(|b| {
                let word = |at: usize| F(f32::from_bits(u32::from_le_bytes(b[at..at + 4].try_into().unwrap())));
                let on = match b[8] {
                    0 => false,
                    1 => true,
                    x => return Err(format!("a ghost's flag is {x}")),
                };
                Ok(Ghost { clock: word(0), alpha: word(4), on })
            })
            .collect::<Result<Vec<_>, String>>()?;
        self.rides.decode(ride)?;
        *self.ghosts = ghosts;
        Ok(())
    }
}

fn ghost_bytes(g: &Ghost) -> [u8; GHOST_BYTES] {
    let mut b = [0; GHOST_BYTES];
    b[..4].copy_from_slice(&g.clock.0.to_bits().to_le_bytes());
    b[4..8].copy_from_slice(&g.alpha.0.to_bits().to_le_bytes());
    b[8] = g.on as u8;
    b
}

pub struct Online {
    net: Netplay<State>,
    wire: Wire,
    sims: Vec<Sim>,
    /// Each seat's bike's put-back ghost (P59), beside its [`Sim`].
    ghosts: Vec<Ghost>,
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
            ghosts: vec![Ghost::default(); starts.len()],
            poses: vec![None; lineup.seats.len()],
            seat,
            dt: rides::tick_dt(0),
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
            poses: vec![None; sims.len()],
            ghosts: vec![Ghost::default(); sims.len()],
            sims,
            seat,
            dt: rides::tick_dt(0),
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

    /// Shell seat `d`'s bike's put-back ghost after the frame, which the shell draws it by.
    pub fn ghost(&self, d: usize) -> Ghost {
        self.drawn().get(d).map_or(Ghost::default(), |&seat| self.ghosts[seat])
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
            let rides = Rides { sims: &mut self.sims, world: place, after: Some(&mut after) };
            let mut game = Seats { rides, ghosts: &mut self.ghosts };
            self.net.frame(&mut game, &local, real);
        }
        self.dt = rides::tick_dt(self.net.tick().saturating_sub(1));
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
