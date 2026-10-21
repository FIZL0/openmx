//! Four-character room codes (opentennis's short codes, `net.rs` B104, with its pkarr relay client).
//!
//! A hosted lobby gets a [`SHORT`]-symbol code people can read out and type. The code names a well-known ed25519
//! key (its seed is the code, [`code_key`]); the host puts its ticket (endpoint ID, relay, direct addresses) under
//! that key as a BEP 44 mutable item on the mainline DHT (no salt; seq = unix µs), as a one-TXT-record DNS packet
//! so the same item is also a pkarr record, and through the pkarr relays (HTTP gateways onto the same items:
//! `relay.pkarr.org`, and first your own from `online.txt`'s `code_server`, `tools/pkarr-relay`). A joiner looks the
//! code up on the LAN first (a host's mDNS listing carries its code, so no internet is needed), then on the DHT and
//! the relays at once. A host re-rolls a code written less than [`STALE`] seconds ago and republishes with its
//! listing; once it stops, the code goes stale and is free again. Anyone may overwrite a code's item (the key is
//! public): the joiner still dials the ticket's endpoint, which QUIC authenticates.

// In a browser (N4) there is no DHT and no mDNS, so their halves go unused there.
#![cfg_attr(target_arch = "wasm32", allow(dead_code, unused_imports, unused_variables, unreachable_code))]

use std::time::Duration;

use iroh::SecretKey;
#[cfg(not(target_arch = "wasm32"))]
use n0_mainline::{Dht, MutableItem, SigningKey};
use n0_future::time::SystemTime;

/// A browser has no UDP socket, so no DHT (N4): codes go through the pkarr relays only. A stand-in nobody can make, so
/// the code that reads the DHT reads the same everywhere.
#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub enum Dht {}

/// The codes' symbols: digits and capitals without the look-alikes 0/O and 1/I (32^4 ≈ 1M codes).
pub const SYMBOLS: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
pub const SHORT: usize = 4;
/// A code or listing older than this (seconds) is a lobby that stopped; hosts republish well inside it.
pub const STALE: u64 = 150;
/// The public pkarr relay, asked after `online.txt`'s own.
pub const PKARR: &str = "https://relay.pkarr.org";

pub fn is_short(code: &str) -> bool {
    code.len() == SHORT && code.bytes().all(|b| SYMBOLS.contains(&b))
}

/// A random code.
pub fn new_short() -> String {
    // a fresh key's bytes as the randomness (no rand dependency); 5 bits of each of the first four
    SecretKey::generate().to_bytes()[..SHORT].iter().map(|b| SYMBOLS[(b & 31) as usize] as char).collect()
}

/// The room code in typed or pasted text, in capitals (`None`: not a code).
pub fn code_in(text: &str) -> Option<String> {
    let t = text.trim();
    let t = t.split_once("code=").map_or(t, |(_, c)| c.split(['&', '#']).next().unwrap_or(""));
    let up = t.to_ascii_uppercase();
    is_short(&up).then_some(up)
}

/// The (public) key a code's record is written under.
pub fn code_key(code: &str) -> SecretKey {
    let mut seed = [0u8; 32];
    let s = format!("openmx-lobby-code/1/{code}");
    seed[..s.len()].copy_from_slice(s.as_bytes());
    SecretKey::from_bytes(&seed)
}

/// `text` as a DNS response with one TXT record `_omx` (strings of ≤ 255 bytes): a pkarr relay only takes DNS packets.
pub fn dns_txt(text: &str) -> Vec<u8> {
    let mut rdata = Vec::new();
    for c in text.as_bytes().chunks(255) {
        rdata.push(c.len() as u8);
        rdata.extend(c);
    }
    let mut p = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0, 4, b'_', b'o', b'm', b'x', 0, 0, 16, 0, 1, 0, 0, 1, 44];
    p.extend((rdata.len() as u16).to_be_bytes());
    p.extend(rdata);
    p
}

/// The first answer's TXT strings, joined (`dns_txt` read back).
pub fn txt_of(p: &[u8]) -> Option<String> {
    let mut i = 12;
    loop {
        match *p.get(i)? {
            0 => break i += 1,
            l if l >= 0xc0 => break i += 2,
            l => i += 1 + l as usize,
        }
    }
    let n = u16::from_be_bytes(p.get(i + 8..i + 10)?.try_into().ok()?) as usize;
    let mut r = p.get(i + 10..i + 10 + n)?;
    let mut out = Vec::new();
    while let Some((&l, rest)) = r.split_first() {
        out.extend(rest.get(..l as usize)?);
        r = &rest[l as usize..];
    }
    String::from_utf8(out).ok()
}

