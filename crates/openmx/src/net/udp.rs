//! The shell's transport until N3's: plain UDP between addresses given on the command line. One
//! socket, non-blocking; a datagram from an address nobody named is dropped.
//!
//! Every datagram carries this build's id ahead of the session's bytes (N7): [`MAGIC`], the id's
//! length, the id (the lobby's, `openmx_net::handshake::build`, which names the ride's tick). Two
//! builds can't be trusted to simulate alike — a 60 Hz one and a 30 Hz one (N6) would start a session
//! and desync — so a datagram with another build's id, or none (a build from before N7), ends the
//! ride with both ids, as the lobby refuses them before one starts.
//!
//! After the id goes the sender's card (N15): a byte a seat, in seat order, of what the path has no
//! room or handshake to say, each of the sender's seats' controller preset (P79). A session cannot
//! tick before it has heard every peer (ggrs' sync is round trips), so every machine has every
//! seat's preset by the first tick, and builds each seat's table on it ([`Udp::presets`]), as the
//! lobby's ride does from the room's picks (N13). The byte's top bit ([`PLANE`]) says the seat starts
//! the ride in the biplane (N18), which every machine puts it in before the first tick
//! ([`Udp::planes`]), as the lobby's ride does from its picks' machine.

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use super::Lineup;

/// The first bytes of every datagram on this path.
pub const MAGIC: &[u8; 3] = b"omp";

/// The first bytes of a datagram from a build from N7 to N14, which sent no card.
pub const NO_CARD: &[u8; 3] = b"omx";

/// A card byte's flag (N18): the seat starts the ride in the biplane. The rest of the byte is its
/// controller preset.
pub const PLANE: u8 = 0x80;

pub struct Udp {
    sock: UdpSocket,
    peers: Vec<(String, SocketAddr)>,
    /// What goes ahead of every datagram: [`MAGIC`], the build id's length, the id, the card's
    /// length, the card.
    head: Vec<u8>,
    /// This machine's card: each of its seats' controller preset, in seat order.
    card: Vec<u8>,
    /// Each peer's card, as its last datagram said it.
    cards: HashMap<String, Vec<u8>>,
}

/// `name@host:port`.
pub fn named(s: &str) -> Result<(String, SocketAddr), String> {
    let (name, addr) = s.split_once('@').ok_or(format!("`{s}`: expected name@host:port"))?;
    let addr = addr.to_socket_addrs().map_err(|e| format!("{addr}: {e}"))?.next().ok_or(format!("{addr}: no address"))?;
    Ok((name.to_string(), addr))
}

/// The build id a datagram carries, its card and the session's bytes after them; `None` when it
/// carries none.
pub fn untagged(d: &[u8]) -> Option<(&[u8], &[u8], &[u8])> {
    let rest = d.strip_prefix(MAGIC)?;
    let (&n, rest) = rest.split_first()?;
    let (id, rest) = rest.split_at_checked(n as usize)?;
    let (&n, rest) = rest.split_first()?;
    let (card, data) = rest.split_at_checked(n as usize)?;
    Some((id, card, data))
}

impl Udp {
    /// This machine at `at`, its seats on the controller presets `card` (in seat order).
    pub fn bind(at: SocketAddr, peers: Vec<(String, SocketAddr)>, card: &[u8]) -> Result<Udp, String> {
        Udp::bind_as(at, peers, openmx_net::handshake::build(), card)
    }

    /// [`Udp::bind`] as the build `build` (the tests' other builds).
    pub fn bind_as(at: SocketAddr, peers: Vec<(String, SocketAddr)>, build: &str, card: &[u8]) -> Result<Udp, String> {
        let sock = UdpSocket::bind(at).map_err(|e| format!("{at}: {e}"))?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        let id = &build.as_bytes()[..build.len().min(255)];
        let card = &card[..card.len().min(255)];
        let head = [&MAGIC[..], &[id.len() as u8], id, &[card.len() as u8], card].concat();
        Ok(Udp { sock, peers, head, card: card.to_vec(), cards: HashMap::new() })
    }

