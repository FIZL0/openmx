//! **One machine riding online over iroh** (N4): N2's session ([`Netplay`]) on the ride, its datagrams
//! carried by N3's transport ([`openmx_net::Net`]) — the lobby's own connection to the host, which
//! relays between guests. The same code on the desktop and in a browser, where the endpoint is
//! relay-only (`openmx_net::net::endpoint`); that is what lets a page ride against desktops.
//!
//! The ride is N2's test ride: [`Rides`], one [`Sim`] a seat over a flat floor, every seat starting
//! from the host's seed. The browser's build does not run the desktop's frame loop (`run` in
//! `main.rs`), so it has no track from the disc yet; what this proves is that the simulation, the
//! session and the transport agree across the two builds, tick by tick.
//!
//! A frame is: what the transport brought goes to the session (a Start makes one), the session runs
//! its ticks on this machine's pads, and what it sends goes back to the transport.
//!
//! A ride opened to drop-ins (N10): [`Peer::open`] hosts one already under way, alone, and takes
//! each peer that arrives into its session; a peer whose join lands in it ([`Event::DropIn`]) waits
//! for the host's state, as the shell's [`crate::online::Online`] does.

use game_sim::input::Report;
use game_sim::world::Flat;
use game_sim::Sim;
use openmx_net::{Cmd, Event, Net, Room};

use crate::net::rides::{self, Rides};
use crate::net::{Lineup, Netplay, Timing};

/// A machine's ride once the host's Start came.
pub struct Riding {
    pub np: Netplay<rides::State>,
    pub sims: Vec<Sim>,
    world: Flat,
    /// The room as the ride started (its seed, its seats).
    pub room: Room,
}

pub struct Peer {
    pub net: Net,
    /// The last status line, the host's ticket, the room as it is now.
    pub status: String,
    pub ticket: String,
    pub room: Option<Room>,
    pub riding: Option<Riding>,
    /// Why this machine is out of the lobby or the ride (refused, the host left, a peer dropped).
    pub failed: Option<String>,
    /// Datagrams the transport could not carry whole (longer than [`openmx_net::net::DATAGRAM_MAX`]):
    /// counted, and sent on the lobby's reliable stream instead, addressed, as the shell's
    /// `online.rs` sends them (a ride's state for a peer that joins it, N10, is pieces that big).
    pub too_big: u32,
    /// Where a desync's report goes.
    pub dumps: std::path::PathBuf,
    /// The session's log lines not printed yet (the caller prints them).
    pub said: Vec<String>,
    /// Hosting a ride opened to drop-ins (N10): the state a joiner's seats spawn in.
    spawn: Option<Vec<u8>>,
}

/// The seats as the session names them: each seat's peer, in seat order.
pub fn lineup(room: &Room) -> Lineup {
    Lineup { seats: room.seats.iter().map(|s| s.as_ref().map(|s| s.peer.clone())).collect() }
}

/// Every seat's bike at the start, from the host's seed: the same on every machine.
pub fn sims(room: &Room) -> Vec<Sim> {
    let first = Sim::new(room.seed);
    (0..room.seats.len())
        .map(|s| {
            let mut sim = first.clone();
            sim.rng = room.seed.wrapping_add(1 + s as u32);
            sim
        })
        .collect()
}

/// A pad nobody holds: seat `seat`'s at tick `tick` — throttle at a pressure that changes, the stick
/// swinging, a trick chord now and then, as the original's bytes (N2's tests ride the same one). The
/// browser peer and the desktop's test ride on it.
pub fn mash(seat: usize, tick: usize) -> Report {
    let phase = (tick / 20 + seat * 7) % 11;
    let mut buttons = 0x4000; // Cross
    if phase == 3 {
        buttons |= 0x2000 | 0x0010; // Circle, d-pad up
    }
    if phase == 7 {
        buttons = 0x8000; // Square
    }
    let mut r = Report::from_buttons(buttons);
    r.0[14] = (40 + 23 * phase) as u8; // Cross's pressure
    r.set_axis(2, ((tick * (seat + 3) * 5) % 256) as u8);
    r.set_axis(3, (0x80 + ((tick / 7 + seat) % 9) * 9) as u8);
    r
}

impl Peer {
    pub fn new(net: Net) -> Peer {
        Peer { net, status: String::new(), ticket: String::new(), room: None, riding: None, failed: None, too_big: 0, dumps: crate::net::desync::dumps(), said: Vec::new(), spawn: None }
    }