/// What a pkarr relay's PUT carries and its GET answers: the signature over BEP 44's bencoded `seq` and `v`, the seq
/// (big-endian), then `v` (the DNS packet).
pub fn relay_body(key: &SecretKey, seq: i64, v: &[u8]) -> Vec<u8> {
    let sig = key.sign(&[format!("3:seqi{seq}e1:v{}:", v.len()).into_bytes(), v.to_vec()].concat());
    [sig.to_bytes().to_vec(), seq.to_be_bytes().to_vec(), v.to_vec()].concat()
}

/// A relay's answer to a GET: the TXT text and its seq.
pub fn relay_record(b: &[u8]) -> Option<(String, i64)> {
    let seq = b.get(64..72).and_then(|s| s.try_into().ok()).map(i64::from_be_bytes)?;
    Some((b.get(72..).and_then(txt_of)?, seq))
}

pub fn unix_micros() -> i64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_micros() as i64)
}

/// Written less than [`STALE`] ago.
pub fn fresh(seq: i64) -> bool {
    unix_micros().saturating_sub(seq) <= STALE as i64 * 1_000_000
}

/// Where codes are read and written: the DHT (none with the board off) and the pkarr relays, in the order asked.
#[derive(Clone)]
pub struct Codes {
    pub dht: Option<Dht>,
    pub servers: Vec<String>,
    http: reqwest::Client,
}

/// A lookup's answer: the ticket and when it was written; Ok(None): nobody wrote it; Err: why it couldn't be read.
pub type Found = Result<Option<(String, i64)>, String>;

impl Codes {
    pub fn new(dht: Option<Dht>, servers: Vec<String>) -> Codes {
        #[cfg(not(target_arch = "wasm32"))]
        let _ = rustls::crypto::ring::default_provider().install_default();
        #[cfg(not(target_arch = "wasm32"))]
        let http = reqwest::Client::builder().timeout(Duration::from_secs(25)).build().expect("the HTTP client");
        // the page's fetch, which has no timeout of its own to set (a lookup's caller bounds it)
        #[cfg(target_arch = "wasm32")]
        let http = reqwest::Client::new();
        Codes { dht, servers: servers.into_iter().map(|s| s.trim_end_matches('/').to_string()).filter(|s| !s.is_empty()).collect(), http }
    }

    async fn dht_get(&self, code: &str) -> Found {
        let Some(dht) = &self.dht else { return Ok(None) };
        #[cfg(target_arch = "wasm32")]
        match *dht {}
        #[cfg(not(target_arch = "wasm32"))]
        {
            let item = dht.get_mutable_most_recent(code_key(code).public().as_bytes(), None).await.map_err(|e| format!("the DHT: {e}"))?;
            Ok(item.and_then(|i| Some((txt_of(i.value())?, i.seq()))))
        }
    }

    /// The record under `name` on the first relay that answers.
    pub async fn relay_get(&self, name: &str) -> Found {
        let mut why = Ok(None);
        for s in &self.servers {
            // a query so no cache between here and the relay hands back an old copy
            let url = format!("{s}/{}?t={}", code_key(name).public().to_z32(), unix_micros());
            match self.http.get(url).send().await {
                Ok(r) if r.status().as_u16() == 404 => {}
                Ok(r) if r.status().is_success() => match r.bytes().await {
                    Ok(b) => return Ok(relay_record(&b)),
                    Err(e) => why = Err(trouble(s, None, &e.to_string())),
                },
                Ok(r) => why = Err(trouble(s, Some(r.status().as_u16()), "")),
                Err(e) => why = Err(trouble(s, None, &e.to_string())),
            }
        }
        why
    }

    /// Puts `text` under `name` on every relay; Ok if one took it.
    pub async fn relay_put(&self, name: &str, text: &str) -> Result<(), String> {
        let key = code_key(name);
        let body = relay_body(&key, unix_micros(), &dns_txt(text));
        let mut why = "no code server set".to_string();
        let mut ok = false;
        for s in &self.servers {
            match self.http.put(format!("{s}/{}", key.public().to_z32())).body(body.clone()).send().await {
                Ok(r) if r.status().is_success() => ok = true,
                Ok(r) => why = trouble(s, Some(r.status().as_u16()), ""),
                Err(e) => why = trouble(s, None, &e.to_string()),
            }
        }
        if ok { Ok(()) } else { Err(why) }
    }

