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

pub struct Online {
    net: Netplay<rides::State>,
    udp: Udp,
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
            udp,
            sims: vec![sim.clone(); lineup.seats.len()],
            seat,
            dt: rides::steady_dt(),
            last: Instant::now(),
            hands: Controls::default(),
            shown: String::new(),
        })
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

    /// The frame length every tick online is.
    pub fn dt(&self) -> F {
        self.dt
    }

    /// One frame: `pad` rides this machine's first seat, `sim` gets that seat's state after. `Err`
    /// is why the ride is over.
    pub fn frame(&mut self, pad: Report, place: &mut dyn World, body: Option<&mut Body>, sim: &mut Sim) -> Result<(), String> {
        for (from, d) in self.udp.receive() {
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
            self.udp.send(&to, &d);
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
