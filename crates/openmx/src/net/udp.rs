//! The shell's transport until N3's: plain UDP between addresses given on the command line. One
//! socket, non-blocking; a datagram from an address nobody named is dropped.
//!
//! Every datagram carries this build's id ahead of the session's bytes (N7): [`MAGIC`], the id's
//! length, the id (the lobby's, `openmx_net::handshake::build`, which names the ride's tick). Two
//! builds can't be trusted to simulate alike — a 60 Hz one and a 30 Hz one (N6) would start a session
//! and desync — so a datagram with another build's id, or none (a build from before N7), ends the
//! ride with both ids, as the lobby refuses them before one starts.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

/// The first bytes of every datagram on this path.
pub const MAGIC: &[u8; 3] = b"omx";

pub struct Udp {
    sock: UdpSocket,
    peers: Vec<(String, SocketAddr)>,
    /// What goes ahead of every datagram: [`MAGIC`], the build id's length, the id.
    head: Vec<u8>,
}

/// `name@host:port`.
pub fn named(s: &str) -> Result<(String, SocketAddr), String> {
    let (name, addr) = s.split_once('@').ok_or(format!("`{s}`: expected name@host:port"))?;
    let addr = addr.to_socket_addrs().map_err(|e| format!("{addr}: {e}"))?.next().ok_or(format!("{addr}: no address"))?;
    Ok((name.to_string(), addr))
}

/// The build id a datagram carries and the session's bytes after it; `None` when it carries none.
pub fn untagged(d: &[u8]) -> Option<(&[u8], &[u8])> {
    let rest = d.strip_prefix(MAGIC)?;
    let (&n, rest) = rest.split_first()?;
    rest.split_at_checked(n as usize)
}

impl Udp {
    pub fn bind(at: SocketAddr, peers: Vec<(String, SocketAddr)>) -> Result<Udp, String> {
        Udp::bind_as(at, peers, openmx_net::handshake::build())
    }

    /// [`Udp::bind`] as the build `build` (the tests' other builds).
    pub fn bind_as(at: SocketAddr, peers: Vec<(String, SocketAddr)>, build: &str) -> Result<Udp, String> {
        let sock = UdpSocket::bind(at).map_err(|e| format!("{at}: {e}"))?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        let id = &build.as_bytes()[..build.len().min(255)];
        let head = [&MAGIC[..], &[id.len() as u8], id].concat();
        Ok(Udp { sock, peers, head })
    }

    pub fn send(&self, to: &str, data: &[u8]) {
        if let Some((_, a)) = self.peers.iter().find(|p| p.0 == to) {
            // a full buffer or an unreachable peer is a lost datagram, which the session expects
            let _ = self.sock.send_to(&[&self.head[..], data].concat(), a);
        }
    }

    /// Everything that came in since the last call: (peer, datagram). `Err` is a peer on another
    /// build, and why.
    pub fn receive(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mine = &self.head[MAGIC.len() + 1..];
        let mut got = Vec::new();
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = self.sock.recv_from(&mut buf) {
            let Some((p, _)) = self.peers.iter().find(|p| p.1 == from) else { continue };
            let refused = match untagged(&buf[..n]) {
                Some((id, data)) if id == mine => {
                    got.push((p.clone(), data.to_vec()));
                    continue;
                }
                Some((id, _)) => format!(
                    "{p} runs build \"{}\" and this machine \"{}\": two builds can't ride together",
                    String::from_utf8_lossy(id),
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
    fn wait(u: &Udp) -> Result<Vec<(String, Vec<u8>)>, String> {
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
        let a = Udp::bind_as(local(), vec![("g".into(), g.local_addr().unwrap())], build).unwrap();
        (a, g)
    }

    #[test]
    fn one_build_rides() {
        // g echoes what a sent it: the same build's datagram, which a hears with the id taken off
        let (a, g) = pair("0.1.0 dev, 30 Hz");
        a.send("g", b"\x01session");
        let mut buf = [0u8; 2048];
        g.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (n, _) = g.recv_from(&mut buf).unwrap();
        assert_eq!(untagged(&buf[..n]), Some((&b"0.1.0 dev, 30 Hz"[..], &b"\x01session"[..])));
        g.send_to(&buf[..n], a.sock.local_addr().unwrap()).unwrap();
        assert_eq!(wait(&a), Ok(vec![("g".to_string(), b"\x01session".to_vec())]));
    }

    #[test]
    fn another_build_is_refused_by_name() {
        let (a, g) = pair("0.1.0 dev, 30 Hz");
        let d = [&MAGIC[..], &[16], b"0.1.0 dev, 60 Hz", b"\x01session"].concat();
        g.send_to(&d, a.sock.local_addr().unwrap()).unwrap();
        let why = wait(&a).unwrap_err();
        assert!(why.contains("g runs build \"0.1.0 dev, 60 Hz\" and this machine \"0.1.0 dev, 30 Hz\""), "{why}");
        // and g is told which build a is, with nothing for its session
        let mut buf = [0u8; 2048];
        g.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let (n, _) = g.recv_from(&mut buf).unwrap();
        assert_eq!(untagged(&buf[..n]), Some((&b"0.1.0 dev, 30 Hz"[..], &b""[..])));
    }

    #[test]
    fn two_builds_since_the_id_both_say_why() {
        let (ga, gb) = (UdpSocket::bind(local()).unwrap(), UdpSocket::bind(local()).unwrap());
        let (at_a, at_b) = (ga.local_addr().unwrap(), gb.local_addr().unwrap());
        drop((ga, gb));
        let a = Udp::bind_as(at_a, vec![("b".into(), at_b)], "0.1.0 dev, 30 Hz").unwrap();
        let b = Udp::bind_as(at_b, vec![("a".into(), at_a)], "0.1.0 dev, 60 Hz").unwrap();
        // b's one datagram: a refuses it and goes quiet, and b hears why from a's answer alone
        b.send("a", b"\x01session");
        assert!(wait(&a).unwrap_err().contains("b runs build \"0.1.0 dev, 60 Hz\""));
        assert!(wait(&b).unwrap_err().contains("a runs build \"0.1.0 dev, 30 Hz\" and this machine \"0.1.0 dev, 60 Hz\""));
    }

    #[test]
    fn a_build_before_the_id_is_refused() {
        let (a, g) = pair("0.1.0 dev, 30 Hz");
        // N2's datagrams start with the session's own bytes
        g.send_to(b"\x01\x00\x07session", a.sock.local_addr().unwrap()).unwrap();
        let why = wait(&a).unwrap_err();
        assert!(why.contains("g sends no build id"), "{why}");
    }

    #[test]
    fn strangers_are_still_dropped() {
        let (a, _g) = pair("0.1.0 dev, 30 Hz");
        let stranger = UdpSocket::bind(local()).unwrap();
        stranger.send_to(b"anything", a.sock.local_addr().unwrap()).unwrap();
        assert_eq!(wait(&a), Ok(Vec::new()));
    }
}