    /// The peers this machine talks to from now on: the tests', which bind every machine first.
    pub fn meet(&mut self, peers: Vec<(String, SocketAddr)>) {
        self.peers = peers;
    }

    /// The address this machine listens on.
    pub fn at(&self) -> SocketAddr {
        self.sock.local_addr().expect("a bound socket")
    }

    /// Every seat's controller preset in `lineup`, this machine being `me`, once every peer in it has
    /// been heard (`None` until then; an empty seat's is 0). `Err` is a peer whose card does not fit
    /// the seats `lineup` gives it — the two machines were given other `--seats` — or names a preset
    /// the original has not.
    pub fn presets(&self, lineup: &Lineup, me: &str) -> Result<Option<Vec<u8>>, String> {
        Ok(self.card_bytes(lineup, me)?.map(|b| b.iter().map(|&b| b & !PLANE).collect()))
    }

    /// Which seats in `lineup` start the ride in the biplane (N18, [`PLANE`]), on
    /// [`Udp::presets`]'s terms: `None` until every peer has been heard; an empty seat is on the bike.
    pub fn planes(&self, lineup: &Lineup, me: &str) -> Result<Option<Vec<bool>>, String> {
        Ok(self.card_bytes(lineup, me)?.map(|b| b.iter().map(|&b| b & PLANE != 0).collect()))
    }

    /// Every seat's card byte in `lineup`, for [`Udp::presets`] and [`Udp::planes`].
    fn card_bytes(&self, lineup: &Lineup, me: &str) -> Result<Option<Vec<u8>>, String> {
        let mut presets = vec![0u8; lineup.seats.len()];
        for p in lineup.peers() {
            let Some(card) = (if p == me { Some(&self.card) } else { self.cards.get(&p) }) else { return Ok(None) };
            let seats = lineup.mine(&p);
            if card.len() != seats.len() {
                return Err(format!(
                    "{p} says {} seats of its own and --seats here gives it {} ({}): two line-ups can't ride together",
                    card.len(),
                    seats.len(),
                    lineup.text()
                ));
            }
            for (&s, &preset) in seats.iter().zip(card) {
                if (preset & !PLANE) as usize >= game_sim::bindings::PRESETS {
                    return Err(format!("{p} rides controller preset {}, which the original has not", preset & !PLANE));
                }
                presets[s] = preset;
            }
        }
        Ok(Some(presets))
    }

    pub fn send(&self, to: &str, data: &[u8]) {
        if let Some((_, a)) = self.peers.iter().find(|p| p.0 == to) {
            // a full buffer or an unreachable peer is a lost datagram, which the session expects
            let _ = self.sock.send_to(&[&self.head[..], data].concat(), a);
        }
    }

