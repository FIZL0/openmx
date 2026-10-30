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
//! A seat's machine switch (N17) is its pad's [`crate::net::SWITCH`] byte, and its plane its `Sim`'s
//! own lane ([`crate::vehicle::Wanted`]): copied into the shell's seat like the rest of it, which the
//! shell's garage follows for the picture.
//!
//! Each of this machine's seats rides its own local player's pad ([`Online::frame_seats`]), and a
//! player on it can take a seat or give theirs up as the ride runs (N20, [`Online::sit`] and
//! [`Online::leave_session`]): the session's join and leave restarts. Not here yet: no Start freeze
//! or tape online.
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
use crate::clock::Instant;

use game_render::rider::Body;
use game_sim::input::Report;
use game_sim::ride::Controls;
use game_sim::rider::Gate;
use game_sim::vehicle_anim;
use game_sim::world::World;
use game_sim::Sim;
use game_sim::flight::Flight;
use ps2_float::F;

use crate::net::rides::{self, Modded, Rides};
use crate::vehicle::{self, Spawn};
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
    /// (N13): its seat's pick says it, and the network seats a joiner before it arrives. And every
    /// seat's look (N16, [`Online::seat_pick`]): a lobby ride's and a drop-in's start on the room
    /// they were started from.
    room: Option<openmx_net::Room>,
    /// `--peer`s (N15): every seat's controller preset is not known yet, and the first tick waits on it.
    presets_owed: bool,
    /// The game modifiers every seat rides (P55, 1P's for all), and each seat's bike file before
    /// them, which a change the host makes mid-ride builds the bikes again from (P100).
    mods: game_sim::modifiers::Mods,
    stock: Vec<game_sim::phy::Phy>,
    /// Each seat rides its own player's modifiers (P134, `rides::ride_set`): a ride from the lobby,
    /// whose line-up is the room's from its first tick. A ride opened to online and `--peer`s ride
    /// 1P's for every seat ([`first_players`]): a seat a join adds is built on the first seat's
    /// bike, which every machine moves onto a set only when the set changes.
    own_rows: bool,
    /// The ride's biplane as built (N17, [`rides::plane_on`]), which a seat's switch takes a copy
    /// of, and the disc's `biplane.phy` before the modifiers, which a change rebuilds it from.
    plane: Option<Flight>,
    stock_plane: Option<game_sim::phy::Plane>,
    /// Each seat's own bay in the ride's lot (N18), where a seat that starts in the plane stands.
    bays: Vec<Spawn>,
    /// `--peer`s (P105): every machine's saved markers, which the session waits on.
    marks: Option<Marks>,
    /// The host left and the lobby handed its room to this machine (N11): it hosts it from here. A
    /// peer that dropped in keeps a seat where the ride started (its first seat as it came in) for the
    /// joiners it takes on if that happens.
    handed: bool,
    spare: Option<Vec<u8>>,
}

/// `--peer`s' saved markers (P105): the track and bike they are read back for, whether every
/// peer's are still to come, and when this machine last offered its own.
struct Marks {
    track: String,
    bike: String,
    owed: bool,
    offered: Option<Instant>,
}

/// How often a `--peer` machine offers its saved markers again until its ride ticks.
const OFFER_EVERY: std::time::Duration = std::time::Duration::from_millis(100);

impl Online {
    /// How many seats `--seats` names, before the session starts: the shell makes a rider for each.
    pub fn seats(seats: Option<&str>) -> Result<usize, String> {
        Ok(Lineup::parse(seats.ok_or("--online wants --seats, like host,g")?)?.seats.len())
    }