    /// Host `room` as a ride under way that takes peers on (N10): this machine rides its first
    /// seat alone from now, and every peer that joins rides in on a seat spawned where the ride
    /// started.
    pub fn open(&mut self, mut room: Room) -> Result<(), String> {
        (room.riding, room.drop_in) = (true, true);
        let np = Netplay::new(Lineup { seats: vec![Some(openmx_net::HOST.into())] }, openmx_net::HOST, Timing::default(), self.dumps.clone())?;
        let first = sims(&room).swap_remove(0);
        self.spawn = Some(game_sim::snapshot::encode(&first));
        self.riding = Some(Riding { np, sims: vec![first], world: Flat::default(), room: room.clone() });
        self.net.send(Cmd::Host(room));
        Ok(())
    }

    /// One tick of real time, `dt` seconds (call it on the ride's 30 Hz schedule, every
    /// `pace::TICK`, as the shell does): the transport's news, then the ride's ticks on `pad(seat, tick)`
    /// for this machine's seats.
    pub fn frame(&mut self, pad: &dyn Fn(usize, usize) -> Report, dt: f32) {
        for e in self.net.poll() {
            self.heard(e);
        }
        let Some(r) = &mut self.riding else { return };
        let np = &mut r.np;
        let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| pad(s, np.tick())).collect();
        let mut game = Rides { sims: &mut r.sims, world: &mut r.world, after: None, time_scale: None };
        np.frame(&mut game, &local, dt);
        self.said.append(&mut np.said);
        for (to, d) in np.outgoing() {
            if d.len() > openmx_net::net::DATAGRAM_MAX {
                self.too_big += 1;
                self.net.send(Cmd::Send([&[to.len() as u8][..], to.as_bytes(), &d].concat()));
                continue;
            }
            self.net.send(Cmd::Datagram(to, d));
        }
        if let Some(end) = &np.end
            && self.failed.is_none()
        {
            self.failed = Some(end.clone());
        }
    }

    fn heard(&mut self, e: Event) {
        match e {
            Event::Status(s) => self.status = s,
            Event::Code(t) => self.ticket = t,
            Event::Room(r) => self.room = Some(r),
            Event::Start(room, me) => {
                let np = match Netplay::new(lineup(&room), &me, Timing::default(), self.dumps.clone()) {
                    Ok(np) => np,
                    Err(e) => {
                        self.failed = Some(e);
                        return;
                    }
                };
                self.riding = Some(Riding { np, sims: sims(&room), world: Flat::default(), room });
            }
            Event::DropIn(room, me) => {
                let np = match Netplay::joining(openmx_net::HOST, &me, Timing::default(), self.dumps.clone()) {
                    Ok(np) => np,
                    Err(e) => {
                        self.failed = Some(e);
                        return;
                    }
                };
                self.riding = Some(Riding { np, sims: vec![sims(&room).swap_remove(0)], world: Flat::default(), room });
            }
            Event::Arrived { peer, seats } => {
                if let (Some(r), Some(spawn)) = (&mut self.riding, &self.spawn)
                    && let Err(e) = r.np.admit(&peer, &seats, spawn.clone())
                {
                    self.said.push(format!("online: {peer} cannot join: {e}"));
                }
            }
            Event::Datagram(from, d) => {
                if let Some(r) = &mut self.riding {
                    r.np.heard(&from, &d);
                }
            }
            // a datagram too big for one, on the stream: for the peer it names
            Event::Ride(from, d) => {
                if let Some(r) = &mut self.riding
                    && let Some((&n, rest)) = d.split_first()
                    && let Some((to, data)) = rest.split_at_checked(n as usize)
                    && to == r.np.me.as_bytes()
                {
                    r.np.heard(&from, data);
                }
            }
            Event::Dropped(who) => self.failed = Some(format!("{who} dropped")),
            Event::Failed(why) => self.failed = Some(why),
            _ => {}
        }
    }
}

/// Ticks `from..to` as `np` ran them, one line each, `<hash> <pads>`: the state's hash before the
/// tick, and the FNV-1a of every seat's pad bytes the tick ran on, both 16 hex digits. Two machines'
/// lines for a settled tick are the same line when they rode the same ride.
pub fn ticks<S: Clone + 'static>(np: &Netplay<S>, from: usize, to: usize) -> String {
    let to = to.min(np.hashes.len()).min(np.log.len());
    let mut out = String::new();
    for t in from.min(to)..to {
        let pads = np.log[t].iter().flat_map(|p| p.0).fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
        out.push_str(&format!("{:016x} {pads:016x}\n", np.hashes[t]));
    }
    out
}