    /// Everything that came in since the last call: (peer, datagram). `Err` is a peer on another
    /// build, and why.
    pub fn receive(&mut self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mine = &self.head[MAGIC.len() + 1..][..self.head[MAGIC.len()] as usize];
        let mut got = Vec::new();
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = self.sock.recv_from(&mut buf) {
            let Some((p, _)) = self.peers.iter().find(|p| p.1 == from) else { continue };
            let refused = match untagged(&buf[..n]) {
                Some((id, card, data)) if id == mine => {
                    if self.cards.get(p).is_none_or(|c| c != card) {
                        self.cards.insert(p.clone(), card.to_vec());
                    }
                    got.push((p.clone(), data.to_vec()));
                    continue;
                }
                Some((id, _, _)) => format!(
                    "{p} runs build \"{}\" and this machine \"{}\": two builds can't ride together",
                    String::from_utf8_lossy(id),
                    String::from_utf8_lossy(mine)
                ),
                None if buf[..n].starts_with(NO_CARD) => format!(
                    "{p} sends no controller presets (a build from before N15) and this machine runs \"{}\": two builds can't ride together",
                    String::from_utf8_lossy(mine)
                ),
                None => format!(
                    "{p} sends no build id (a build from before N7) and this machine runs \"{}\": two builds can't ride together",
                    String::from_utf8_lossy(mine)
                ),
            };
            // this machine's id alone, so a peer on another build since N7 is told too, though this
            // machine stops sending: its own first datagrams may have gone before the peer was up
            for _ in 0..3 {
                let _ = self.sock.send_to(&self.head, from);
            }
            return Err(refused);
        }
        Ok(got)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn local() -> SocketAddr {
        "127.0.0.1:0".parse().unwrap()
    }

    /// What `u` received within two seconds, or its refusal.
    fn wait(u: &mut Udp) -> Result<Vec<(String, Vec<u8>)>, String> {
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            let got = u.receive()?;
            if !got.is_empty() || Instant::now() > until {
                return Ok(got);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// A machine `a` and a raw socket as `g` beside it, each naming the other.
    fn pair(build: &str) -> (Udp, UdpSocket) {
        let g = UdpSocket::bind(local()).unwrap();
        let a = Udp::bind_as(local(), vec![("g".into(), g.local_addr().unwrap())], build, &[3]).unwrap();
        (a, g)
    }

    #[test]
    fn one_build_rides() {
        // g echoes what a sent it: the same build's datagram, which a hears with the id taken off
        let (mut a, g) = pair("0.1.0 dev, 30 Hz");
        a.send("g", b"\x01session");
        let mut buf = [0u8; 2048];
        g.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (n, _) = g.recv_from(&mut buf).unwrap();
        assert_eq!(untagged(&buf[..n]), Some((&b"0.1.0 dev, 30 Hz"[..], &[3u8][..], &b"\x01session"[..])));
        g.send_to(&buf[..n], a.sock.local_addr().unwrap()).unwrap();
        assert_eq!(wait(&mut a), Ok(vec![("g".to_string(), b"\x01session".to_vec())]));
    }

    #[test]
    fn another_build_is_refused_by_name() {
        let (mut a, g) = pair("0.1.0 dev, 30 Hz");
        let d = [&MAGIC[..], &[16], b"0.1.0 dev, 60 Hz", &[1, 0], b"\x01session"].concat();
        g.send_to(&d, a.sock.local_addr().unwrap()).unwrap();
        let why = wait(&mut a).unwrap_err();
        assert!(why.contains("g runs build \"0.1.0 dev, 60 Hz\" and this machine \"0.1.0 dev, 30 Hz\""), "{why}");
        // and g is told which build a is, with nothing for its session
        let mut buf = [0u8; 2048];
        g.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (n, _) = g.recv_from(&mut buf).unwrap();
        assert_eq!(untagged(&buf[..n]), Some((&b"0.1.0 dev, 30 Hz"[..], &[3u8][..], &b""[..])));
    }

    #[test]
    fn two_builds_since_the_id_both_say_why() {
        let (ga, gb) = (UdpSocket::bind(local()).unwrap(), UdpSocket::bind(local()).unwrap());
        let (at_a, at_b) = (ga.local_addr().unwrap(), gb.local_addr().unwrap());
        drop((ga, gb));
        let mut a = Udp::bind_as(at_a, vec![("b".into(), at_b)], "0.1.0 dev, 30 Hz", &[0]).unwrap();
        let mut b = Udp::bind_as(at_b, vec![("a".into(), at_a)], "0.1.0 dev, 60 Hz", &[0]).unwrap();
        // b's one datagram: a refuses it and goes quiet, and b hears why from a's answer alone
        b.send("a", b"\x01session");
        assert!(wait(&mut a).unwrap_err().contains("b runs build \"0.1.0 dev, 60 Hz\""));
        assert!(wait(&mut b).unwrap_err().contains("a runs build \"0.1.0 dev, 30 Hz\" and this machine \"0.1.0 dev, 60 Hz\""));
    }

    #[test]
    fn a_build_before_the_id_is_refused() {
        let (mut a, g) = pair("0.1.0 dev, 30 Hz");
        // N2's datagrams start with the session's own bytes
        g.send_to(b"\x01\x00\x07session", a.sock.local_addr().unwrap()).unwrap();
        let why = wait(&mut a).unwrap_err();
        assert!(why.contains("g sends no build id"), "{why}");
    }

    #[test]
    fn a_build_before_the_card_is_refused() {
        let (mut a, g) = pair("0.1.0 dev, 30 Hz");
        // N7's: the same build id, and the session's bytes straight after it
        let d = [&NO_CARD[..], &[16], b"0.1.0 dev, 30 Hz", b"\x01session"].concat();
        g.send_to(&d, a.sock.local_addr().unwrap()).unwrap();
        let why = wait(&mut a).unwrap_err();
        assert!(why.contains("g sends no controller presets (a build from before N15)"), "{why}");
    }

    /// N15: each machine's card reaches the others, and every seat's preset is known once every
    /// peer has been heard, an empty seat's 0.
    #[test]
    fn every_seat_has_its_preset_once_every_peer_is_heard() {
        let lineup = Lineup::parse("host,g,-,k,k").unwrap();
        let mut host = Udp::bind_as(local(), vec![], "b", &[3]).unwrap();
        let mut g = Udp::bind_as(local(), vec![("host".into(), host.at())], "b", &[2]).unwrap();
        let k = Udp::bind_as(local(), vec![("host".into(), host.at())], "b", &[1, 0]).unwrap();
        host.meet(vec![("g".into(), g.at()), ("k".into(), k.at())]);
        assert_eq!(host.presets(&lineup, "host"), Ok(None), "nobody heard yet");
        g.send("host", b"x");
        wait(&mut host).unwrap();
        assert_eq!(host.presets(&lineup, "host"), Ok(None), "k not heard yet");
        k.send("host", b"y");
        assert_eq!(wait(&mut host), Ok(vec![("k".to_string(), b"y".to_vec())]), "the card is not the session's");
        assert_eq!(host.presets(&lineup, "host"), Ok(Some(vec![3, 2, 0, 1, 0])));
        // and g hears the host's
        host.send("g", b"z");
        wait(&mut g).unwrap();
        assert_eq!(g.cards.get("host").map(|c| &c[..]), Some(&[3u8][..]));
        // a peer that counts its seats otherwise was given other --seats
        let why = host.presets(&Lineup::parse("host,g,k").unwrap(), "host").unwrap_err();
        assert!(why.contains("k says 2 seats of its own and --seats here gives it 1"), "{why}");
        // and a preset the original has not is refused
        let bad = Udp::bind_as(local(), vec![("host".into(), host.at())], "b", &[game_sim::bindings::PRESETS as u8, 0]).unwrap();
        host.meet(vec![("g".into(), g.at()), ("k".into(), bad.at())]);
        bad.send("host", b"w");
        wait(&mut host).unwrap();
        assert!(host.presets(&lineup, "host").unwrap_err().contains("k rides controller preset"));
    }

    /// N18: a seat that starts in the biplane says so in its card byte's top bit, beside its
    /// preset; every machine reads the same seats in the plane, and the bit is no preset.
    #[test]
    fn every_seat_s_starting_machine_is_known_once_every_peer_is_heard() {
        let lineup = Lineup::parse("host,g,-,g").unwrap();
        let mut host = Udp::bind_as(local(), vec![], "b", &[2 | PLANE]).unwrap();
        let mut g = Udp::bind_as(local(), vec![("host".into(), host.at())], "b", &[1, PLANE]).unwrap();
        host.meet(vec![("g".into(), g.at())]);
        assert_eq!(host.planes(&lineup, "host"), Ok(None), "g not heard yet");
        g.send("host", b"x");
        wait(&mut host).unwrap();
        host.send("g", b"y");
        wait(&mut g).unwrap();
        for (who, u) in [("host", &host), ("g", &g)] {
            assert_eq!(u.planes(&lineup, who), Ok(Some(vec![true, false, false, true])), "{who}");
            assert_eq!(u.presets(&lineup, who), Ok(Some(vec![2, 1, 0, 0])), "{who}");
        }
    }

    #[test]
    fn strangers_are_still_dropped() {
        let (mut a, _g) = pair("0.1.0 dev, 30 Hz");
        let stranger = UdpSocket::bind(local()).unwrap();
        stranger.send_to(b"anything", a.sock.local_addr().unwrap()).unwrap();
        assert_eq!(wait(&mut a), Ok(Vec::new()));
    }
}
