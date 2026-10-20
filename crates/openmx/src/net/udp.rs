//! The shell's transport until N3's: plain UDP between addresses given on the command line. One
//! socket, non-blocking; a datagram from an address nobody named is dropped.

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

pub struct Udp {
    sock: UdpSocket,
    peers: Vec<(String, SocketAddr)>,
}

/// `name@host:port`.
pub fn named(s: &str) -> Result<(String, SocketAddr), String> {
    let (name, addr) = s.split_once('@').ok_or(format!("`{s}`: expected name@host:port"))?;
    let addr = addr.to_socket_addrs().map_err(|e| format!("{addr}: {e}"))?.next().ok_or(format!("{addr}: no address"))?;
    Ok((name.to_string(), addr))
}

impl Udp {
    pub fn bind(at: SocketAddr, peers: Vec<(String, SocketAddr)>) -> Result<Udp, String> {
        let sock = UdpSocket::bind(at).map_err(|e| format!("{at}: {e}"))?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Udp { sock, peers })
    }

    pub fn send(&self, to: &str, data: &[u8]) {
        if let Some((_, a)) = self.peers.iter().find(|p| p.0 == to) {
            // a full buffer or an unreachable peer is a lost datagram, which the session expects
            let _ = self.sock.send_to(data, a);
        }
    }

    /// Everything that came in since the last call: (peer, datagram).
    pub fn receive(&self) -> Vec<(String, Vec<u8>)> {
        let mut got = Vec::new();
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = self.sock.recv_from(&mut buf) {
            if let Some((p, _)) = self.peers.iter().find(|p| p.1 == from) {
                got.push((p.clone(), buf[..n].to_vec()));
            }
        }
        got
    }
}