    /// The freshest record under `code` on the DHT and the relays, asked at once.
    pub async fn lookup(&self, code: &str) -> Found {
        let (d, r) = tokio::join!(self.dht_get(code), self.relay_get(code));
        match (d, r) {
            (Ok(a), Ok(b)) => Ok(a.into_iter().chain(b).max_by_key(|(_, seq)| *seq)),
            (Ok(Some(a)), Err(_)) | (Err(_), Ok(Some(a))) => Ok(Some(a)),
            (Err(e), _) | (_, Err(e)) => Err(e),
        }
    }

    /// Puts `ticket` under `code` on the DHT and the relays at once: Ok once one of them took it.
    pub async fn publish(&self, code: &str, ticket: &str) -> Result<(), String> {
        let dht = async {
            let Some(dht) = &self.dht else { return Err("no DHT".to_string()) };
            #[cfg(target_arch = "wasm32")]
            match *dht {}
            #[cfg(not(target_arch = "wasm32"))]
            {
                let item = MutableItem::new(&SigningKey::from_bytes(&code_key(code).to_bytes()), &dns_txt(ticket), unix_micros(), None);
                dht.put_mutable(item, None).await.map(|_| ()).map_err(|e| format!("the DHT: {e}"))
            }
        };
        let relays = async { if self.servers.is_empty() { Err("no code server".to_string()) } else { self.relay_put(code, ticket).await } };
        match tokio::join!(dht, relays) {
            (Ok(()), _) | (_, Ok(())) => Ok(()),
            (Err(a), Err(b)) => Err(if self.dht.is_none() { b } else { format!("{a}; {b}") }),
        }
    }

    /// A free code for the lobby with `ticket`, `code` first, published; each code tried after a taken one goes to
    /// `show`.
    pub async fn roll(&self, mut code: String, ticket: &str, show: impl Fn(&str)) -> Result<String, String> {
        for _ in 0..5 {
            match n0_future::time::timeout(Duration::from_secs(15), self.lookup(&code)).await {
                Ok(Ok(Some((_, seq)))) if fresh(seq) => {
                    code = new_short();
                    show(&code);
                }
                _ => return self.publish(&code, ticket).await.map(|_| code),
            }
        }
        Err("five codes in a row were taken".into())
    }
}

/// A relay's refusal or absence in words a player can act on.
fn trouble(server: &str, status: Option<u16>, err: &str) -> String {
    match status {
        Some(429) => format!("the code server ({server}) is refusing this network for now (too many requests): wait a minute and try again"),
        Some(s) => format!("the code server ({server}) answered HTTP {s}"),
        None => format!("couldn't reach the code server ({server}: {err}): check the internet connection"),
    }
}

/// Why `code` led to no lobby, from its last lookup.
pub fn missed(code: &str, last: Result<Option<i64>, String>) -> String {
    match last {
        Ok(Some(seq)) => {
            let m = unix_micros().saturating_sub(seq) / 60_000_000;
            let ago = if m < 2 { "about a minute".into() } else if m < 120 { format!("{m} minutes") } else { "hours".into() };
            format!("The lobby {code} stopped answering {ago} ago: the host left it. Ask for the code their lobby shows now.")
        }
        Ok(None) => format!("No open lobby has the code {code}: check it against the host's screen."),
        Err(e) => format!("Couldn't look up {code}: {e}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_parts() {
        let t = "endpointabc".repeat(60);
        assert_eq!(txt_of(&dns_txt(&t)).as_deref(), Some(t.as_str()));
        let key = code_key("K7QX");
        let b = relay_body(&key, 42, &dns_txt("ticket"));
        assert_eq!(relay_record(&b), Some(("ticket".into(), 42)));
        assert_eq!(code_in(" k7qx "), Some("K7QX".into()));
        assert_eq!(code_in("openmx://join?code=k7qx&x=1"), Some("K7QX".into()));
        assert_eq!(code_in("K0QX"), None, "0 isn't a symbol");
        assert_eq!(code_in("K7QXY"), None);
        assert!(is_short(&new_short()));
        assert_ne!(code_key("AAAA").public(), code_key("AAAB").public());
        assert!(fresh(unix_micros() - 140_000_000) && !fresh(unix_micros() - 160_000_000));
        assert!(missed("K7QX", Ok(None)).starts_with("No open lobby has the code K7QX"));
        assert!(missed("K7QX", Ok(Some(unix_micros() - 5 * 60_000_000))).contains("5 minutes ago"));
    }
}