    /// `--online`, `--peer`s and `--seats`, seat `s` starting as `starts[s]` (every machine must
    /// hand the same ones), this machine's first seat on controller preset `preset` and any other
    /// of its seats (no pad) on 0 — each machine's own, which its datagrams tell the others (N15).
    /// That first seat starts in the biplane if `plane` (N18, `--vehicle biplane`), told the same way.
    pub fn start(me: &str, peers: &[String], seats: Option<&str>, starts: &[Sim], preset: u8, plane: bool) -> Result<Online, String> {
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
        let card: Vec<u8> = (0..lineup.mine(&me).len()).map(|i| if i == 0 { preset | if plane { udp::PLANE } else { 0 } } else { 0 }).collect();
        let udp = Udp::bind(at, peers, &card)?;
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
            presets_owed: true,
            mods: Default::default(),
            stock: Vec::new(),
            own_rows: false,
            plane: None,
            stock_plane: None,
            bays: Vec::new(),
            marks: Some(Marks { track: String::new(), bike: String::new(), owed: true, offered: None }),
            handed: false,
            spare: None,
        })
    }

    /// `--peer`s (N15): every seat's table on its machine's controller preset, once every peer has
    /// said its own — before the first tick, which ggrs runs only once it has heard every peer.
    fn take_presets(&mut self) -> Result<(), String> {
        let Wire::Udp(udp) = &self.wire else { return Ok(()) };
        if !self.presets_owed {
            return Ok(());
        }
        // and each seat's starting machine (N18), which every machine has by the same tick
        let heard = udp.presets(&self.net.lineup, &self.net.me)?.zip(udp.planes(&self.net.lineup, &self.net.me)?);
        if let Some((presets, planes)) = heard {
            for (sim, &p) in self.sims.iter_mut().zip(&presets) {
                sim.set_presets([p as usize, 0]);
            }
            if presets.iter().any(|&p| p != 0) {
                println!("online: controller presets {}", presets.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(","));
            }
            self.start_planes(&planes)?;
            self.presets_owed = false;
        } else if self.net.tick() > 0 {
            return Err("the ride started before every peer said its controller preset".into());
        }
        Ok(())
    }

    /// The lobby's ride (N3): its line-up, this machine as `me` in it, and a [`Sim`] a seat, each on
    /// its seat's bike, over the lobby's connection.
    pub fn lobby(net: Arc<openmx_net::Net>, room: &openmx_net::Room, me: &str, sims: Vec<Sim>) -> Result<Online, String> {
        let lineup = Lineup { seats: room.seats.iter().map(|s| s.as_ref().map(|s| s.peer.clone())).collect() };
        if sims.len() != lineup.seats.len() {
            return Err(format!("{} seats and {} rides", lineup.seats.len(), sims.len()));
        }
        let seat = *lineup.mine(me).first().ok_or("the lobby's line-up gives this machine no seat")?;
        let mut session = Netplay::new(lineup.clone(), me, Timing::default(), crate::net::desync::dumps())?;
        // a player this machine adds mid-ride (N20) starts where its first seat started
        session.admit.spawn = Some(rides::seat_bytes(&sims[seat]));
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
            room: Some(room.clone()),
            presets_owed: false,
            mods: Default::default(),
            stock: Vec::new(),
            own_rows: true,
            plane: None,
            stock_plane: None,
            bays: Vec::new(),
            marks: None,
            handed: false,
            spare: None,
        })
    }

    /// A local freeride opened to online players (N10): the host's ride of one from here on, `sim`
    /// its seat as it is now, over `net`, which hosts the room. `spawn` is a seat where the ride
    /// starts, which every peer that joins rides in from.
    pub fn open(net: Arc<openmx_net::Net>, sim: Sim, spawn: &Sim) -> Result<Online, String> {
        let me = openmx_net::HOST;
        let mut session = Netplay::new(Lineup { seats: vec![Some(me.to_string())] }, me, Timing::default(), crate::net::desync::dumps())?;
        // a second player on this machine (N20) rides in where a joiner does
        session.admit.spawn = Some(rides::seat_bytes(spawn));
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
            presets_owed: false,
            mods: Default::default(),
            stock: Vec::new(),
            own_rows: false,
            plane: None,
            stock_plane: None,
            bays: Vec::new(),
            marks: None,
            handed: false,
            spare: None,
        })
    }

    /// Into a ride under way (N10), as `me` in `room`: no seat until the host takes this machine on
    /// and sends the ride's state. `first` is the host's first seat's rider and bike, whose
    /// constants every seat of the ride is decoded onto ([`Rides`]).
    pub fn drop_in(net: Arc<openmx_net::Net>, room: &openmx_net::Room, me: &str, first: Sim) -> Result<Online, String> {
        let mut session = Netplay::joining(openmx_net::HOST, me, Timing::default(), crate::net::desync::dumps())?;
        println!("online: dropping in on {}'s ride, waiting for the host", room.name);
        let spare = Some(rides::seat_bytes(&first));
        // where a player this machine adds would spawn (N20), should the room come to it
        session.admit.spawn = spare.clone();
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
            room: Some(room.clone()),
            presets_owed: false,
            mods: Default::default(),
            stock: Vec::new(),
            own_rows: false,
            plane: None,
            stock_plane: None,
            bays: Vec::new(),
            marks: None,
            handed: false,
            spare,
        })
    }

    /// The game modifiers the ride starts on, and each seat's bike file before them (`stock[s]`, a
    /// seat past the last on the first's): its time scale, and what a change mid-ride (P100) is
    /// built on. Online every seat rides 1P's, so the set is 1P's for every seat.
    pub fn modifiers(&mut self, mods: game_sim::modifiers::Mods, stock: Vec<game_sim::phy::Phy>) {
        self.mods = self.set_of(&mods);
        self.time_scale = self.mods.time_scale();
        self.net.mods = self.mods.text();
        self.stock = stock;
    }

    /// The ride's biplane (N17): `stock`, the disc's `biplane.phy`, built on the modifiers the ride
    /// is on ([`Online::modifiers`], called first) as `like` (a plane of this ride's) was, for every
    /// seat's switch to the plane to take a copy of. Without both, a seat asking for the plane stays
    /// on its bike.
    pub fn planes(&mut self, stock: Option<game_sim::phy::Plane>, like: Option<&Flight>) {
        self.plane = stock.as_ref().zip(like).map(|(s, l)| rides::plane_on(s, &self.mods, l));
        self.stock_plane = stock;
    }

    /// Each seat's own bay in the ride's lot (N18): seat `s` stands in `bays[s]` if it starts in the
    /// plane ([`Online::start_planes`]). Every machine parks the same lot off the same disc.
    ///
    /// They are each seat's plane start for marker slot 1 too (P132), beside the first seat's bike
    /// start that every seat's bike is a copy of: [`game_sim::markers::Home::online`], the same on
    /// every machine, put on every seat (and so on any seat a join adds, a copy of the first).
    pub fn bays(&mut self, bays: Vec<Spawn>) {
        if let Some(first) = self.sims.first() {
            let home = std::sync::Arc::new(first.home.online(bays.iter().map(|b| (b.at, b.facing)).collect()));
            self.sims.iter_mut().for_each(|sim| sim.home = std::sync::Arc::clone(&home));
        }
        self.bays = bays;
    }

    /// The seats `flies` says start the ride in the biplane (N18): each takes a copy of the ride's
    /// plane ([`Online::planes`], called first) into its [`Sim::plane`], standing in its own bay
    /// ([`Online::bays`]), before the first tick, so every machine's tick 0 has it — the lobby's
    /// ride from its picks' machine, `--peer`s' from their cards ([`crate::net::udp::PLANE`]). A
    /// ride with no plane, or no bay for the seat, starts the seat on the bike and says so; every
    /// machine has the same disc, and so does the same.
    pub fn start_planes(&mut self, flies: &[bool]) -> Result<(), String> {
        if self.net.tick() > 0 {
            return Err("the ride started before every seat's machine was known".into());
        }
        for (s, sim) in self.sims.iter_mut().enumerate().filter(|&(s, _)| flies.get(s) == Some(&true)) {
            match (&self.plane, self.bays.get(s)) {
                (Some(template), Some(&bay)) => vehicle::start_lane(template, bay, sim),
                _ => println!("online: seat {} picked the biplane, but this ride has none for it; the bike", s + 1),
            }
        }
        let n = self.sims.iter().filter(|s| s.plane.is_some()).count();
        if n > 0 {
            println!("online: {n} {} in the biplane", if n == 1 { "seat starts" } else { "seats start" });
        }
        Ok(())
    }

    /// Whether every seat's starting machine is in the ride yet (N18): `--peer`s say theirs with
    /// their controller presets, before the first tick; every other ride knows them from the start.
    pub fn machines_known(&self) -> bool {
        !self.presets_owed
    }

    /// Which seats of the lobby's `room` start in the biplane (N18): each taken seat's pick's machine.
    pub fn picked_planes(room: &openmx_net::Room) -> Vec<bool> {
        room.seats.iter().map(|s| s.as_ref().is_some_and(|s| s.pick.machine == vehicle::Kind::Biplane.name())).collect()
    }

    /// `--peer`s (P105): this machine's player's saved markers on `track` for `bike`, which every
    /// machine's first seat for it starts the ride with — sent to every peer before the first tick,
    /// as every peer's come here. Not called, this machine's first seat brings none.
    pub fn saved_markers(&mut self, mine: &game_sim::markers::Markers, track: &str, bike: &str) {
        if let (Wire::Udp(udp), Some(marks)) = (&mut self.wire, &mut self.marks) {
            udp.set_markers(crate::saved_markers::to_wire(mine, bike, track));
            (marks.track, marks.bike) = (track.to_string(), bike.to_string());
        }
    }

    /// `--peer`s (P105): offer this machine's saved markers until the ride ticks, and take every
    /// seat's once every peer's are in. `false` while some are still to come: the session hears
    /// nothing and runs nothing until then, so no peer synchronises with this machine — and no ride
    /// ticks — before every machine has every seat's.
    fn take_markers(&mut self) -> bool {
        let (Wire::Udp(udp), Some(marks)) = (&mut self.wire, &mut self.marks) else { return true };
        if self.net.tick() == 0 && marks.offered.is_none_or(|t| t.elapsed() >= OFFER_EVERY) {
            udp.offer();
            marks.offered = Some(Instant::now());
        }
        if !marks.owed {
            return true;
        }
        let Some(all) = udp.markers(&self.net.lineup, &self.net.me) else { return false };
        for (s, (sim, bytes)) in self.sims.iter_mut().zip(&all).enumerate() {
            sim.markers = crate::saved_markers::from_wire(bytes, &marks.bike, &marks.track);
            let placed = sim.markers.placed();
            if placed > 0 {
                println!("online: seat {} starts on {placed} saved markers", s + 1);
            }
        }
        marks.owed = false;
        true
    }

    /// The modifiers the ride is on now: the host's last change once every machine took it.
    pub fn mods(&self) -> game_sim::modifiers::Mods {
        self.mods
    }

    /// The tick the last change of the modifiers was taken at, every machine alike.
    pub fn remodified_at(&self) -> Option<u32> {
        self.net.remods.last().map(|r| r.0)
    }

    /// Whether this machine hosts the ride: only the host changes its modifiers.
    pub fn host(&self) -> bool {
        self.net.host()
    }

    /// How many ticks the session has run: what `--frames` counts online, where a frame spent
    /// waiting for a peer (still loading, or a stall for its pads) runs none.
    pub fn ticks(&self) -> u64 {
        self.net.tick() as u64
    }

    /// The set this ride rides `m` as: each seat its own row from the lobby (P134), 1P's for every
    /// seat otherwise.
    pub fn set_of(&self, m: &game_sim::modifiers::Mods) -> game_sim::modifiers::Mods {
        if self.own_rows { rides::ride_set(m) } else { first_players(m) }
    }

    /// Whether each seat rides its own modifiers here (P134), so a guest can change its own.
    pub fn own_rows(&self) -> bool {
        self.own_rows
    }

    /// This machine's seats in the ride.
    pub fn my_seats(&self) -> Vec<usize> {
        self.net.lineup.mine(&self.net.me)
    }

    /// Whether a player on this machine can take a seat in the ride as it runs (N20): a ride over
    /// the lobby (a `--peer` ride's seats are its `--seats`, every machine's presets and markers
    /// swapped for them before it starts) this machine is in and not leaving.
    pub fn can_sit(&self) -> bool {
        matches!(self.wire, Wire::Lobby { .. }) && self.seated() && self.net.end.is_none() && !self.net.leave.leaving()
    }

    /// One more player on this machine takes a seat (N20), on controller preset `preset`: the host
    /// seats them at a tick every machine takes it at ([`Netplay::sit`]); this machine's
    /// [`Online::my_seats`] has the seat from then, its shell seat right after its other players'
    /// ([`Online::drawn`]), riding the pad [`Online::frame_seats`] hands it.
    pub fn sit(&mut self, preset: u8) -> Result<(), String> {
        if !self.can_sit() {
            return Err("this ride takes no more players from this machine".into());
        }
        let r = self.net.sit(preset);
        for line in self.net.said.drain(..) {
            println!("{line}");
        }
        r
    }

    /// The host changes the ride's modifiers (P100): every machine takes them at the tick of the
    /// next restart the session can make, the way a join is taken ([`crate::net::admit`]); until
    /// then the ride is on the old ones. `to` is the host's own set; from the lobby (P134) the
    /// world's and the biplane's are its 1P's, the host's seat its own, and the knobs `all` the host
    /// set through All Players every seat's; otherwise every seat rides 1P's of it.
    pub fn remodify(&mut self, to: &game_sim::modifiers::Mods, all: &[game_sim::modifiers::Knob]) -> Result<(), String> {
        if self.stock.is_empty() {
            return Err("this ride has no stock bike file to build the modifiers on".into());
        }
        if !self.own_rows {
            return self.net.remodify(first_players(to).text());
        }
        let ride = game_sim::modifiers::Mods::parse(self.net.mods_next())?;
        self.net.remodify(rides::host_set(&ride, to, &self.my_seats(), all).text())
    }

    /// A guest's own modifiers (P134), its machine's 1P's of `own`, onto its seat at a tick every
    /// machine agrees, through the host. Only a ride from the lobby has a row a seat.
    pub fn remodify_own(&mut self, own: &game_sim::modifiers::Mods) -> Result<(), String> {
        if !self.own_rows {
            return Err("a ride opened to online rides the host's modifiers for every seat".into());
        }
        self.net.remodify_own(rides::own_set(own, 0, false).text())
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

    /// Where [`Online::leave`] takes this machine, for the pause's confirm (P110): back to the lobby
    /// for the host of a lobby's ride, out of the game for a guest, and to the main menu off an opened
    /// freeride.
    pub fn leaves_to(&self) -> crate::menu::LeaveTo {
        use crate::menu::LeaveTo;
        match &self.wire {
            Wire::Lobby { .. } if self.spawn.is_some() => LeaveTo::Main,
            Wire::Lobby { .. } if self.net.me == openmx_net::HOST || self.handed => LeaveTo::Lobby,
            Wire::Lobby { .. } => LeaveTo::Online,
            _ => LeaveTo::Main,
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
        if self.net.me == openmx_net::HOST || self.handed {
            net.send(openmx_net::Cmd::Lobby);
            true
        } else {
            net.send(openmx_net::Cmd::Leave);
            false
        }
    }

    /// Leave Session from the pause (N11): this machine's seats go and the ride goes on for the others
    /// — every seat of its for its first player (`local` 0), else that local player's seat only. The
    /// session takes them out at a tick every machine agrees on; [`Online::frame`] then passes the
    /// others what this machine owes them, and lets the lobby go ([`Online::gone`]).
    pub fn leave_session(&mut self, local: usize) -> Result<(), String> {
        let mine = self.net.lineup.mine(&self.net.me);
        let seats = match local {
            0 => Vec::new(),
            k => vec![*mine.get(k).ok_or(format!("this machine has no player {}", k + 1))?],
        };
        self.net.leave(&seats)
    }

    /// Whether this machine is on its way out of the ride (Leave Session), for the pause.
    pub fn leaving(&self) -> bool {
        self.net.leave.leaving()
    }

    /// Out of the ride after Leave Session (N11): out of the lobby too, a host handing its room to
    /// the peer the session made host.
    fn gone(&self) {
        let Wire::Lobby { net, .. } = &self.wire else { return };
        match &self.net.leave.heir {
            Some(heir) => net.send(openmx_net::Cmd::HandOver(heir.clone())),
            None => net.send(openmx_net::Cmd::Leave),
        }
    }

    /// Everything that came in since the last frame, as (peer, datagram); `Err` when the lobby's
    /// network ended the ride, or a `--peer` runs another build.
    fn receive(&mut self) -> Result<Vec<(String, Vec<u8>)>, String> {
        use openmx_net::Event;
        let me = self.net.me.clone();
        let session = &mut self.net;
        let (spawn, arrived, room) = (&self.spawn, &mut self.arrived, &mut self.room);
        let first = self.sims.first();
        let (template, bays) = (self.plane.as_ref(), &self.bays);
        match &mut self.wire {
            Wire::Udp(udp) => udp.receive(),
            Wire::Lobby { net, kept } => {
                let mut got = Vec::new();
                let mut end = None;
                let mut handed = false;
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
                            // the joiner's player's saved markers (P105), as its handshake put
                            // them in its pick: on its seat, in the state every machine is sent
                            let marked = match (seats.as_slice(), seats.first().and_then(|&s| pick(s))) {
                                ([_], Some(seat)) if !seat.pick.markers.is_empty() => {
                                    let track = room.as_ref().map_or("", |r| r.track.as_str());
                                    let m = crate::saved_markers::from_pick(&seat.pick.markers, &seat.pick.bike, track);
                                    let placed = m.placed();
                                    match first.map(|f| rides::spawn_marked(f, spawn, m)) {
                                        Some(Ok(b)) if placed > 0 => {
                                            println!("online: {peer} rides in on {placed} saved markers");
                                            Some(b)
                                        }
                                        Some(Err(why)) => {
                                            println!("online: {peer}'s saved markers: {why}");
                                            None
                                        }
                                        _ => None,
                                    }
                                }
                                _ => None,
                            };
                            let spawn = marked.as_deref().unwrap_or(spawn);
                            // the joiner's setup machine (N19), as its pick says it: a seat that
                            // picked the plane rides in flying from its own bay, in the state
                            // every machine is sent, as a lobby ride's seat starts in it (N18)
                            let flying = match (seats.as_slice(), seats.first().and_then(|&s| pick(s))) {
                                ([s], Some(seat)) if seat.pick.machine == vehicle::Kind::Biplane.name() => match (first, template, bays.get(*s)) {
                                    (Some(f), Some(t), Some(&bay)) => match rides::spawn_flying(f, spawn, t, bay) {
                                        Ok(b) => {
                                            println!("online: {peer} rides in on the biplane");
                                            Some(b)
                                        }
                                        Err(why) => {
                                            println!("online: {peer}'s biplane: {why}");
                                            None
                                        }
                                    },
                                    _ => {
                                        println!("online: {peer} picked the biplane, but this ride has none for seat {}; the bike", s + 1);
                                        None
                                    }
                                },
                                (seats, _) if seats.len() > 1 && seats.iter().any(|&s| pick(s).is_some_and(|seat| seat.pick.machine == vehicle::Kind::Biplane.name())) => {
                                    println!("online: {peer} brings {} seats, which ride in on the bike", seats.len());
                                    None
                                }
                                _ => None,
                            };
                            let spawn = flying.as_deref().unwrap_or(spawn);
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
                        // the host went and the lobby handed this machine its room (N11)
                        Event::HandedOver(ref h) if *h == me => {
                            handed = true;
                            kept.push(e);
                        }
                        e => kept.push(e),
                    }
                }
                if handed {
                    println!("online: the host left: this machine hosts the room now");
                    self.handed = true;
                    if self.spawn.is_none() {
                        self.spawn = self.spare.take();
                    }
                    if self.net.admit.spawn.is_none() {
                        self.net.admit.spawn = self.spawn.clone();
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
    /// order. Shell seat `d` is seat `drawn()[d]`. With more than one player here (N20) each of
    /// this machine's seats comes first, in seat order — the shell's views, one a local player.
    pub fn drawn(&self) -> Vec<usize> {
        let mut lead: Vec<usize> = self.my_seats().into_iter().filter(|&s| s < self.sims.len()).collect();
        if lead.first() != Some(&self.seat) {
            lead.retain(|&s| s != self.seat);
            lead.insert(0, self.seat);
        }
        let rest: Vec<usize> = (0..self.sims.len()).filter(|s| !lead.contains(s)).collect();
        lead.into_iter().chain(rest).collect()
    }

    /// Every seat's player by name, `None` for an empty seat: what the markers' slot display calls
    /// the other players (P88).
    pub fn names(&self) -> Vec<Option<String>> {
        (0..self.sims.len()).map(|s| self.net.lineup.seats.get(s).cloned().flatten()).collect()
    }

    /// Shell seat `d`'s room seat as the network last said it (N16): who sits in it and their pick,
    /// whose look the shell dresses the seat's rider and bike in. `None` for an empty seat, and for a
    /// seat the room has nobody in under the session's name for it (a `--peer` ride has no room).
    pub fn seat_pick(&self, d: usize) -> Option<&openmx_net::room::Seat> {
        let &seat = self.drawn().get(d)?;
        let peer = self.net.lineup.seats.get(seat)?.as_ref()?;
        let seats = &self.room.as_ref()?.seats;
        // The room's seat of the same number if the peer sits in it, else the peer's own seats in
        // order: the session's numbers are the room's, but a restart may have moved them.
        let theirs = || {
            let n = self.net.lineup.seats[..seat].iter().filter(|s| s.as_ref() == Some(peer)).count();
            seats.iter().flatten().filter(|s| s.peer == *peer).nth(n)
        };
        seats.get(seat).and_then(Option::as_ref).filter(|s| s.peer == *peer).or_else(theirs)
    }

    /// Whether shell seat `d`'s player had their pause open on the last tick ridden (P84): the
    /// paused bit of that tick's pad, as every machine stepped it, so the seat frozen and drawn
    /// see-through is the same seat on every machine.
    pub fn paused(&self, d: usize) -> bool {
        let Some(&seat) = self.drawn().get(d) else { return false };
        let pad = self.net.log.last().and_then(|pads| pads.get(seat).copied());
        pad.is_some_and(|p| vehicle::Wanted::of(crate::net::unwire(p).0[crate::net::SWITCH]).paused)
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
            // its hold and its feet off the pegs (V68), which the seat's body machine stepped with it
            body.held_by(&sim.rider);
        }
        self.net.lineup.seats[seat].is_some()
    }

    /// One frame: `pad` rides this machine's first seat; [`Online::show`] reads every seat after.
    /// `body` is any rider's, for the clip table the body's machine reads. `Err` is why the ride is
    /// over.
    pub fn frame(&mut self, pad: Report, place: &mut dyn World, body: Option<&Body>) -> Result<(), String> {
        self.frame_seats(&[pad], place, body)
    }

    /// [`Online::frame`] with a pad for each of this machine's players (N20): `pads[k]` rides its
    /// `k`th seat in seat order, a seat with no pad here an idle one.
    pub fn frame_seats(&mut self, pads: &[Report], place: &mut dyn World, body: Option<&Body>) -> Result<(), String> {
        let came = match self.receive() {
            Ok(came) => came,
            Err(why) => {
                self.net.finish(why.clone());
                return Err(format!("online: {why}"));
            }
        };
        if !self.take_markers() {
            self.last = Instant::now();
            return Ok(());
        }
        for (from, d) in came {
            self.net.heard(&from, &d);
        }
        if let Err(why) = self.take_presets() {
            self.net.finish(why.clone());
            return Err(format!("online: {why}"));
        }
        let mine = self.net.lineup.mine(&self.net.me);
        let local: Vec<Report> = (0..mine.len()).map(|k| pads.get(k).copied().unwrap_or_else(Report::idle)).collect();
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
            let mut rides = Rides { sims: &mut self.sims, world: place, after: Some(&mut after), time_scale: self.time_scale, plane: self.plane.as_mut() };
            let mut game = Modded { rides: &mut rides, stock: &self.stock, mods: &mut self.mods, stock_plane: self.stock_plane.as_ref() };
            self.net.frame(&mut game, &local, real);
        }
        self.time_scale = self.mods.time_scale();
        if let Err(why) = self.take_presets() {
            self.net.finish(why);
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
            // a leaver (N11) first passes on what it owes the others: a host, the restart that hands
            // the ride on
            if self.net.leave.leaving() {
                if !self.net.flushed() {
                    return Ok(());
                }
                self.gone();
            }
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

/// 1P's modifiers for every player: what an online ride rides, every seat's bike built on 1P's.
pub fn first_players(m: &game_sim::modifiers::Mods) -> game_sim::modifiers::Mods {
    use game_sim::modifiers::{Knob, Mods};
    let mut one = Mods::original();
    for k in Knob::ALL {
        for s in 0..game_sim::MAX_PLAYERS {
            one.set(s, k, m.get(0, k));
        }
    }
    one
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
